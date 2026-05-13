//! Tauri command handlers invoked from the React frontend via `invoke()`.
//!
//! `detect_games` returns Steam accounts + games from every enabled
//! launcher. `apply_changes` does the actual library mutation:
//!
//! 1. Pick the Steam account.
//! 2. Load existing `shortcuts.vdf` (or start empty if `--init` flow).
//! 3. Add new shortcuts from the selected games, preserving any
//!    existing entries that aren't ours.
//! 4. Optionally remove shortcuts whose target executables no longer
//!    exist on disk.
//! 5. Back up the old file (unless `live_dangerously`) and write the
//!    new bytes atomically (write to `.tmp`, then `rename`).
//! 6. Optionally download Steam grid art for each selected game in
//!    parallel.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use serde::Serialize;
use tauri::{Emitter, Window};

use crate::api::{download_art_all, ArtTarget, SgdbClient};
use crate::error::{Error, Result};
use crate::launchers;
use crate::steam;
use crate::steam::id::{shortcut_id_signed, shortcut_id_unsigned};
use crate::steam::shortcuts::{self, Shortcut, Value};
use crate::types::{
    default_steam_path, known_sources, ApplyResult, DetectResult, Game, SteamAccount,
    SyncOptions,
};

/// All art URLs for one game, returned by `fetch_art_previews` so the
/// Apply view can show a full preview of every asset before the user
/// commits.
#[derive(Debug, Clone, Serialize)]
pub struct ArtPreview {
    pub display_name: String,
    /// Canonical SGDB game name when a match was found.
    pub sgdb_name: Option<String>,
    /// Vertical box art (600×900) — shown as the cover in Steam.
    pub box_art_url: Option<String>,
    /// Hero / background image (3840×1240 or similar).
    pub hero_url: Option<String>,
    /// Transparent logo PNG.
    pub logo_url: Option<String>,
    /// Wide / big-picture grid art (920×430 or 460×215).
    pub wide_url: Option<String>,
}

