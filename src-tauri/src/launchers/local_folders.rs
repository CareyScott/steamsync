//! Local game folder scanner.
//!
//! For each user-configured root path, scans subfolders up to two levels
//! deep. A folder whose top level contains at least one non-directory entry
//! is treated as a game folder; the largest non-helper `.exe` found within
//! it (recursively) becomes the shortcut target.
//!
//! If a level-1 subfolder contains no top-level files it is treated as a
//! category/organizational folder and its own direct children (level 2) are
//! checked as game folders instead.

use std::fs;
use std::path::{Path, PathBuf};

use crate::error::Result;
use crate::types::Game;

pub fn collect(root_paths: &[String]) -> Result<Vec<Game>> {
    let mut games = Vec::new();
    for root in root_paths {
        let root_path = Path::new(root);
        if !root_path.is_dir() {
            eprintln!("Local folder not found, skipping: {root}");
            continue;
        }
        scan_root(root_path, &mut games);
    }
    Ok(games)
}

fn scan_root(root: &Path, out: &mut Vec<Game>) {
    let entries = match fs::read_dir(root) {
        Ok(d) => d,
        Err(e) => {
            eprintln!("Cannot read {:?}: {e}", root);
            return;
        }
    };

    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        if has_top_level_files(&path) {
            // Looks like a game folder (has files at its root).
            if let Some(game) = folder_to_game(&path) {
                out.push(game);
            }
        } else {
            // Looks like an organizational folder — check one level deeper.
            let sub_entries = match fs::read_dir(&path) {
                Ok(d) => d,
                Err(_) => continue,
            };
            for sub in sub_entries.flatten() {
                let sub_path = sub.path();
                if !sub_path.is_dir() {
                    continue;
                }
                if let Some(game) = folder_to_game(&sub_path) {
                    out.push(game);
                }
            }
        }
    }
}

/// Returns `true` when `dir` contains at least one non-directory entry at
/// its immediate root. Organizational folders (pure containers of other
/// folders) return `false`.
fn has_top_level_files(dir: &Path) -> bool {
    fs::read_dir(dir)
        .map(|entries| entries.flatten().any(|e| e.path().is_file()))
        .unwrap_or(false)
}

/// Try to build a `Game` from a folder. Returns `None` if no suitable
/// `.exe` is found within the folder, or if the folder is a known
/// game-client launcher rather than an actual game.
fn folder_to_game(folder: &Path) -> Option<Game> {
    let raw_name = folder.file_name()?.to_string_lossy().into_owned();
    if is_known_client_folder(&raw_name) {
        return None;
    }
    let candidates = collect_exes_in(folder, 4);
    if candidates.is_empty() {
        return None;
    }
    let preferred = pick_preferred_exe(&candidates);
    let display_name = prettify_name(&raw_name);
    let exe_str = preferred.to_string_lossy().into_owned();
    let install_folder = folder.to_string_lossy().into_owned();
    let exe_candidates: Vec<String> = candidates
        .iter()
        .map(|p| p.to_string_lossy().into_owned())
        .collect();

    Some(Game {
        // Use the exe path as the canonical identifier so the shortcut ID
        // is stable even if the user renames the game folder.
        app_name: exe_str.clone(),
        display_name,
        executable_path: exe_str.clone(),
        install_folder,
        launch_arguments: String::new(),
        icon: exe_str,
        uri: None,
        storetag: "local".into(),
        shortcut_id: None,
        exe_candidates,
    })
}

/// Walk `dir` recursively up to `depth` levels and return all qualifying
/// game exes, sorted largest-first.
fn collect_exes_in(dir: &Path, depth: u32) -> Vec<PathBuf> {
    let mut exes: Vec<(u64, PathBuf)> = Vec::new();
    gather_exes(dir, depth, &mut exes);
    exes.sort_by(|a, b| b.0.cmp(&a.0));
    exes.into_iter().map(|(_, p)| p).collect()
}

fn gather_exes(dir: &Path, depth: u32, out: &mut Vec<(u64, PathBuf)>) {
    if depth == 0 {
        return;
    }
    let Ok(entries) = fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            gather_exes(&path, depth - 1, out);
        } else if is_game_exe(&path) {
            let size = fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
            out.push((size, path));
        }
    }
}

