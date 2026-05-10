//! Launcher scrapers (Phase 3 — not yet implemented).
//!
//! Each launcher will live in its own submodule:
//!   - egs       — Epic Games Store (parse .item JSON manifests)
//!   - itch      — itch.io app (parse receipt.json.gz + .itch.toml)
//!   - xbox      — Microsoft Store (shell PowerShell, parse XML manifests)
//!   - legendary — legendary CLI (subprocess wrapper)
//!
//! For now `collect_games` always returns an empty list so the Detect view
//! still renders Steam accounts.

use crate::error::Result;
use crate::types::{Game, SyncOptions};

pub fn collect_games(_opts: &SyncOptions) -> Result<Vec<Game>> {
    Ok(Vec::new())
}
