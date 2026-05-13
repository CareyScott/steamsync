//! Epic Games Store launcher scraper. Port of
//! steamsync/steamsync/launchers/egs.py.
//!
//! Reads every `*.item` JSON file under EGS's Manifests directory and
//! turns each one into a [`Game`]. The default path on Windows is
//! `C:\ProgramData\Epic\EpicGamesLauncher\Data\Manifests`.
//!
//! Filtering rules match upstream Python:
//! - skip if `bIsIncompleteInstall`
//! - skip if not `bIsApplication`
//! - skip unless `AppCategories` contains `"games"`
//! - skip if `InstallLocation` or `LaunchExecutable` is missing
//! - skip if the resolved executable path doesn't exist on disk

use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::error::Result;
use crate::types::Game;

/// Just the fields steamsync looks at. EGS writes many more — serde
/// ignores unknown ones by default, so the file format can evolve
/// without breaking us.
#[derive(Debug, Default, Deserialize)]
#[allow(non_snake_case)]
struct EgsItem {
    #[serde(default)]
    AppName: Option<String>,
    #[serde(default)]
    DisplayName: Option<String>,
    #[serde(default)]
    InstallLocation: Option<String>,
    #[serde(default)]
    LaunchExecutable: Option<String>,
    #[serde(default)]
    LaunchCommand: Option<String>,
    #[serde(default)]
    bIsIncompleteInstall: bool,
    #[serde(default)]
    bIsApplication: bool,
    #[serde(default)]
    AppCategories: Vec<String>,
}

pub fn collect(manifest_path: &Path) -> Result<Vec<Game>> {
    let mut games = Vec::new();
    if !manifest_path.is_dir() {
        return Ok(games);
    }
    for entry in fs::read_dir(manifest_path)? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("item") {
            continue;
        }
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(_) => continue,
        };
        let item: EgsItem = match serde_json::from_slice(&bytes) {
            Ok(i) => i,
            Err(_) => continue,
        };
        if let Some(game) = item_to_game(item) {
            games.push(game);
        }
    }
    games.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(games)
}

