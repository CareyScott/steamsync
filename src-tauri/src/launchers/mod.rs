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
pub mod local_folders;
pub mod xbox;

use std::path::Path;

use crate::error::Result;
use crate::types::{Game, SyncOptions};

const SRC_EPIC: &str = "epicstore";
const SRC_XBOX: &str = "xbox";
const SRC_LOCAL: &str = "local";

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

    if opts.sources.iter().any(|s| s == SRC_LOCAL) {
        match local_folders::collect(&opts.local_folders) {
            Ok(g) => games.extend(g),
            Err(e) => eprintln!("Local folders scan failed: {e}"),
        }
    }

    // Drop local games that are duplicates of EGS/Xbox entries.
    //
    // Simple exe-path equality misses the common case where EGS's
    // InstallLocation is D:\Games\Fortnite but the local scanner descended
    // one level deeper and found D:\Games\Fortnite\Fortnite\FortniteGame\...
    // Instead we check containment: if a local game's exe path starts with
    // any non-local install folder, it belongs to that installation.
    //
    // Build lowercase install-folder prefixes from EGS + Xbox results.
    // A trailing separator prevents "D:\Games\Fortnite\" from matching
    // "D:\Games\FortniteOther\...".
    let non_local_roots: Vec<String> = games
        .iter()
        .filter(|g| {
            g.storetag != "local"
                && !g.install_folder.is_empty()
                && g.install_folder != "/"
        })
        .map(|g| {
            let mut p = g.install_folder.to_ascii_lowercase().replace('/', "\\");
            if !p.ends_with('\\') {
                p.push('\\');
            }
            p
        })
        .collect();

    if !non_local_roots.is_empty() {
        games.retain(|g| {
            if g.storetag != "local" {
                return true;
            }
            let exe = g.executable_path.to_ascii_lowercase().replace('/', "\\");
            !non_local_roots.iter().any(|root| exe.starts_with(root.as_str()))
        });
    }

    games.sort_by(|a, b| a.display_name.cmp(&b.display_name));
    Ok(games)
}
