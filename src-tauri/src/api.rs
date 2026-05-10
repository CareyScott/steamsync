// Phase 5 wires this into the apply path; suppress until then.
#![allow(dead_code)]

//! SteamGridDB-backed art downloader.
//!
//! We replaced the original Steam CDN approach because the community-
//! maintained art at <https://www.steamgriddb.com> is dramatically
//! higher quality than Steam's auto-generated headers, especially for
//! the smaller Epic / Xbox catalog this app targets.
//!
//! Per game we do:
//!
//! 1. **Find** the SGDB game id by searching `display_name` against the
//!    autocomplete endpoint (handles fuzzy name matches natively, so
//!    we can drop the regex cascade the Python version needed).
//! 2. **Resolve** four art URLs in parallel — vertical grid (600x900),
//!    horizontal grid for big-picture (920x430 / 460x215), hero, logo.
//! 3. **Download** each URL into the user's `userdata/<id>/config/grid/`
//!    folder, named so Steam picks them up:
//!    - `<shortcut_id>p.jpg`         vertical box
//!    - `<shortcut_id>_hero.jpg`     hero banner
//!    - `<shortcut_id>_logo.png`     transparent logo
//!    - `<shortcut_id>_bigpicture.png`  big-picture banner
//!
//! Across all selected games this is done at up to `ART_PARALLELISM`
//! requests in flight, so a hundred-game library finishes in seconds.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;
use tokio::sync::Mutex;

use crate::error::{Error, Result};

const SGDB_BASE: &str = "https://www.steamgriddb.com/api/v2";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const ART_PARALLELISM: usize = 8;

/// Authenticated SteamGridDB client. The API key is mandatory — every
/// endpoint requires a Bearer token. Users get one for free at
/// <https://www.steamgriddb.com/profile/preferences/api>.
pub struct SgdbClient {
    client: reqwest::Client,
    api_key: String,
}

#[derive(Debug, Clone)]
pub struct ArtTarget {
    /// Stable per-game identifier (the launcher's `app_name`). Used as
    /// the key when returning the per-game icon paths so the caller can
    /// look them up while building shortcuts.
    pub app_name: String,
    pub display_name: String,
    pub shortcut_id_unsigned: u32,
}

/// What `download_art_all` returned. `count_written` is purely
/// informational; `icon_paths` is the map the apply path uses to set
/// each shortcut's `icon` field to the freshly-downloaded image.
#[derive(Debug, Default)]
pub struct ArtResult {
    pub count_written: usize,
    pub icon_paths: HashMap<String, PathBuf>,
}

#[derive(Debug, Deserialize)]
struct SgdbList<T> {
    #[serde(default)]
    success: bool,
    #[serde(default = "Vec::new")]
    data: Vec<T>,
}

#[derive(Debug, Deserialize)]
struct SgdbSearchHit {
    id: u32,
    #[serde(default)]
    #[allow(dead_code)]
    name: String,
}

#[derive(Debug, Deserialize)]
struct SgdbArt {
    #[serde(default)]
    #[allow(dead_code)]
    id: u32,
    url: String,
}

impl SgdbClient {
    pub fn new(api_key: String) -> Result<Self> {
        if api_key.trim().is_empty() {
            return Err(Error::VdfParse(
                "SteamGridDB API key is required for art download. \
                 Get one from https://www.steamgriddb.com/profile/preferences/api"
                    .into(),
            ));
        }
        let client = reqwest::Client::builder()
            .timeout(HTTP_TIMEOUT)
            .build()
            .map_err(|e| Error::VdfParse(format!("reqwest client: {e}")))?;
        Ok(Self { client, api_key })
    }

    /// Best-effort lookup of a display name → SGDB game id. Returns
    /// `None` if the API errors or there are no results.
    pub async fn find_game_id(&self, display_name: &str) -> Option<u32> {
        let url = format!(
            "{SGDB_BASE}/search/autocomplete/{}",
            urlencoding::encode(display_name)
        );
        let list: SgdbList<SgdbSearchHit> = self.get_json(&url).await.ok()?;
        list.data.into_iter().next().map(|h| h.id)
    }

