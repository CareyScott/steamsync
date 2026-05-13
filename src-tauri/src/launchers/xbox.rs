//! Xbox / Microsoft Store launcher scraper. Port of
//! steamsync/steamsync/launchers/xbox.py.
//!
//! There's no clean WinRT API we can hit without a much bigger dep, so
//! we keep the upstream PowerShell approach: invoke `powershell.exe`
//! with an embedded script that calls `Get-AppxPackage`, parse the
//! resulting JSON, then read each app's `MicrosoftGame.config` (or
//! `AppxManifest.xml` for older titles) to get the executable name and
//! display name.
//!
//! Launching by exe path doesn't reliably work for Xbox games because
//! the install path includes a version-stamped folder name that
//! changes. Instead we launch via `explorer.exe shell:appsFolder\<AUMID>`
//! — see [`Game::uri`].

use std::path::{Path, PathBuf};
use std::process::Command;

#[cfg(windows)]
use std::os::windows::process::CommandExt;

use quick_xml::events::Event;
use quick_xml::reader::Reader;
use serde::Deserialize;

use crate::error::{Error, Result};
use crate::types::Game;

const PS_SCRIPT: &str = include_str!("../../scripts/list_xbox_games.ps1");

// Suppresses the console window that would otherwise flash whenever a
// windowed release build spawns a console subprocess like powershell.exe.
#[cfg(windows)]
const CREATE_NO_WINDOW: u32 = 0x0800_0000;

/// One row from the PowerShell script's `ConvertTo-Json` output.
///
/// All fields are `Option<String>` because PowerShell serialises missing or
/// null manifest attributes as JSON `null`, which cannot be deserialised into
/// a plain `String`. Fields that are essential for a usable shortcut (Aumid,
/// InstallLocation) cause the entry to be skipped when absent; the rest fall
/// back to empty strings.
#[derive(Debug, Deserialize)]
#[allow(non_snake_case)]
struct PsApp {
    #[serde(default)]
    Kind: Option<String>,
    #[serde(default)]
    Aumid: Option<String>,
    #[serde(default)]
    PrettyName: Option<String>,
    #[serde(default)]
    Icon: Option<String>,
    #[serde(default)]
    InstallLocation: Option<String>,
}

pub fn collect() -> Result<Vec<Game>> {
    let mut cmd = Command::new("powershell.exe");
    cmd.args(["-NoProfile", "-NonInteractive", "-Command", PS_SCRIPT]);
    #[cfg(windows)]
    cmd.creation_flags(CREATE_NO_WINDOW);

    let output = match cmd.output() {
        Ok(o) => o,
        Err(e) => {
            return Err(Error::VdfParse(format!(
                "could not run powershell.exe: {e}"
            )))
        }
    };

    if !output.status.success() {
        return Err(Error::VdfParse(format!(
            "powershell exited {}: {}",
            output.status,
            String::from_utf8_lossy(&output.stderr).trim()
        )));
    }

    // PowerShell -Command emits no output if the array is empty. Treat that
    // as "no Xbox games".
    let stdout = String::from_utf8_lossy(&output.stdout);
    let trimmed = stdout.trim();
    if trimmed.is_empty() {
        return Ok(Vec::new());
    }

    // ConvertTo-Json emits a single object (not an array) when there's
    // exactly one entry. Normalize to a Vec in both shapes.
    let raw: serde_json::Value = serde_json::from_str(trimmed)
        .map_err(|e| Error::VdfParse(format!("invalid powershell JSON: {e}")))?;
    let apps: Vec<PsApp> = match raw {
        serde_json::Value::Array(_) => serde_json::from_value(raw)
            .map_err(|e| Error::VdfParse(format!("expected PsApp array: {e}")))?,
        serde_json::Value::Object(_) => vec![serde_json::from_value(raw)
            .map_err(|e| Error::VdfParse(format!("expected PsApp object: {e}")))?],
        _ => return Ok(Vec::new()),
    };

    let mut games = Vec::new();
    for app in apps {
        if let Some(g) = app_to_game(app) {
            games.push(g);
        }
    }
    games.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(games)
}