/// Frontend-visible event payloads emitted during apply_changes. Listen
/// in React via `listen("apply-progress", ...)`.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "stage", rename_all = "kebab-case")]
enum ApplyEvent {
    /// We're walking each launcher's library.
    Detecting { launcher: &'static str },
    /// We're merging selections into shortcuts.vdf.
    WritingShortcuts,
    /// Per-game art download tick. `current` is 1-indexed, `total` is the
    /// total number of games we're fetching art for.
    DownloadingArt {
        game: String,
        current: usize,
        total: usize,
    },
}

/// Try to find the Steam install path from the Windows registry. Returns
/// `None` on non-Windows or when Steam isn't installed. On success, the
/// path is verified to actually exist before being returned so we don't
/// pass a stale registry value back to the UI.
#[tauri::command]
pub async fn auto_detect_steam_path() -> Option<String> {
    let candidate = registry_steam_path().or_else(common_steam_path)?;
    if PathBuf::from(&candidate).join("steam.exe").is_file()
        || PathBuf::from(&candidate).join("steam.sh").is_file()
        || PathBuf::from(&candidate).is_dir()
    {
        Some(candidate)
    } else {
        None
    }
}

#[cfg(windows)]
fn registry_steam_path() -> Option<String> {
    use winreg::enums::HKEY_LOCAL_MACHINE;
    use winreg::RegKey;

    let hklm = RegKey::predef(HKEY_LOCAL_MACHINE);
    // 64-bit Windows hides 32-bit registry entries here. Steam ships as
    // a 32-bit app even on 64-bit Windows.
    let keys = [
        "SOFTWARE\\WOW6432Node\\Valve\\Steam",
        "SOFTWARE\\Valve\\Steam",
    ];
    for key_path in keys {
        if let Ok(key) = hklm.open_subkey(key_path) {
            if let Ok(path) = key.get_value::<String, _>("InstallPath") {
                return Some(path);
            }
        }
    }
    None
}

#[cfg(not(windows))]
fn registry_steam_path() -> Option<String> {
    None
}

/// Last-resort fallback: check the conventional install paths.
fn common_steam_path() -> Option<String> {
    let candidates = if cfg!(target_os = "linux") {
        vec![
            dirs::home_dir().map(|h| h.join(".steam").join("steam"))?,
            dirs::home_dir().map(|h| h.join(".local").join("share").join("Steam"))?,
        ]
    } else {
        vec![
            PathBuf::from("C:\\Program Files (x86)\\Steam"),
            PathBuf::from("C:\\Program Files\\Steam"),
        ]
    };
    candidates
        .into_iter()
        .find(|p| p.is_dir())
        .map(|p| p.to_string_lossy().into_owned())
}

/// Fetch the SGDB box-art URL for each provided display name, in
/// parallel. Returns `box_art_url = None` when no match is found.
/// Used by the Apply view to render a thumbnail grid before the user
/// commits to writing shortcuts.
#[tauri::command]
pub async fn fetch_art_previews(
    api_key: String,
    display_names: Vec<String>,
) -> Result<Vec<ArtPreview>> {
    use futures::stream::{self, StreamExt};

    if display_names.is_empty() {
        return Ok(Vec::new());
    }
    let sgdb = SgdbClient::new(api_key)?;

    let previews: Vec<ArtPreview> = stream::iter(display_names)
        .map(|name| {
            let sgdb = &sgdb;
            async move {
                let (urls, sgdb_name) = match sgdb.find_game(&name).await {
                    Some((id, canonical)) => (Some(sgdb.art_for(id).await), Some(canonical)),
                    None => (None, None),
                };
                let (box_art_url, hero_url, logo_url, wide_url) = match urls {
                    Some(u) => {
                        let hero = u.hero.or_else(|| u.big_picture.clone());
                        let logo = u.logo.or_else(|| u.box_art.clone());
                        (u.box_art, hero, logo, u.big_picture)
                    }
                    None => (None, None, None, None),
                };
                ArtPreview { display_name: name, sgdb_name, box_art_url, hero_url, logo_url, wide_url }
            }
        })
        .buffered(8)
        .collect()
        .await;
    Ok(previews)
}

#[tauri::command]
pub async fn detect_games(opts: SyncOptions) -> Result<DetectResult> {
    let steam_path = resolve_steam_path(&opts);
    let accounts = steam::enumerate_accounts(&steam_path)?;
    let games = launchers::collect_games(&opts)?;

    // Determine which games are already in the user's Steam shortcuts.vdf.
    // We use the same exe|args key as apply_changes so the match is exact.
    let existing_app_names = {
        // Pick the account to check: explicit steamid > only account > skip.
        let sid = if !opts.steamid.is_empty() {
            Some(opts.steamid.as_str())
        } else if accounts.len() == 1 {
            Some(accounts[0].steamid.as_str())
        } else {
            None
        };

        sid.and_then(|sid| {
            let path = steam_path
                .join("userdata")
                .join(sid)
                .join("config")
                .join("shortcuts.vdf");
            let bytes = std::fs::read(&path).ok()?;
            let parsed = shortcuts::parse(&bytes).ok()?;
            let (existing, _) = shortcuts::extract_shortcuts(&parsed).ok()?;
            // Primary key: exe|args (path-based, backward compatible).
            let keys: HashSet<String> = existing
                .iter()
                .map(|(_, sc)| format!("{}|{}", unquote(&sc.exe), unquote(&sc.launch_options)))
                .collect();
            // Secondary: tag "2" (stored app_name) — catches games whose exe
            // was changed via the override picker since the last apply run.
            let tag2_set: HashSet<String> = existing
                .iter()
                .filter(|(_, sc)| sc.tags.values().any(|v| v == "steamsync"))
                .filter_map(|(_, sc)| sc.tags.get("2").cloned())
                .collect();
            let matched = games
                .iter()
                .filter(|g| {
                    if tag2_set.contains(&g.app_name) {
                        return true;
                    }
                    let (exe, args) = launch_target(g, opts.use_uri);
                    keys.contains(&format!("{exe}|{args}"))
                })
                .map(|g| g.app_name.clone())
                .collect();
            Some(matched)
        })
        .unwrap_or_default()
    };

    Ok(DetectResult {
        games,
        accounts,
        default_steam_path: default_steam_path(),
        sources: known_sources(),
        existing_app_names,
    })
}

#[tauri::command]
pub async fn apply_changes(
    window: Window,
    opts: SyncOptions,
    selected_app_names: Vec<String>,
    name_overrides: HashMap<String, String>,
    exe_overrides: HashMap<String, String>,
) -> Result<ApplyResult> {
    let emit = |event: ApplyEvent| {
        // Failure to emit shouldn't fail the apply — log and continue.
        let _ = window.emit("apply-progress", &event);
    };

    let steam_path = resolve_steam_path(&opts);

    // 1. Pick the Steam account. --steamid is required when there are
    // multiple accounts; the UI surfaces that via the Configure view.
    let accounts = steam::enumerate_accounts(&steam_path)?;
    let user = pick_account(&accounts, &opts.steamid)?;

    // 2. Collect everything (for both the selection filter and the
    // remove-missing pass, which needs the full known-games set).
    for launcher in &opts.sources {
        emit(ApplyEvent::Detecting {
            launcher: match launcher.as_str() {
                "epicstore" => "epicstore",
                "xbox" => "xbox",
                _ => "other",
            },
        });
    }
    let all_games = launchers::collect_games(&opts)?;
    let selected: HashSet<&str> = selected_app_names.iter().map(String::as_str).collect();
    // Apply name and exe overrides: clone and patch any game that has overrides.
    let selected_owned: Vec<Game> = all_games
        .iter()
        .filter(|g| selected.contains(g.app_name.as_str()))
        .map(|g| {
            let display_name = name_overrides
                .get(&g.app_name)
                .cloned()
                .unwrap_or_else(|| g.display_name.clone());
            let (executable_path, icon) = if let Some(exe) = exe_overrides.get(&g.app_name) {
                (exe.clone(), exe.clone())
            } else {
                (g.executable_path.clone(), g.icon.clone())
            };
            Game { display_name, executable_path, icon, ..g.clone() }
        })
        .collect();
    let selected_games: Vec<&Game> = selected_owned.iter().collect();

    // 3. Load existing shortcuts.vdf.
    let shortcuts_path = steam_path
        .join("userdata")
        .join(&user.steamid)
        .join("config")
        .join("shortcuts.vdf");
    let (mut existing, leftover_root) = if shortcuts_path.is_file() {
        let bytes = std::fs::read(&shortcuts_path)?;
        let parsed = shortcuts::parse(&bytes)?;
        shortcuts::extract_shortcuts(&parsed)?
    } else {
        (Vec::<(String, Shortcut)>::new(), Vec::<(String, Value)>::new())
    };

    // 4. Build two indexes over existing shortcuts.
    //
    // Primary: "{exe}|{launch_options}" — path-based dedup (backward compat).
    // Secondary: tag "2" value → index, steamsync entries only. Tag "2" stores
    // the launcher's canonical app_name so we can find a shortcut even when
    // the user has changed the exe via the override picker (which changes the
    // primary key but not the game identity).
    let mut path_to_index: HashMap<String, usize> = HashMap::new();
    let mut appname_to_index: HashMap<String, usize> = HashMap::new();
    for (i, (_, sc)) in existing.iter().enumerate() {
        // Normalise by stripping quotes — shortcuts written by Steam or by
        // older runs of this app may or may not quote paths. `launch_target`
        // always returns unquoted strings, so we match on the unquoted form.
        path_to_index.insert(format!("{}|{}", unquote(&sc.exe), unquote(&sc.launch_options)), i);
        if sc.tags.values().any(|v| v == "steamsync") {
            if let Some(stored_app_name) = sc.tags.get("2") {
                appname_to_index.insert(stored_app_name.clone(), i);
            }
        }
    }

    // 5. Compute the effective `shortcut_id_unsigned` per selected game.
    //    For new shortcuts this is CRC32(exe, app_name). For shortcuts
    //    that already exist we preserve the on-disk appid so previously
    //    downloaded grid art stays matched.
    let mut targets: Vec<ArtTarget> = Vec::with_capacity(selected_games.len());
    for game in &selected_games {
        let (exe, args) = launch_target(game, opts.use_uri);
        let key = format!("{exe}|{args}");
        let existing_idx = appname_to_index
            .get(&game.app_name)
            .or_else(|| path_to_index.get(&key));
        let id_u = if let Some(&i) = existing_idx {
            existing[i].1.appid as u32
        } else {
            shortcut_id_unsigned(&exe, &game.app_name)
        };
        targets.push(ArtTarget {
            app_name: game.app_name.clone(),
            display_name: game.display_name.clone(),
            shortcut_id_unsigned: id_u,
        });
    }

    // 6. Download art *before* writing shortcuts so each shortcut's
    //    `icon` field can point at the freshly-downloaded icon file.
    //    Without this the Steam library sidebar shows an empty box for
    //    games whose .exe Windows can't extract an icon from.
    let mut icon_paths: HashMap<String, PathBuf> = HashMap::new();
    if opts.download_art && !selected_games.is_empty() {
        let sgdb = SgdbClient::new(opts.steamgriddb_api_key.clone())?;
        let grid_folder = steam_path
            .join("userdata")
            .join(&user.steamid)
            .join("config")
            .join("grid");

        let total = targets.len();
        for (i, t) in targets.iter().enumerate() {
            emit(ApplyEvent::DownloadingArt {
                game: t.display_name.clone(),
                current: i + 1,
                total,
            });
        }

        let result = download_art_all(&sgdb, targets.clone(), grid_folder, false).await?;
        icon_paths = result.icon_paths;
    }

    // 7. Merge selected games into the shortcut list. The downloaded
    //    icon path (if any) overrides the launcher-supplied icon so the
    //    sidebar gets the high-quality SGDB image rather than an icon
    //    extracted from the exe.
    //
    // Dedup rules:
    //   • steamsync-owned shortcut found (by app_name tag OR by exe|args):
    //     always update in-place — never create a second entry.
    //   • shortcut found but NOT owned by steamsync (e.g. a game the user
    //     added to Steam manually, or a native Steam shortcut): skip entirely
    //     — don't overwrite it, don't duplicate it.
    //   • no match: add as a new entry and register both indexes so a second
    //     selected game resolving to the same exe|args cannot produce a dupe.
    let mut added: u32 = 0;
    for game in &selected_games {
        let (exe, args) = launch_target(game, opts.use_uri);
        let key = format!("{exe}|{args}");
        let icon_override = icon_paths
            .get(&game.app_name)
            .map(|p| p.to_string_lossy().into_owned());

        // Check secondary index first: handles the case where the user picked
        // a different exe via the override picker (key changes, identity doesn't).
        let existing_idx = appname_to_index
            .get(&game.app_name)
            .or_else(|| path_to_index.get(&key))
            .copied();

        if let Some(i) = existing_idx {
            let is_ours = existing[i].1.tags.values().any(|v| v == "steamsync");
            if is_ours {
                // Our shortcut — always refresh it so settings/exe/art stay current.
                let preserved_id = existing[i].1.appid;
                existing[i].1 = build_shortcut(game, opts.use_uri, Some(preserved_id), icon_override);
                added += 1;
            }
            // Not ours (added by Steam or user) — leave it untouched, no duplicate.
            continue;
        }

        // Genuinely new — add it and register in both indexes so a second
        // selected game with the same resolved exe cannot create a duplicate.
        let new_idx = existing.len();
        existing.push((
            "__placeholder__".into(),
            build_shortcut(game, opts.use_uri, None, icon_override),
        ));
        path_to_index.insert(key, new_idx);
        appname_to_index.insert(game.app_name.clone(), new_idx);
        added += 1;
    }

    // 8. Optionally remove dead shortcuts.
    let mut removed: u32 = 0;
    if opts.remove_missing {
        let known_uris: HashSet<&str> =
            all_games.iter().filter_map(|g| g.uri.as_deref()).collect();
        let before = existing.len();
        existing.retain(|(_, sc)| shortcut_is_alive(sc, &known_uris));
        removed = (before - existing.len()) as u32;
    }

    // 9. Re-index entries: Steam expects "0", "1", "2", ... contiguous.
    for (i, entry) in existing.iter_mut().enumerate() {
        entry.0 = i.to_string();
    }

    // 10. Write back if anything changed (or always, when art was
    //     refreshed on existing shortcuts — the icon-path update needs
    //     to land on disk).
    let wrote_shortcuts =
        added > 0 || removed > 0 || (!icon_paths.is_empty() && !existing.is_empty());
    if wrote_shortcuts {
        emit(ApplyEvent::WritingShortcuts);
        let root = shortcuts::build_root(&existing, &leftover_root);
        let new_bytes = shortcuts::serialize(&root);
        // The GUI always backs up — the "live dangerously" flag from the
        // Python CLI isn't exposed here on purpose.
        write_shortcuts_safely(&shortcuts_path, &new_bytes, true)?;
    }

    Ok(ApplyResult {
        added,
        removed,
        wrote_shortcuts,
        steamid: user.steamid.clone(),
        username: user.username.clone(),
    })
}

/// Kill Steam and re-launch it from the configured Steam path. Used by
/// the success screen so the user's new shortcuts appear immediately
/// without having to manually restart Steam.
#[tauri::command]
pub async fn restart_steam(steam_path: String) -> Result<()> {
    use std::time::Duration;
    #[cfg(windows)]
    use std::os::windows::process::CommandExt;
    #[cfg(windows)]
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;

    // Kill any running Steam process. Ignore errors — Steam may not be running.
    let mut kill = std::process::Command::new("taskkill");
    kill.args(["/f", "/im", "steam.exe"]);
    #[cfg(windows)]
    kill.creation_flags(CREATE_NO_WINDOW);
    let _ = kill.output();

    // Give it a moment to fully exit before re-launching.
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let exe = PathBuf::from(&steam_path).join("steam.exe");
    if exe.is_file() {
        let mut start = std::process::Command::new(&exe);
        #[cfg(windows)]
        start.creation_flags(CREATE_NO_WINDOW);
        let _ = start.spawn();
    }
    Ok(())
}

// ----------------------------------------------------------------------
// Helpers
// ----------------------------------------------------------------------

fn resolve_steam_path(opts: &SyncOptions) -> PathBuf {
    PathBuf::from(if opts.steam_path.is_empty() {
        default_steam_path()
    } else {
        opts.steam_path.clone()
    })
}

fn pick_account(accounts: &[SteamAccount], requested: &str) -> Result<SteamAccount> {
    if accounts.is_empty() {
        return Err(Error::SteamPathMissing(
            "no Steam accounts found on this machine".into(),
        ));
    }
    if requested.is_empty() {
        if accounts.len() == 1 {
            return Ok(accounts[0].clone());
        }
        return Err(Error::VdfParse(
            "multiple Steam accounts on this machine — pick one in Configure".into(),
        ));
    }
    accounts
        .iter()
        .find(|a| a.steamid == requested || a.username == requested)
        .cloned()
        .ok_or_else(|| {
            Error::VdfParse(format!(
                "Steam account '{requested}' not found on this machine"
            ))
        })
}

/// Resolve the launch target (executable path or URI) and any launch
/// arguments, matching Python `GameDefinition.get_launcher`.
fn launch_target(game: &Game, use_uri: bool) -> (String, String) {
    if game.storetag == "xbox" {
        // Xbox always launches via explorer.exe with the AUMID URI.
        let explorer = std::env::var("WINDIR")
            .map(|w| format!("{w}/explorer.exe"))
            .unwrap_or_else(|_| "C:/Windows/explorer.exe".to_string());
        return (
            explorer,
            game.uri.clone().unwrap_or_default(),
        );
    }
    if use_uri {
        if let Some(uri) = &game.uri {
            return (uri.clone(), game.launch_arguments.clone());
        }
    }
    (game.executable_path.clone(), game.launch_arguments.clone())
}

/// Wrap a filesystem path in double-quotes so Steam can parse it when
/// it contains spaces. URIs and already-quoted strings are returned as-is.
fn quote_path(s: &str) -> String {
    if s.starts_with('"') || s.contains("://") || s.to_lowercase().ends_with("explorer.exe") {
        s.to_string()
    } else {
        format!("\"{s}\"")
    }
}

/// Strip surrounding double-quotes for consistent key comparison.
/// Existing shortcuts written by Steam or other tools may quote paths;
/// ours may not have in older runs. Normalise before comparing.
fn unquote(s: &str) -> &str {
    s.trim_matches('"')
}

fn build_shortcut(
    game: &Game,
    use_uri: bool,
    preserved_appid: Option<i32>,
    icon_override: Option<String>,
) -> Shortcut {
    let (raw_exe, launch_args) = launch_target(game, use_uri);
    // The shortcut_id must be computed from the unquoted path so it is
    // stable regardless of whether we quoted on a previous run.
    let appid = preserved_appid.unwrap_or_else(|| shortcut_id_signed(&raw_exe, &game.app_name));
    let exe = quote_path(&raw_exe);
    let start_dir = quote_path(&game.install_folder);
    let icon = icon_override.unwrap_or_else(|| game.icon.clone());

    use std::collections::BTreeMap;
    let mut tags = BTreeMap::new();
    tags.insert("0".into(), "steamsync".into());
    tags.insert("1".into(), game.storetag.clone());
    // Tag "2" stores the launcher's canonical app_name so apply_changes can
    // find this shortcut even when the exe path changes (e.g. user picks a
    // different executable via the override picker between runs).
    tags.insert("2".into(), game.app_name.clone());

    Shortcut {
        appid,
        app_name: game.display_name.clone(),
        exe,
        start_dir,
        icon,
        shortcut_path: String::new(),
        launch_options: launch_args,
        is_hidden: 0,
        allow_desktop_config: 1,
        allow_overlay: 1,
        openvr: 0,
        devkit: 0,
        devkit_game_id: String::new(),
        last_play_time: 0,
        tags,
        extra: Vec::new(),
    }
}

/// Decide whether a shortcut still points at something real.
fn shortcut_is_alive(sc: &Shortcut, known_uris: &HashSet<&str>) -> bool {
    if sc.exe.is_empty() {
        // Defensive: keep anything we don't understand rather than
        // deleting it.
        return true;
    }

    let is_uri = sc.exe.contains("://") || sc.exe.to_lowercase().ends_with("explorer.exe");
    if is_uri {
        // For URI-launched shortcuts (EGS / Xbox), only delete when we
        // know they're not in any current launcher's library.
        return known_uris.contains(sc.exe.as_str())
            || known_uris.contains(sc.launch_options.as_str());
    }
    let path = PathBuf::from(&sc.exe);
    if path.is_file() {
        return true;
    }
    // Manually added shortcuts may have extra quotes.
    let unquoted = PathBuf::from(sc.exe.trim_matches('"'));
    unquoted.is_file()
}

/// Backup + atomic write. Same safety guarantees as the Python fix:
/// copy old → .bak before touching anything, write new to .tmp, then
/// rename .tmp over the target so a crash mid-write can't truncate the
/// user's library.
fn write_shortcuts_safely(
    target: &std::path::Path,
    new_bytes: &[u8],
    backup: bool,
) -> Result<()> {
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if backup && target.is_file() {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let backup_path = target.with_extension(format!("vdf-{stamp}.bak"));
        std::fs::copy(target, &backup_path)?;
    }
    let tmp = target.with_extension("vdf.tmp");
    std::fs::write(&tmp, new_bytes)?;
    std::fs::rename(&tmp, target)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pick_account_returns_single_when_empty_requested() {
        let accs = vec![SteamAccount {
            steamid: "1".into(),
            username: "u".into(),
        }];
        let picked = pick_account(&accs, "").unwrap();
        assert_eq!(picked.steamid, "1");
    }

    #[test]
    fn pick_account_requires_steamid_when_multiple() {
        let accs = vec![
            SteamAccount {
                steamid: "1".into(),
                username: "a".into(),
            },
            SteamAccount {
                steamid: "2".into(),
                username: "b".into(),
            },
        ];
        let err = pick_account(&accs, "").unwrap_err();
        assert!(matches!(err, Error::VdfParse(_)));
    }

    #[test]
    fn pick_account_finds_by_username_or_steamid() {
        let accs = vec![
            SteamAccount {
                steamid: "1".into(),
                username: "alice".into(),
            },
            SteamAccount {
                steamid: "2".into(),
                username: "bob".into(),
            },
        ];
        assert_eq!(pick_account(&accs, "2").unwrap().username, "bob");
        assert_eq!(pick_account(&accs, "alice").unwrap().steamid, "1");
    }

    #[test]
    fn pick_account_rejects_unknown() {
        let accs = vec![SteamAccount {
            steamid: "1".into(),
            username: "u".into(),
        }];
        assert!(pick_account(&accs, "nobody").is_err());
    }

    #[test]
    fn shortcut_is_alive_keeps_uris_in_known_set() {
        let mut sc = Shortcut::from_value(&Value::Object(vec![
            (
                "Exe".into(),
                Value::String("com.epicgames.launcher://apps/x?action=launch".into()),
            ),
            ("LaunchOptions".into(), Value::String(String::new())),
        ]))
        .unwrap();
        let mut known: HashSet<&str> = HashSet::new();
        known.insert("com.epicgames.launcher://apps/x?action=launch");
        assert!(shortcut_is_alive(&sc, &known));
        // Drop it from known → should be reaped.
        sc.exe = "com.epicgames.launcher://apps/x?action=launch".into();
        let empty: HashSet<&str> = HashSet::new();
        assert!(!shortcut_is_alive(&sc, &empty));
    }

    #[test]
    fn shortcut_is_alive_falls_back_to_filesystem() {
        let tmp = tempfile::NamedTempFile::new().unwrap();
        let sc = Shortcut::from_value(&Value::Object(vec![(
            "Exe".into(),
            Value::String(tmp.path().to_string_lossy().into_owned()),
        )]))
        .unwrap();
        let known: HashSet<&str> = HashSet::new();
        assert!(shortcut_is_alive(&sc, &known));
    }
}