fn item_to_game(item: EgsItem) -> Option<Game> {
    if item.bIsIncompleteInstall || !item.bIsApplication {
        return None;
    }
    if !item.AppCategories.iter().any(|c| c == "games") {
        return None;
    }

    let install_location = item.InstallLocation?;
    let raw_launch_exe = item.LaunchExecutable?;
    let app_name = item.AppName?;
    let display_name = item.DisplayName.unwrap_or_else(|| app_name.clone());

    // Sanitize paths that look absolute but aren't (e.g. RiME's
    // "/RiME/SirenGame/Binaries/Win64/RiME.exe" — leading slash but the
    // path is actually relative to InstallLocation).
    let launch_exe = raw_launch_exe.trim_start_matches(['/', '\\']);
    let exe_path = PathBuf::from(&install_location).join(launch_exe);
    if !exe_path.is_file() {
        return None;
    }

    let exe_str = exe_path.to_string_lossy().into_owned();
    let uri = format!(
        "com.epicgames.launcher://apps/{app_name}?action=launch&silent=true"
    );

    Some(Game {
        app_name,
        display_name,
        executable_path: exe_str.clone(),
        install_folder: install_location,
        launch_arguments: item.LaunchCommand.unwrap_or_default(),
        icon: exe_str,
        uri: Some(uri),
        storetag: "epicstore".into(),
        shortcut_id: None,
        exe_candidates: Vec::new(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::io::Write;
    use tempfile::TempDir;

    /// Make a manifests dir containing an item JSON and a touched exe.
    fn build_fixture(dir: &Path, exe_relative: &str) -> PathBuf {
        let install = dir.join("Installed_Game");
        fs::create_dir_all(&install).unwrap();
        let exe_full = install.join(exe_relative);
        if let Some(parent) = exe_full.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&exe_full, b"fake exe").unwrap();
        install
    }

    fn write_item(manifests: &Path, name: &str, body: &str) {
        fs::create_dir_all(manifests).unwrap();
        let mut f = fs::File::create(manifests.join(format!("{name}.item"))).unwrap();
        f.write_all(body.as_bytes()).unwrap();
    }

    #[test]
    fn returns_empty_for_missing_path() {
        let games = collect(Path::new("Z:\\definitely\\not\\here")).unwrap();
        assert!(games.is_empty());
    }

    #[test]
    fn parses_one_complete_game() {
        let tmp = TempDir::new().unwrap();
        let install = build_fixture(tmp.path(), "Game.exe");
        let manifests = tmp.path().join("Manifests");
        let body = format!(
            r#"{{
                "AppName": "Fortnite",
                "DisplayName": "Fortnite",
                "InstallLocation": "{}",
                "LaunchExecutable": "Game.exe",
                "LaunchCommand": "",
                "bIsIncompleteInstall": false,
                "bIsApplication": true,
                "AppCategories": ["games", "public"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "fortnite", &body);

        let games = collect(&manifests).unwrap();
        assert_eq!(games.len(), 1);
        let g = &games[0];
        assert_eq!(g.app_name, "Fortnite");
        assert_eq!(g.display_name, "Fortnite");
        assert_eq!(g.storetag, "epicstore");
        assert!(g.executable_path.ends_with("Game.exe"));
        assert!(g.uri.as_ref().unwrap().contains("com.epicgames.launcher"));
    }

    #[test]
    fn skips_incomplete_installs() {
        let tmp = TempDir::new().unwrap();
        let install = build_fixture(tmp.path(), "Game.exe");
        let manifests = tmp.path().join("Manifests");
        let body = format!(
            r#"{{
                "AppName": "X", "DisplayName": "X",
                "InstallLocation": "{}",
                "LaunchExecutable": "Game.exe",
                "bIsIncompleteInstall": true,
                "bIsApplication": true,
                "AppCategories": ["games"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "x", &body);
        assert!(collect(&manifests).unwrap().is_empty());
    }

    #[test]
    fn skips_non_game_categories() {
        let tmp = TempDir::new().unwrap();
        let install = build_fixture(tmp.path(), "Tool.exe");
        let manifests = tmp.path().join("Manifests");
        let body = format!(
            r#"{{
                "AppName": "Tool", "DisplayName": "Tool",
                "InstallLocation": "{}",
                "LaunchExecutable": "Tool.exe",
                "bIsApplication": true,
                "AppCategories": ["applications", "public"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "tool", &body);
        assert!(collect(&manifests).unwrap().is_empty());
    }

    #[test]
    fn skips_when_exe_missing_on_disk() {
        let tmp = TempDir::new().unwrap();
        // Don't create the exe.
        let install = tmp.path().join("Installed_Game");
        fs::create_dir_all(&install).unwrap();
        let manifests = tmp.path().join("Manifests");
        let body = format!(
            r#"{{
                "AppName": "Phantom", "DisplayName": "Phantom",
                "InstallLocation": "{}",
                "LaunchExecutable": "Missing.exe",
                "bIsApplication": true,
                "AppCategories": ["games"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "phantom", &body);
        assert!(collect(&manifests).unwrap().is_empty());
    }

    /// Reproduces the RiME case from upstream: LaunchExecutable starts
    /// with `/` but the path is actually relative to InstallLocation.
    #[test]
    fn strips_leading_slash_from_launch_executable() {
        let tmp = TempDir::new().unwrap();
        let install = build_fixture(tmp.path(), "RiME.exe");
        let manifests = tmp.path().join("Manifests");
        let body = format!(
            r#"{{
                "AppName": "RiME", "DisplayName": "RiME",
                "InstallLocation": "{}",
                "LaunchExecutable": "/RiME.exe",
                "bIsApplication": true,
                "AppCategories": ["games"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "rime", &body);
        let games = collect(&manifests).unwrap();
        assert_eq!(games.len(), 1);
        assert!(games[0].executable_path.ends_with("RiME.exe"));
    }

    #[test]
    fn malformed_json_is_skipped_not_fatal() {
        let tmp = TempDir::new().unwrap();
        let manifests = tmp.path().join("Manifests");
        write_item(&manifests, "broken", "{not json");
        // Also add a valid one so we can assert it's still returned.
        let install = build_fixture(tmp.path(), "Game.exe");
        let body = format!(
            r#"{{
                "AppName": "OK", "DisplayName": "OK",
                "InstallLocation": "{}",
                "LaunchExecutable": "Game.exe",
                "bIsApplication": true,
                "AppCategories": ["games"]
            }}"#,
            install.to_string_lossy().replace('\\', "\\\\"),
        );
        write_item(&manifests, "ok", &body);

        let games = collect(&manifests).unwrap();
        assert_eq!(games.len(), 1);
        assert_eq!(games[0].app_name, "OK");
    }
}