    /// Fetch the five art URLs for one SGDB game id. Any individual
    /// endpoint may legitimately return zero results (not every game
    /// has every art type); those become `None`.
    ///
    /// `big_picture` falls back to the hero URL when SGDB has no
    /// dedicated wide grid — otherwise Steam shows an empty grey
    /// placeholder in its "Wide Cover" slot. Using the hero looks
    /// stretched but is consistent with the rest of the art set.
    pub async fn art_for(&self, sgdb_id: u32) -> ArtUrls {
        let grid_vertical = format!("{SGDB_BASE}/grids/game/{sgdb_id}?dimensions=600x900");
        let grid_wide =
            format!("{SGDB_BASE}/grids/game/{sgdb_id}?dimensions=920x430,460x215");
        let heroes = format!("{SGDB_BASE}/heroes/game/{sgdb_id}");
        let logos = format!("{SGDB_BASE}/logos/game/{sgdb_id}");
        let icons = format!("{SGDB_BASE}/icons/game/{sgdb_id}");
        let (box_art, hero, logo, wide_grid, icon) = futures::join!(
            self.first_url(&grid_vertical),
            self.first_url(&heroes),
            self.first_url(&logos),
            self.first_url(&grid_wide),
            self.first_url(&icons),
        );
        let big_picture = wide_grid.or_else(|| hero.clone());
        ArtUrls {
            box_art,
            hero,
            logo,
            big_picture,
            icon,
        }
    }

    async fn first_url(&self, endpoint: &str) -> Option<String> {
        let list: SgdbList<SgdbArt> = self.get_json(endpoint).await.ok()?;
        list.data.into_iter().next().map(|a| a.url)
    }

    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let resp = self
            .client
            .get(url)
            .bearer_auth(&self.api_key)
            .send()
            .await
            .map_err(|e| Error::VdfParse(format!("SGDB GET {url}: {e}")))?;
        if !resp.status().is_success() {
            return Err(Error::VdfParse(format!(
                "SGDB {url} returned HTTP {}",
                resp.status()
            )));
        }
        resp.json()
            .await
            .map_err(|e| Error::VdfParse(format!("SGDB JSON decode {url}: {e}")))
    }
}

/// The five art URLs we'd write for a single game. Each is independently
/// optional — SGDB has good coverage but not every game has every type.
pub struct ArtUrls {
    pub box_art: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub big_picture: Option<String>,
    /// Small sidebar icon. Lands at `<id>_icon.<ext>` and is then
    /// pointed at by the shortcut's `icon` field so the Steam library
    /// sidebar has a thumbnail instead of an empty box.
    pub icon: Option<String>,
}

/// Download art for every target in parallel via SGDB. Returns the
/// number of files actually written, plus a map of `app_name → icon
/// path` for the caller to bake into each shortcut's `icon` field.
///
/// Existing files aren't re-downloaded unless `replace_existing`.
pub async fn download_art_all(
    sgdb: &SgdbClient,
    targets: Vec<ArtTarget>,
    grid_folder: PathBuf,
    replace_existing: bool,
) -> Result<ArtResult> {
    tokio::fs::create_dir_all(&grid_folder)
        .await
        .map_err(Error::Io)?;

    let icon_paths: Mutex<HashMap<String, PathBuf>> = Mutex::new(HashMap::new());

    let total: usize = stream::iter(targets)
        .map(|target| {
            let grid = grid_folder.clone();
            let icon_paths = &icon_paths;
            async move {
                let Some(sgdb_id) = sgdb.find_game_id(&target.display_name).await else {
                    return 0;
                };
                let urls = sgdb.art_for(sgdb_id).await;
                let (written, icon_path) =
                    download_one_game(sgdb, &target, urls, &grid, replace_existing).await;
                if let Some(p) = icon_path {
                    icon_paths.lock().await.insert(target.app_name.clone(), p);
                }
                written
            }
        })
        .buffer_unordered(ART_PARALLELISM)
        .fold(0usize, |acc, written| async move { acc + written })
        .await;

    Ok(ArtResult {
        count_written: total,
        icon_paths: icon_paths.into_inner(),
    })
}