fn app_to_game(app: PsApp) -> Option<Game> {
    // Without these two fields we can't build a usable shortcut.
    let aumid = app.Aumid?;
    let install_location = app.InstallLocation?;
    let pretty_name = app.PrettyName.unwrap_or_default();
    let icon_str = app.Icon.unwrap_or_default();
    let kind = app.Kind.unwrap_or_default();

    let install = PathBuf::from(&install_location);
    let config = install.join("MicrosoftGame.config");
    let (exe_name, display_name) = if config.is_file() {
        read_microsoft_game_config(&config).unwrap_or((None, pretty_name.clone()))
    } else {
        let manifest = install.join("AppxManifest.xml");
        let is_kind_game = kind == "Game";
        let is_game = is_kind_game
            || (manifest.is_file()
                && is_game_judging_by_manifest(&manifest).unwrap_or(false));
        if !is_game {
            return None;
        }
        // Older games: launch by AUMID, no exe path needed.
        (None, pretty_name.clone())
    };

    // If we have an exe path, validate it exists.
    let (exe, working_dir) = if let Some(name) = exe_name {
        let exe_path = install.join(&name);
        if !exe_path.is_file() {
            return None;
        }
        // Python uses `exe.parent.anchor` as a minimal valid path —
        // we never launch via this since Xbox needs `shell:appsFolder\AUMID`.
        let working_dir = exe_path
            .ancestors()
            .last()
            .map(|p| p.to_string_lossy().into_owned())
            .unwrap_or_default();
        (exe_path.to_string_lossy().into_owned(), working_dir)
    } else {
        // Minimal valid path since we won't actually launch via exe.
        (String::new(), "/".to_string())
    };

    // Pick a usable icon: prefer the one the script gave us, fall back to
    // the targetsize-48 variant (Spiritfarer), then the exe.
    let icon_path = PathBuf::from(&icon_str);
    let icon = if icon_path.is_file() {
        icon_str.clone()
    } else {
        let ext = icon_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        let alt = icon_path.with_file_name(format!(
            "{}.targetsize-48.{ext}",
            icon_path
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
        ));
        if alt.is_file() {
            alt.to_string_lossy().into_owned()
        } else if !exe.is_empty() {
            exe.clone()
        } else {
            icon_str
        }
    };

    let uri = format!("shell:appsFolder\\{aumid}");

    Some(Game {
        app_name: aumid,
        display_name,
        executable_path: exe,
        install_folder: working_dir,
        launch_arguments: String::new(),
        icon,
        uri: Some(uri),
        storetag: "xbox".into(),
        shortcut_id: None,
        exe_candidates: Vec::new(),
    })
}

/// Read `<Executable Name="..."/>` and `<ShellVisuals DefaultDisplayName="..."/>`
/// from a `MicrosoftGame.config`. Returns `(exe_name, display_name)`.
fn read_microsoft_game_config(path: &Path) -> Result<(Option<String>, String)> {
    let mut reader = Reader::from_file(path)
        .map_err(|e| Error::VdfParse(format!("failed to open {}: {e}", path.display())))?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut exe: Option<String> = None;
    let mut display: Option<String> = None;
    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Empty(e)) | Ok(Event::Start(e)) => {
                let name = e.name();
                let local = name.as_ref();
                if local == b"Executable" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            exe = Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                    }
                } else if local == b"ShellVisuals" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"DefaultDisplayName" {
                            display =
                                Some(attr.unescape_value().unwrap_or_default().into_owned());
                        }
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(e) => {
                return Err(Error::VdfParse(format!(
                    "{}: XML parse error at pos {}: {e}",
                    path.display(),
                    reader.buffer_position()
                )))
            }
            _ => {}
        }
        buf.clear();
    }
    Ok((exe, display.unwrap_or_default()))
}

