// Phase 5 wires this into the apply path; suppress until then.
#![allow(dead_code)]

//! Steam catalog + grid art downloader.
//!
//! Two responsibilities:
//!
//! 1. **Catalog** — fetch `IStoreService/GetAppList/v1` with proper
//!    pagination via `last_appid` (the Python implementation hardcodes
//!    `max_results=50000` and silently truncates the catalog at that
//!    cutoff — fixed here). Cached to disk for `CATALOG_TTL` so we don't
//!    re-fetch on every run.
//!
//! 2. **Art** — download the four grid images Steam shows for a non-
//!    Steam shortcut (vertical box, hero, logo, big-picture banner) into
//!    `<steam_path>/userdata/<id>/config/grid/`. Parallelized across
//!    games via `futures::stream::buffer_unordered` so a 100-game
//!    library finishes in seconds rather than minutes.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures::stream::{self, StreamExt};
use regex::Regex;
use serde::{Deserialize, Serialize};

use crate::error::{Error, Result};

const STEAM_CDN: &str = "https://steamcdn-a.akamaihd.net/steam/apps";
const CATALOG_TTL: Duration = Duration::from_secs(7 * 24 * 60 * 60); // 7 days
const APPLIST_URL: &str = "https://api.steampowered.com/IStoreService/GetAppList/v1/";
const HTTP_TIMEOUT: Duration = Duration::from_secs(30);
const ART_PARALLELISM: usize = 8;

/// Cache schema. Bumped whenever the on-disk shape changes.
const CACHE_VERSION: u32 = 1;

#[derive(Debug, Serialize, Deserialize)]
struct CachedCatalog {
    version: u32,
    download_timestamp: u64, // seconds since UNIX_EPOCH
    apps: Vec<(u32, String)>,
}

/// In-memory name→appid index. Built from the cache or a fresh fetch.
pub struct Catalog {
    /// Comparable name (lowercased, common prefixes stripped) → appid.
    name_to_id: HashMap<String, u32>,
    /// Looser name→appid index for fallback matches (no accents,
    /// no punctuation, etc.). Populated alongside name_to_id.
    stripped_to_id: HashMap<String, u32>,
}

impl Catalog {
    /// Load the cached catalog if it's still fresh; otherwise fetch a
    /// new one and rewrite the cache.
    pub async fn load_or_fetch(steam_api_key: &str, cache_dir: &Path) -> Result<Catalog> {
        let cache_path = cache_dir.join("applist.json");

        if let Ok(c) = read_cache(&cache_path) {
            if c.version == CACHE_VERSION && !cache_is_stale(c.download_timestamp) {
                return Ok(Catalog::from_apps(c.apps));
            }
        }

        let apps = fetch_full_catalog(steam_api_key).await?;
        let _ = write_cache(&cache_path, &apps);
        Ok(Catalog::from_apps(apps))
    }

    fn from_apps(apps: Vec<(u32, String)>) -> Self {
        let mut name_to_id: HashMap<String, u32> = HashMap::with_capacity(apps.len());
        let mut stripped_to_id: HashMap<String, u32> = HashMap::with_capacity(apps.len() / 2);

        for (appid, name) in apps {
            if name.is_empty() {
                continue;
            }
            let comparable = make_comparable(&name);
            name_to_id.entry(comparable.clone()).or_insert(appid);

            // Skip noise — Python excludes trials/demos from the stripped
            // index so a real game doesn't get knocked out by its demo.
            if comparable.contains(" trial") || comparable.contains(" demo") {
                continue;
            }
            let no_accents = remove_accents(&comparable);
            stripped_to_id.entry(no_accents).or_insert(appid);
            let no_punct = remove_punctuation(&comparable);
            stripped_to_id.entry(no_punct).or_insert(appid);
            let no_subtitle = remove_subtitle_re().replace(&comparable, "").into_owned();
            stripped_to_id.entry(no_subtitle).or_insert(appid);
        }

        Catalog {
            name_to_id,
            stripped_to_id,
        }
    }