/// Returns `(files_written, icon_path_or_none)`.
async fn download_one_game(
    sgdb: &SgdbClient,
    target: &ArtTarget,
    urls: ArtUrls,
    grid_folder: &Path,
    replace_existing: bool,
) -> (usize, Option<PathBuf>) {
    let id = target.shortcut_id_unsigned;

    // Steam's four grid art slots. Filenames are conventional (matching
    // Python steamsync) — Steam content-sniffs anyway so the extension
    // doesn't have to match the actual format.
    let grid_plan = [
        (urls.box_art, grid_folder.join(format!("{id}p.jpg"))),
        (urls.hero, grid_folder.join(format!("{id}_hero.jpg"))),
        (urls.logo, grid_folder.join(format!("{id}_logo.png"))),
        (
            urls.big_picture,
            grid_folder.join(format!("{id}_bigpicture.png")),
        ),
    ];

    let mut written = 0;
    for (maybe_url, dest) in grid_plan {
        let Some(url) = maybe_url else { continue };
        if !replace_existing && dest.is_file() {
            continue;
        }
        if download_to(&sgdb.client, &url, &dest).await {
            written += 1;
        }
    }

    // Icon is special: the file path goes into `shortcut.icon`, so we
    // honor the source extension (Steam reads icons more strictly than
    // grid art — .ico vs .png matters for some renderers).
    let mut icon_path: Option<PathBuf> = None;
    if let Some(url) = urls.icon {
        let ext = url_extension(&url).unwrap_or("png");
        let dest = grid_folder.join(format!("{id}_icon.{ext}"));
        if replace_existing || !dest.is_file() {
            if download_to(&sgdb.client, &url, &dest).await {
                written += 1;
                icon_path = Some(dest);
            }
        } else {
            // Already on disk from a previous run — still surface the
            // path so the shortcut's icon field gets pointed at it.
            icon_path = Some(dest);
        }
    }

    (written, icon_path)
}

/// Extract a likely image extension from a URL (e.g. `https://.../foo.png?v=2`
/// → `png`). Returns `None` if the URL doesn't end in something that
/// looks like an image extension.
fn url_extension(url: &str) -> Option<&str> {
    let cleaned = url.split(['?', '#']).next().unwrap_or(url);
    let ext = cleaned.rsplit('.').next()?;
    matches!(
        ext.to_ascii_lowercase().as_str(),
        "png" | "jpg" | "jpeg" | "ico" | "webp" | "gif"
    )
    .then_some(ext)
}

async fn download_to(client: &reqwest::Client, url: &str, dest: &Path) -> bool {
    let resp = match client.get(url).send().await {
        Ok(r) if r.status().is_success() => r,
        _ => return false,
    };
    let bytes = match resp.bytes().await {
        Ok(b) => b,
        Err(_) => return false,
    };
    tokio::fs::write(dest, &bytes).await.is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_rejects_empty_key() {
        assert!(SgdbClient::new(String::new()).is_err());
    }

    #[test]
    fn new_rejects_whitespace_key() {
        assert!(SgdbClient::new("   ".into()).is_err());
    }

    #[test]
    fn new_accepts_real_key() {
        // Build via match so we don't need Debug on SgdbClient itself.
        match SgdbClient::new("a-real-key".into()) {
            Ok(c) => assert_eq!(c.api_key, "a-real-key"),
            Err(e) => panic!("expected Ok, got Err({e})"),
        }
    }
}