/// From a list of candidates (largest-first), prefer any whose stem
/// contains "launcher" (case-insensitive). Falls back to the first entry.
fn pick_preferred_exe(candidates: &[PathBuf]) -> &PathBuf {
    candidates
        .iter()
        .find(|p| {
            p.file_stem()
                .and_then(|s| s.to_str())
                .is_some_and(|s| s.to_ascii_lowercase().contains("launcher"))
        })
        .unwrap_or(&candidates[0])
}

/// Returns `true` if `path` looks like a game binary rather than an
/// installer, redistributable, crash handler, or launcher client.
fn is_game_exe(path: &Path) -> bool {
    if path.extension().and_then(|e| e.to_str()) != Some("exe") {
        return false;
    }

    // Skip exes that live inside known non-game subdirectories.
    // This prevents picking up e.g. Rockstar launcher from a game's
    // Redistributables\ folder as the "main" executable.
    for component in path.components() {
        let s = component.as_os_str().to_string_lossy().to_ascii_lowercase();
        if matches!(
            s.as_str(),
            "redistributables"
                | "_redistributables"
                | "redist"
                | "_commonredist"
                | "installers"
                | "installer"
                | "prerequisites"
                | "directx"
                | "dotnet"
        ) {
            return false;
        }
    }

    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    !stem.starts_with("uninstall")
        && !stem.starts_with("setup")
        && !stem.contains("redist")
        && !stem.contains("crashhandler")
        && !stem.contains("crash_handler")
        && !matches!(
            stem.as_str(),
            "dxsetup"
                | "easyanticheat_setup"
                | "battleye_installer"
                | "riotclientservices"
                | "epicgameslauncher"
                | "galaxyclient"
                | "galaxybackgroundservice"
                | "eadesktop"
                | "upc"
                | "ubisoftconnect"
        )
}

/// Returns `true` for folder names that belong to game launcher clients
/// rather than actual games. Case-insensitive.
fn is_known_client_folder(name: &str) -> bool {
    matches!(
        name.to_ascii_lowercase().as_str(),
        "riot client"
            | "epic games launcher"
            | "gog galaxy"
            | "battle.net"
            | "battlenet"
            | "origin"
            | "ea desktop"
            | "ea app"
            | "ubisoft connect"
            | "uplay"
            | "rockstar games launcher"
            | "bethesda.net launcher"
            | "bethesda launcher"
            | "amazon games"
            | "humble app"
            | "itch"
    )
}