/// Heuristic copy of `_is_game_judging_by_manifest` from xbox.py: look
/// at `AppxManifest.xml` and try to decide whether the app is a game.
fn is_game_judging_by_manifest(path: &Path) -> Result<bool> {
    let mut reader = Reader::from_file(path)
        .map_err(|e| Error::VdfParse(format!("failed to open {}: {e}", path.display())))?;
    reader.config_mut().trim_text(true);

    let mut buf = Vec::new();
    let mut current_tag: Vec<u8> = Vec::new();
    let mut visual_has_xbox = false;
    let mut is_desktop = false;
    let mut uses_game_dll = false;
    let mut uses_unity = false;

    loop {
        match reader.read_event_into(&mut buf) {
            Ok(Event::Start(e)) | Ok(Event::Empty(e)) => {
                current_tag = e.name().as_ref().to_vec();
                let name = e.name();
                let local = name.as_ref();
                if local.ends_with(b"VisualElements") {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"DisplayName" {
                            let v = attr.unescape_value().unwrap_or_default();
                            if v.to_ascii_lowercase().contains("xbox") {
                                visual_has_xbox = true;
                            }
                        }
                    }
                } else if local == b"TargetDeviceFamily" {
                    for attr in e.attributes().flatten() {
                        if attr.key.as_ref() == b"Name" {
                            let v = attr.unescape_value().unwrap_or_default();
                            if v.eq_ignore_ascii_case("windows.desktop") {
                                is_desktop = true;
                            }
                        }
                    }
                }
            }
            Ok(Event::Text(t)) => {
                if current_tag == b"Path" {
                    let s = t.unescape().unwrap_or_default();
                    if s == "Microsoft.Xbox.Services.dll" {
                        uses_game_dll = true;
                    } else if s == "UnityPlayer.dll" {
                        uses_unity = true;
                    }
                }
            }
            Ok(Event::Eof) => break,
            Err(_) => return Ok(false),
            _ => {}
        }
        buf.clear();
    }

    if visual_has_xbox {
        // App tile contains "Xbox" — likely the Xbox app itself, not a game.
        return Ok(false);
    }
    if !is_desktop {
        return Ok(false);
    }
    Ok(uses_game_dll || uses_unity)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    #[test]
    fn parses_microsoft_game_config() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("MicrosoftGame.config");
        fs::write(
            &path,
            r#"<?xml version="1.0"?>
            <Game>
                <ShellVisuals DefaultDisplayName="Tetris® Effect: Connected"/>
                <Executables>
                    <Executable Name="Tetris.exe"/>
                </Executables>
            </Game>"#,
        )
        .unwrap();
        let (exe, name) = read_microsoft_game_config(&path).unwrap();
        assert_eq!(exe.as_deref(), Some("Tetris.exe"));
        assert_eq!(name, "Tetris® Effect: Connected");
    }

    #[test]
    fn microsoft_game_config_without_executable_returns_none_exe() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("MicrosoftGame.config");
        fs::write(
            &path,
            r#"<Game>
                <ShellVisuals DefaultDisplayName="No Exe Game"/>
            </Game>"#,
        )
        .unwrap();
        let (exe, name) = read_microsoft_game_config(&path).unwrap();
        assert_eq!(exe, None);
        assert_eq!(name, "No Exe Game");
    }

    #[test]
    fn manifest_with_xbox_dll_and_desktop_family_is_a_game() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AppxManifest.xml");
        fs::write(
            &path,
            r#"<Package>
                <Applications>
                    <Application>
                        <uap:VisualElements DisplayName="My Cool Game"/>
                    </Application>
                </Applications>
                <Dependencies>
                    <TargetDeviceFamily Name="Windows.Desktop"/>
                </Dependencies>
                <Resources>
                    <Path>Microsoft.Xbox.Services.dll</Path>
                </Resources>
            </Package>"#,
        )
        .unwrap();
        assert!(is_game_judging_by_manifest(&path).unwrap());
    }

    #[test]
    fn manifest_for_unity_desktop_app_is_a_game() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AppxManifest.xml");
        fs::write(
            &path,
            r#"<Package>
                <Applications>
                    <Application>
                        <uap:VisualElements DisplayName="Indie Game"/>
                    </Application>
                </Applications>
                <Dependencies>
                    <TargetDeviceFamily Name="Windows.Desktop"/>
                </Dependencies>
                <Resources>
                    <Path>UnityPlayer.dll</Path>
                </Resources>
            </Package>"#,
        )
        .unwrap();
        assert!(is_game_judging_by_manifest(&path).unwrap());
    }

    #[test]
    fn manifest_for_xbox_app_itself_is_not_a_game() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AppxManifest.xml");
        fs::write(
            &path,
            r#"<Package>
                <Applications>
                    <Application>
                        <uap:VisualElements DisplayName="Xbox Console Companion"/>
                    </Application>
                </Applications>
                <Dependencies>
                    <TargetDeviceFamily Name="Windows.Desktop"/>
                </Dependencies>
                <Resources>
                    <Path>UnityPlayer.dll</Path>
                </Resources>
            </Package>"#,
        )
        .unwrap();
        assert!(!is_game_judging_by_manifest(&path).unwrap());
    }

    #[test]
    fn manifest_for_mobile_only_app_is_not_a_game() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("AppxManifest.xml");
        fs::write(
            &path,
            r#"<Package>
                <Dependencies>
                    <TargetDeviceFamily Name="Windows.Mobile"/>
                </Dependencies>
                <Resources>
                    <Path>UnityPlayer.dll</Path>
                </Resources>
            </Package>"#,
        )
        .unwrap();
        assert!(!is_game_judging_by_manifest(&path).unwrap());
    }
}
