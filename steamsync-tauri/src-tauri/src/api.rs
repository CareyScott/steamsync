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

use std::path::{Path, PathBuf};
use std::time::Duration;

use futures::stream::{self, StreamExt};
use serde::Deserialize;

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
    pub display_name: String,
    pub shortcut_id_unsigned: u32,
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

    /// Fetch the four art URLs for one SGDB game id. Any individual
    /// endpoint may legitimately return zero results (not every game
    /// has every art type); those become `None`.
    pub async fn art_for(&self, sgdb_id: u32) -> ArtUrls {
        let grid_vertical = format!("{SGDB_BASE}/grids/game/{sgdb_id}?dimensions=600x900");
        let grid_wide = format!("{SGDB_BASE}/grids/game/{sgdb_id}?dimensions=920x430,460x215");
        let heroes = format!("{SGDB_BASE}/heroes/game/{sgdb_id}");
        let logos = format!("{SGDB_BASE}/logos/game/{sgdb_id}");
        let (box_art, hero, logo, big_picture) = futures::join!(
            self.first_url(&grid_vertical),
            self.first_url(&heroes),
            self.first_url(&logos),
            self.first_url(&grid_wide),
        );
        ArtUrls {
            box_art,
            hero,
            logo,
            big_picture,
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

/// The four art URLs we'd write for a single game. Each is independently
/// optional — SGDB has good coverage but not every game has every type.
pub struct ArtUrls {
    pub box_art: Option<String>,
    pub hero: Option<String>,
    pub logo: Option<String>,
    pub big_picture: Option<String>,
}

/// Download art for every target in parallel via SGDB. Returns the
/// number of files actually written (existing files aren't re-downloaded
/// unless `replace_existing`).
pub async fn download_art_all(
    sgdb: &SgdbClient,
    targets: Vec<ArtTarget>,
    grid_folder: PathBuf,
    replace_existing: bool,
) -> Result<usize> {
    tokio::fs::create_dir_all(&grid_folder)
        .await
        .map_err(Error::Io)?;

    // For each target: lookup → resolve URLs → download each. We pipe
    // the whole thing through buffer_unordered so we get up to
    // ART_PARALLELISM games-in-flight without serializing on the slowest
    // network call.
    let total: usize = stream::iter(targets)
        .map(|target| {
            let grid = grid_folder.clone();
            async move {
                let Some(sgdb_id) = sgdb.find_game_id(&target.display_name).await else {
                    return 0;
                };
                let urls = sgdb.art_for(sgdb_id).await;
                download_one_game(sgdb, &target, urls, &grid, replace_existing).await
            }
        })
        .buffer_unordered(ART_PARALLELISM)
        .fold(0usize, |acc, written| async move { acc + written })
        .await;

    Ok(total)
}

async fn download_one_game(
    sgdb: &SgdbClient,
    target: &ArtTarget,
    urls: ArtUrls,
    grid_folder: &Path,
    replace_existing: bool,
) -> usize {
    let id = target.shortcut_id_unsigned;
    let plan = [
        (urls.box_art, grid_folder.join(format!("{id}p.jpg"))),
        (urls.hero, grid_folder.join(format!("{id}_hero.jpg"))),
        (urls.logo, grid_folder.join(format!("{id}_logo.png"))),
        (
            urls.big_picture,
            grid_folder.join(format!("{id}_bigpicture.png")),
        ),
    ];

    let mut written = 0;
    for (maybe_url, dest_base) in plan {
        let Some(url) = maybe_url else { continue };
        // SGDB serves images at the URL's native extension; we keep our
        // Steam-expected suffix but use the source extension if it's a
        // known image type, so the file is well-formed.
        let dest = with_source_extension(&dest_base, &url);
        if !replace_existing && dest.is_file() {
            continue;
        }
        if download_to(&sgdb.client, &url, &dest).await {
            written += 1;
        }
    }
    written
}

/// Steam doesn't actually care about the extension on grid art files —
/// it sniffs the content. So we keep our convention (`p.jpg`,
/// `_logo.png`, etc.) which is what Python's steamsync writes, even if
/// the upstream URL is a different format. Means SGDB's `.webp` files
/// land at `_hero.jpg` etc., but Steam happily renders them.
fn with_source_extension(dest_base: &Path, _url: &str) -> PathBuf {
    dest_base.to_path_buf()
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