    /// Best-effort lookup of a display name → Steam appid.
    ///
    /// Tries the comparable name first, then a cascade of fallbacks
    /// matching Python `SteamDatabase.guess_appid`. Returns `None` if
    /// no match found.
    pub fn guess_appid(&self, display_name: &str) -> Option<u32> {
        let name = make_comparable(display_name);

        // Direct hit.
        if let Some(id) = self.name_to_id.get(&name) {
            // Hardcoded swap: Python prefers Prey 2017 (480490) over Prey
            // 2006 (3970) since the newer one has more grid art.
            if *id == 3970 {
                return Some(480490);
            }
            return Some(*id);
        }

        // "Control" → "Control Ultimate Edition"
        for suffix in [" ultimate edition", " digital edition", " steam edition"] {
            let extended = format!("{name}{suffix}");
            if let Some(id) = self.name_to_id.get(&extended) {
                return Some(*id);
            }
        }

        // "Death's Door Win10" → "Death's Door"
        let stripped = remove_win10_re().replace(&name, "").into_owned();
        if let Some(id) = self.name_to_id.get(&stripped) {
            return Some(*id);
        }

        // "Yakuza Kiwami (PC)" → "Yakuza Kiwami"
        let stripped = remove_braces_re().replace(&name, "").into_owned();
        if let Some(id) = self.name_to_id.get(&stripped) {
            return Some(*id);
        }

        // "Ghost of a Tale PC" / "Genesis Noir for Windows" → root
        let stripped = remove_pc_re().replace(&name, "").into_owned();
        if let Some(id) = self.name_to_id.get(&stripped) {
            return Some(*id);
        }

        // "Grand Theft Auto V: Premium Edition" → "Grand Theft Auto V"
        let stripped = remove_subtitle_re().replace(&name, "").into_owned();
        if let Some(id) = self.stripped_to_id.get(&stripped) {
            return Some(*id);
        }

        // "Raji: An Ancient Epic" → "Raji An Ancient Epic" (punctuation)
        let stripped = remove_punctuation(&name);
        if let Some(id) = self
            .name_to_id
            .get(&stripped)
            .or_else(|| self.stripped_to_id.get(&stripped))
        {
            return Some(*id);
        }

        // "ABZÛ" → "ABZU" (accent fold)
        let stripped = remove_accents(&name);
        if let Some(id) = self
            .name_to_id
            .get(&stripped)
            .or_else(|| self.stripped_to_id.get(&stripped))
        {
            return Some(*id);
        }

        // "Rocket League®" → "Rocket League" (drop non-ASCII)
        let stripped = strip_nonascii(&name);
        if let Some(id) = self.name_to_id.get(&stripped) {
            return Some(*id);
        }

        None
    }
}

/// Make a display name easier to compare across stores. Lowercases and
/// removes the hyphen-space sequence many stores insert ("Half-Life 2:
/// Lost Coast" vs "Half Life 2: Lost Coast").
fn make_comparable(name: &str) -> String {
    let no_hyphen = name.replace("- ", "");
    no_hyphen.to_lowercase()
}

fn remove_accents(s: &str) -> String {
    // Map common Latin-1 supplement accents back to ASCII. Good enough
    // for the cases steamsync hit historically (ABZÛ, café, etc.) without
    // pulling in a full unicode-normalization crate.
    s.chars()
        .filter_map(|c| match c {
            'à' | 'á' | 'â' | 'ã' | 'ä' | 'å' => Some('a'),
            'è' | 'é' | 'ê' | 'ë' => Some('e'),
            'ì' | 'í' | 'î' | 'ï' => Some('i'),
            'ò' | 'ó' | 'ô' | 'õ' | 'ö' => Some('o'),
            'ù' | 'ú' | 'û' | 'ü' => Some('u'),
            'ñ' => Some('n'),
            'ç' => Some('c'),
            c if c.is_ascii() => Some(c),
            // Drop other non-ASCII for the stripped index.
            _ => Some(c),
        })
        .collect()
}

fn remove_punctuation(s: &str) -> String {
    s.chars()
        .filter(|c| !matches!(*c, ':' | ';' | ',' | '.' | '=' | '+' | '?'))
        .collect()
}

fn strip_nonascii(s: &str) -> String {
    s.chars().filter(|c| c.is_ascii()).collect::<String>().trim().to_string()
}

fn remove_subtitle_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\s*:.*").unwrap())
}

fn remove_braces_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"\s*\(.*\)").unwrap())
}

fn remove_pc_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r"[ _](pc|for windows|windows)$").unwrap())
}

fn remove_win10_re() -> &'static Regex {
    static R: OnceLock<Regex> = OnceLock::new();
    R.get_or_init(|| Regex::new(r" win10\b").unwrap())
}

// ----------------------------------------------------------------------
// Catalog fetch (paginated)
// ----------------------------------------------------------------------