/// Attempt to turn a raw folder name into a readable display name.
///
/// Handles the two most common compressed-name patterns:
///   - CamelCase:      "TombRaider"    → "Tomb Raider"
///   - AllCaps prefix: "GTAVEnhanced"  → "GTAV Enhanced"
///   - Digit boundary: "Tycoon3"       → "Tycoon 3"
///
/// Names that already contain spaces are returned unchanged.
fn prettify_name(name: &str) -> String {
    if name.contains(' ') {
        return name.to_string();
    }
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::with_capacity(name.len() + 8);
    for (i, &c) in chars.iter().enumerate() {
        if i > 0 {
            let prev = chars[i - 1];
            let next = chars.get(i + 1).copied();
            let split = (prev.is_ascii_lowercase() && c.is_ascii_uppercase())
                || (prev.is_ascii_uppercase()
                    && c.is_ascii_uppercase()
                    && next.is_some_and(|n| n.is_ascii_lowercase()))
                || (prev.is_alphabetic() && c.is_ascii_digit())
                || (prev.is_ascii_digit() && c.is_alphabetic());
            if split {
                out.push(' ');
            }
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn touch(path: &Path) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, b"fake exe").unwrap();
    }

    fn touch_sized(path: &Path, size: usize) {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(path, vec![0u8; size]).unwrap();
    }

    #[test]
    fn returns_empty_for_missing_root() {
        let result = collect(&["Z:\\does\\not\\exist".to_string()]).unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn finds_simple_game_folder() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("Cyberpunk 2077");
        touch(&game_dir.join("Cyberpunk2077.exe"));

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].display_name, "Cyberpunk 2077");
        assert_eq!(games[0].storetag, "local");
        assert!(games[0].executable_path.ends_with("Cyberpunk2077.exe"));
    }

    #[test]
    fn finds_exe_in_subdirectory() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("The Witcher 3");
        // Game has a config file at the root and an exe in a subdirectory.
        touch(&game_dir.join("config.ini"));
        touch(&game_dir.join("bin").join("x64").join("witcher3.exe"));

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].display_name, "The Witcher 3");
        assert!(games[0].executable_path.ends_with("witcher3.exe"));
    }

    #[test]
    fn picks_largest_exe() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("MyGame");
        touch_sized(&game_dir.join("small_helper.exe"), 100);
        touch_sized(&game_dir.join("MyGame.exe"), 10_000);
        touch_sized(&game_dir.join("setup.exe"), 500); // skipped by filter

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert!(games[0].executable_path.ends_with("MyGame.exe"));
    }

    #[test]
    fn skips_non_game_executables() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("Game");
        // Only non-game exes — should yield no game.
        touch_sized(&game_dir.join("uninstall.exe"), 9999);
        touch_sized(&game_dir.join("setup.exe"), 9998);
        touch_sized(&game_dir.join("vcredist_x64.exe"), 9997);

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn descends_into_category_folder() {
        let tmp = TempDir::new().unwrap();
        // RPGs is a category folder (no top-level files, only subfolders).
        let rpgs = tmp.path().join("RPGs");
        let bg3 = rpgs.join("Baldurs Gate 3");
        touch(&bg3.join("bg3.exe"));

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].display_name, "Baldurs Gate 3");
    }

    #[test]
    fn does_not_skip_folder_with_top_level_files_even_if_exe_is_deep() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("Deep Game");
        // A config file at the root makes it look like a game folder.
        touch(&game_dir.join("readme.txt"));
        touch(&game_dir.join("bin").join("win64").join("DeepGame.exe"));

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].display_name, "Deep Game");
    }

    #[test]
    fn skips_known_client_folders() {
        let tmp = TempDir::new().unwrap();
        touch(&tmp.path().join("Riot Client").join("RiotClientServices.exe"));
        touch(&tmp.path().join("GOG Galaxy").join("GalaxyClient.exe"));
        // A real game alongside the clients should still appear.
        touch(&tmp.path().join("Cyberpunk 2077").join("Cyberpunk2077.exe"));

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].display_name, "Cyberpunk 2077");
    }

    #[test]
    fn skips_exe_inside_redistributables_dir() {
        let tmp = TempDir::new().unwrap();
        let game_dir = tmp.path().join("MyGame");
        // The real game exe is small; a large installer sits in Redistributables.
        touch_sized(&game_dir.join("MyGame.exe"), 1_000);
        touch_sized(
            &game_dir.join("Redistributables").join("BigInstaller.exe"),
            9_999,
        );

        let games = collect(&[tmp.path().to_string_lossy().into_owned()]).unwrap();
        assert_eq!(games.len(), 1);
        assert!(games[0].executable_path.ends_with("MyGame.exe"));
    }

    #[test]
    fn prettify_camel_case() {
        assert_eq!(prettify_name("TombRaider"), "Tomb Raider");
        assert_eq!(prettify_name("RollerCoasterTycoon3"), "Roller Coaster Tycoon 3");
        assert_eq!(prettify_name("GTAVEnhanced"), "GTAV Enhanced");
        assert_eq!(prettify_name("TotalWarROMEII"), "Total War ROMEII");
        assert_eq!(prettify_name("VALORANT"), "VALORANT");
    }

    #[test]
    fn prettify_leaves_spaced_names_unchanged() {
        assert_eq!(prettify_name("Call of Duty Modern Warfare"), "Call of Duty Modern Warfare");
        assert_eq!(prettify_name("Riot Client"), "Riot Client");
    }

    #[test]
    fn multiple_roots() {
        let tmp1 = TempDir::new().unwrap();
        let tmp2 = TempDir::new().unwrap();
        touch(&tmp1.path().join("GameA").join("a.exe"));
        touch(&tmp2.path().join("GameB").join("b.exe"));

        let games = collect(&[
            tmp1.path().to_string_lossy().into_owned(),
            tmp2.path().to_string_lossy().into_owned(),
        ])
        .unwrap();
        assert_eq!(games.len(), 2);
    }
}
