use serde::{Deserialize, Serialize};

/// One game discovered in a launcher's library. Mirrors the TS shape in
/// `src/types.ts` exactly so the frontend can render the result without
/// further transformation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Game {
    pub app_name: String,
    pub display_name: String,
    pub executable_path: String,
    pub install_folder: String,
    pub launch_arguments: String,
    pub icon: String,
    pub uri: Option<String>,
    pub storetag: String,
    pub shortcut_id: Option<i64>,
    /// Alternative executables the user can pick from (local games only).
    /// Sorted largest-first; first entry is the recommended default.
    #[serde(default)]
    pub exe_candidates: Vec<String>,
}

/// A Steam account discovered on this machine.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SteamAccount {
    pub steamid: String,
    pub username: String,
}

/// Result of running the Detect phase.
#[derive(Debug, Clone, Serialize)]
pub struct DetectResult {
    pub games: Vec<Game>,
    pub accounts: Vec<SteamAccount>,
    pub default_steam_path: String,
    pub sources: Vec<String>,
    /// `app_name` values of games already present in the Steam library as
    /// shortcuts. Empty when no Steam account can be determined yet.
    pub existing_app_names: Vec<String>,
}

/// Result of running the Apply phase.
#[derive(Debug, Clone, Serialize)]
pub struct ApplyResult {
    pub added: u32,
    pub removed: u32,
    pub wrote_shortcuts: bool,
    pub steamid: String,
    pub username: String,
}

/// Inputs the user controls in the UI, sent to both detect_games and
/// apply_changes commands. Most fields are consumed once the Phase 3
/// launchers and Phase 5 apply path land.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "snake_case")]
#[allow(dead_code)]
pub struct SyncOptions {
    pub steamid: String,
    pub sources: Vec<String>,
    pub use_uri: bool,
    pub replace_existing: bool,
    pub remove_missing: bool,
    pub download_art: bool,
    pub egs_manifests: String,
    pub steam_path: String,
    /// SteamGridDB API key (https://www.steamgriddb.com). Required when
    /// download_art is true.
    pub steamgriddb_api_key: String,
    /// Root folders to scan for local games (one subfolder per game).
    #[serde(default)]
    pub local_folders: Vec<String>,
}

/// Sources the Tauri app supports. Narrower than the Python CLI on
/// purpose — itch.io and legendary remain available only via the
/// standalone CLI for headless users.
pub fn known_sources() -> Vec<String> {
    vec!["epicstore".to_string(), "xbox".to_string(), "local".to_string()]
}

/// Default steam install path for the current platform.
pub fn default_steam_path() -> String {
    if cfg!(target_os = "linux") {
        if let Some(home) = dirs::home_dir() {
            return home.join(".steam").join("steam").to_string_lossy().into_owned();
        }
    }
    "C:\\Program Files (x86)\\Steam".to_string()
}