async fn fetch_full_catalog(api_key: &str) -> Result<Vec<(u32, String)>> {
    if api_key.is_empty() {
        return Err(Error::VdfParse(
            "Steam API key required to download art. Set one in Configure.".into(),
        ));
    }

    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| Error::VdfParse(format!("reqwest client: {e}")))?;

    let mut all: Vec<(u32, String)> = Vec::new();
    let mut last_appid: u64 = 0;
    loop {
        let resp: serde_json::Value = client
            .get(APPLIST_URL)
            .query(&[
                ("key", api_key.to_string()),
                ("max_results", "50000".to_string()),
                ("last_appid", last_appid.to_string()),
            ])
            .send()
            .await
            .map_err(|e| Error::VdfParse(format!("catalog GET: {e}")))?
            .json()
            .await
            .map_err(|e| Error::VdfParse(format!("catalog JSON: {e}")))?;

        let response = resp
            .get("response")
            .ok_or_else(|| Error::VdfParse("catalog response missing 'response'".into()))?;

        let apps = response
            .get("apps")
            .and_then(|v| v.as_array())
            .ok_or_else(|| Error::VdfParse("catalog response missing 'apps' array".into()))?;

        for app in apps {
            let appid = app
                .get("appid")
                .and_then(|v| v.as_u64())
                .map(|n| n as u32)
                .unwrap_or(0);
            let name = app
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if appid != 0 && !name.is_empty() {
                all.push((appid, name));
            }
        }

        let have_more = response
            .get("have_more_results")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
        if !have_more {
            break;
        }
        last_appid = response
            .get("last_appid")
            .and_then(|v| v.as_u64())
            .unwrap_or(0);
        if last_appid == 0 {
            break; // defensive: would otherwise loop forever
        }
    }
    Ok(all)
}

fn read_cache(path: &Path) -> Result<CachedCatalog> {
    let bytes = std::fs::read(path)?;
    let cached: CachedCatalog = serde_json::from_slice(&bytes)
        .map_err(|e| Error::VdfParse(format!("cache parse: {e}")))?;
    Ok(cached)
}

fn write_cache(path: &Path, apps: &[(u32, String)]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let cached = CachedCatalog {
        version: CACHE_VERSION,
        download_timestamp: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0),
        apps: apps.to_vec(),
    };
    let bytes = serde_json::to_vec(&cached)
        .map_err(|e| Error::VdfParse(format!("cache write: {e}")))?;
    std::fs::write(path, bytes)?;
    Ok(())
}

fn cache_is_stale(download_ts: u64) -> bool {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    now.saturating_sub(download_ts) > CATALOG_TTL.as_secs()
}

// ----------------------------------------------------------------------
// Art download
// ----------------------------------------------------------------------

/// One game's worth of art to fetch. `appid` is the Steam appid (after
/// `guess_appid`); `shortcut_id_unsigned` is what Steam expects in the
/// grid filenames.
#[derive(Debug, Clone)]
pub struct ArtTarget {
    pub appid: u32,
    pub shortcut_id_unsigned: u32,
    pub display_name: String,
}

/// Download all art for every target in parallel. Returns the count of
/// new files actually written (existing files are not re-downloaded).
pub async fn download_art_all(
    targets: Vec<ArtTarget>,
    grid_folder: PathBuf,
    replace_existing: bool,
) -> Result<usize> {
    tokio::fs::create_dir_all(&grid_folder)
        .await
        .map_err(Error::Io)?;

    let client = reqwest::Client::builder()
        .timeout(HTTP_TIMEOUT)
        .build()
        .map_err(|e| Error::VdfParse(format!("reqwest client: {e}")))?;

    let total_written: usize = stream::iter(targets)
        .map(|target| {
            let client = client.clone();
            let grid = grid_folder.clone();
            async move { download_one_game(&client, &target, &grid, replace_existing).await }
        })
        .buffer_unordered(ART_PARALLELISM)
        .fold(0usize, |acc, written| async move { acc + written })
        .await;

    Ok(total_written)
}

