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

/// One game's box-art URL, used to show a thumbnail grid before the
/// user commits to writing shortcuts.
#[derive(Debug, Clone, Serialize)]
pub struct ArtPreview {
    pub display_name: String,
    pub box_art_url: Option<String>,
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
                let box_art_url = match sgdb.find_game_id(&name).await {
                    Some(id) => sgdb.art_for(id).await.box_art,
                    None => None,
                };
                ArtPreview {
                    display_name: name,
                    box_art_url,
                }
            }
        })
        .buffer_unordered(8)
        .collect()
        .await;
    Ok(previews)
}

#[tauri::command]
pub async fn detect_games(opts: SyncOptions) -> Result<DetectResult> {
    let steam_path = resolve_steam_path(&opts);
    let accounts = steam::enumerate_accounts(&steam_path)?;
    let games = launchers::collect_games(&opts)?;
    Ok(DetectResult {
        games,
        accounts,
        default_steam_path: default_steam_path(),
        sources: known_sources(),
    })
}

#[tauri::command]
pub async fn apply_changes(
    window: Window,
    opts: SyncOptions,
    selected_app_names: Vec<String>,
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
    let selected_games: Vec<&Game> = all_games
        .iter()
        .filter(|g| selected.contains(g.app_name.as_str()))
        .collect();

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

    // 4. Index existing shortcuts by "{exe}|{launch_options}" — the same
    // key the Python implementation uses to dedupe.
    let mut path_to_index: HashMap<String, usize> = HashMap::new();
    for (i, (_, sc)) in existing.iter().enumerate() {
        path_to_index.insert(format!("{}|{}", sc.exe, sc.launch_options), i);
    }

    // 5. Compute the effective `shortcut_id_unsigned` per selected game.
    //    For new shortcuts this is CRC32(exe, app_name). For shortcuts
    //    that already exist we preserve the on-disk appid so previously
    //    downloaded grid art stays matched.
    let mut targets: Vec<ArtTarget> = Vec::with_capacity(selected_games.len());
    for game in &selected_games {
        let (exe, args) = launch_target(game, opts.use_uri);
        let key = format!("{exe}|{args}");
        let id_u = if let Some(&i) = path_to_index.get(&key) {
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
    let mut added: u32 = 0;
    for game in &selected_games {
        let (exe, args) = launch_target(game, opts.use_uri);
        let key = format!("{exe}|{args}");
        let icon_override = icon_paths
            .get(&game.app_name)
            .map(|p| p.to_string_lossy().into_owned());

        if let Some(&i) = path_to_index.get(&key) {
            // Already present — replace only if explicitly requested.
            // We *do* always refresh the icon path when one was just
            // downloaded, even outside replace_existing, since that's
            // a pure improvement with no risk of dropping user state.
            if opts.replace_existing {
                let preserved_id = existing[i].1.appid;
                existing[i].1 = build_shortcut(
                    game,
                    opts.use_uri,
                    Some(preserved_id),
                    icon_override,
                );
                added += 1;
            } else if let Some(icon) = icon_override {
                existing[i].1.icon = icon;
            }
            continue;
        }
        existing.push((
            "__placeholder__".into(),
            build_shortcut(game, opts.use_uri, None, icon_override),
        ));
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

fn build_shortcut(
    game: &Game,
    use_uri: bool,
    preserved_appid: Option<i32>,
    icon_override: Option<String>,
) -> Shortcut {
    let (exe, launch_args) = launch_target(game, use_uri);
    let appid = preserved_appid.unwrap_or_else(|| shortcut_id_signed(&exe, &game.app_name));
    let icon = icon_override.unwrap_or_else(|| game.icon.clone());

    use std::collections::BTreeMap;
    let mut tags = BTreeMap::new();
    tags.insert("0".into(), "steamsync".into());
    tags.insert("1".into(), game.storetag.clone());

    Shortcut {
        appid,
        app_name: game.display_name.clone(),
        exe,
        start_dir: game.install_folder.clone(),
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
