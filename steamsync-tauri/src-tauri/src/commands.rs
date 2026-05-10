//! Tauri command handlers invoked from the React frontend via `invoke()`.
//!
//! These intentionally do almost no business logic — they sequence calls
//! into the `steam` and `launchers` modules and shape results into the
//! types defined in `types.rs`.

use std::path::PathBuf;

use crate::error::{Error, Result};
use crate::launchers;
use crate::steam;
use crate::types::{
    default_steam_path, known_sources, ApplyResult, DetectResult, SyncOptions,
};

#[tauri::command]
pub async fn detect_games(opts: SyncOptions) -> Result<DetectResult> {
    let steam_path = PathBuf::from(if opts.steam_path.is_empty() {
        default_steam_path()
    } else {
        opts.steam_path.clone()
    });

    // Account enumeration is the only thing that requires the steam path
    // exist. If it's missing, surface that clearly — the frontend already
    // renders the error string from Error.
    let accounts = steam::enumerate_accounts(&steam_path)?;

    // Phase 1: launchers always return empty. Phase 3 fills this in.
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
    _opts: SyncOptions,
    _selected_app_names: Vec<String>,
) -> Result<ApplyResult> {
    // The binary shortcuts.vdf codec lands in Phase 2 — until then refuse
    // rather than risk corrupting a real Steam library.
    Err(Error::NotYetImplemented)
}