async fn download_one_game(
    client: &reqwest::Client,
    target: &ArtTarget,
    grid_folder: &Path,
    replace_existing: bool,
) -> usize {
    let urls = [
        (
            format!("{STEAM_CDN}/{}/library_600x900_2x.jpg", target.appid),
            grid_folder.join(format!("{}p.jpg", target.shortcut_id_unsigned)),
        ),
        (
            format!("{STEAM_CDN}/{}/library_hero.jpg", target.appid),
            grid_folder.join(format!("{}_hero.jpg", target.shortcut_id_unsigned)),
        ),
        (
            format!("{STEAM_CDN}/{}/logo.png", target.appid),
            grid_folder.join(format!("{}_logo.png", target.shortcut_id_unsigned)),
        ),
        (
            format!("{STEAM_CDN}/{}/header.jpg", target.appid),
            grid_folder.join(format!("{}_bigpicture.png", target.shortcut_id_unsigned)),
        ),
    ];

    let mut written = 0;
    for (url, dest) in urls {
        if !replace_existing && dest.is_file() {
            continue;
        }
        match client.get(&url).send().await {
            Ok(resp) if resp.status().is_success() => {
                if let Ok(bytes) = resp.bytes().await {
                    if tokio::fs::write(&dest, &bytes).await.is_ok() {
                        written += 1;
                    }
                }
            }
            _ => {
                // 404s are normal — not every game has every art type.
                // Stay quiet rather than spam stderr per game.
            }
        }
    }
    written
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    fn catalog(rows: &[(u32, &str)]) -> Catalog {
        Catalog::from_apps(rows.iter().map(|(id, n)| (*id, (*n).to_string())).collect())
    }

    #[test]
    fn direct_hit() {
        let c = catalog(&[(620, "Portal 2"), (440, "Team Fortress 2")]);
        assert_eq!(c.guess_appid("Portal 2"), Some(620));
        assert_eq!(c.guess_appid("Team Fortress 2"), Some(440));
    }

    #[test]
    fn prey_swap() {
        // 3970 is old Prey; 480490 is Prey 2017. The catalog returns 3970
        // for "Prey" but we should rewrite to 480490 (more grid art).
        let c = catalog(&[(3970, "Prey"), (480490, "Prey")]);
        assert_eq!(c.guess_appid("Prey"), Some(480490));
    }

    #[test]
    fn ultimate_edition_suffix() {
        let c = catalog(&[(870780, "Control Ultimate Edition")]);
        assert_eq!(c.guess_appid("Control"), Some(870780));
    }

    #[test]
    fn strips_win10_suffix() {
        let c = catalog(&[(894020, "Death's Door")]);
        assert_eq!(c.guess_appid("Death's Door Win10"), Some(894020));
    }

    #[test]
    fn strips_parenthetical_suffix() {
        let c = catalog(&[(813780, "Yakuza Kiwami")]);
        assert_eq!(c.guess_appid("Yakuza Kiwami (PC)"), Some(813780));
    }

    #[test]
    fn strips_pc_suffix() {
        let c = catalog(&[(417860, "Ghost of a Tale")]);
        assert_eq!(c.guess_appid("Ghost of a Tale PC"), Some(417860));
    }

    #[test]
    fn strips_for_windows_suffix() {
        let c = catalog(&[(1316840, "Genesis Noir")]);
        assert_eq!(c.guess_appid("Genesis Noir for Windows"), Some(1316840));
    }

    #[test]
    fn strips_subtitle_via_stripped_index() {
        let c = catalog(&[(271590, "Grand Theft Auto V")]);
        assert_eq!(
            c.guess_appid("Grand Theft Auto V: Premium Edition"),
            Some(271590)
        );
    }

    #[test]
    fn folds_accents() {
        let c = catalog(&[(384190, "ABZU")]);
        assert_eq!(c.guess_appid("ABZÛ"), Some(384190));
    }

    #[test]
    fn folds_punctuation() {
        let c = catalog(&[(1119980, "Raji An Ancient Epic")]);
        assert_eq!(c.guess_appid("Raji: An Ancient Epic"), Some(1119980));
    }

    #[test]
    fn strips_trademark_marks() {
        let c = catalog(&[(252950, "Rocket League")]);
        assert_eq!(c.guess_appid("Rocket League®"), Some(252950));
    }

    #[test]
    fn returns_none_for_unknown() {
        let c = catalog(&[(620, "Portal 2")]);
        assert_eq!(c.guess_appid("Some Game That Does Not Exist"), None);
    }

    #[test]
    fn trial_and_demo_dont_pollute_stripped_index() {
        let c = catalog(&[
            (111, "Game Trial"),
            (222, "Game Demo"),
            (333, "Game"),
        ]);
        // Demos/trials should still match their exact names...
        assert_eq!(c.guess_appid("Game Trial"), Some(111));
        // ...but a query for the real game shouldn't fall back to them.
        assert_eq!(c.guess_appid("Game"), Some(333));
    }
}
