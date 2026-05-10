//! Launcher scrapers.
//!
//! Each launcher knows how to enumerate the games installed by its
//! storefront and turn each into a `Game`. The dispatcher below
//! invokes only the launchers the user has enabled via `SyncOptions::sources`.
//!
//! Per-launcher failures don't abort the whole detection run — we log
//! to stderr and skip, matching the Python behavior. Otherwise a single
//! misconfigured launcher would hide every game the user owns elsewhere.

pub mod egs;
pub mod xbox;

use std::path::Path;

use crate::error::Result;
use crate::types::{Game, SyncOptions};

const SRC_EPIC: &str = "epicstore";
const SRC_XBOX: &str = "xbox";

pub fn collect_games(opts: &SyncOptions) -> Result<Vec<Game>> {
    let mut games = Vec::new();

    if opts.sources.iter().any(|s| s == SRC_EPIC) {
        match egs::collect(Path::new(&opts.egs_manifests)) {
            Ok(g) => games.extend(g),
            Err(e) => eprintln!("EGS scan failed: {e}"),
        }
    }

    if opts.sources.iter().any(|s| s == SRC_XBOX) {
        match xbox::collect() {
            Ok(g) => games.extend(g),
            Err(e) => eprintln!("Xbox scan failed: {e}"),
        }
    }

    games.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(games)
}
