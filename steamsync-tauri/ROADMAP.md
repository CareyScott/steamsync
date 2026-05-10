# Roadmap

The native Rust port is complete. This file kept around so the phasing story is preserved for future contributors.

## Phase 1 — Foundation ✅

- Tauri 2 + React 18 scaffold, no sidecar.
- `steam::account::enumerate_accounts` — text VDF reader for `localconfig.vdf`. Handles `Friends` / `friends` casing.
- `steam::id::shortcut_id_*` — CRC32 algorithm, bit-identical to Python's `_get_steam_shortcut_id`. Reference values from the live Python implementation are baked into tests.
- Typed `Error` (`thiserror`), serializes to a string for the frontend.

## Phase 2 — Binary `shortcuts.vdf` codec ✅

- Hand-rolled reader + writer for Steam's tagged little-endian format.
- `Value` enum (Object / String / Int32 / UInt64) preserving insertion order so identity round-trips are byte-exact.
- Typed `Shortcut` struct with `extra: Vec<(String, Value)>` for unknown fields → never drops data on a round-trip.
- Cross-validated against Python's `vdf.binary_dumps` (byte-for-byte equal).
- 12 unit tests.

## Phase 3 — Native launchers ✅

Scope narrowed to **Epic Games Store** + **Xbox**. itch.io and legendary remain in the Python CLI for headless users.

- `launchers::egs` — globs `*.item` JSON files from the manifests dir, applies the same filtering rules as Python (`bIsApplication`, `AppCategories` contains `"games"`, sanitize leading slash, exe must exist on disk).
- `launchers::xbox` — invokes `powershell.exe` with the embedded `list_xbox_games.ps1`, parses the JSON output, then reads `MicrosoftGame.config` (preferred) or `AppxManifest.xml` via `quick-xml` for older titles. Heuristics ported verbatim from `xbox.py`.
- 13 unit tests using synthetic XML and JSON fixtures.

## Phase 4 — Steam catalog + parallel art download ✅

- `api::Catalog::load_or_fetch` — disk-cached, 7-day TTL.
- **Pagination fix:** uses `last_appid` to fetch the full catalog. Python's `max_results=50000` silently truncates and misses any game past that cutoff.
- `Catalog::guess_appid` — ports every regex fallback from `steameditor.py` (Ultimate Edition suffix, Win10 strip, parenthetical strip, accent fold, punctuation fold, non-ASCII strip, the Prey-1-vs-Prey-2017 swap).
- `download_art_all` — parallel grid art download via `tokio` + `futures::buffer_unordered` (8 in-flight). Python is sequential; this is a free speedup.
- 13 unit tests for the name-matching cascade.

## Phase 5 — Apply path wired ✅

- `commands::apply_changes` does the full flow: pick account, load existing `shortcuts.vdf`, merge selected games (dedup by `"{exe}|{launch_options}"`), optionally remove dead entries, re-index, atomic backup + write, optional parallel art download.
- Backup format matches Python: `shortcuts.vdf-YYYYMMDD-HHMMSS.bak`.
- Atomic write: temp file + `rename` so a crash mid-write can't truncate the user's library.
- Shortcut `appid` is preserved when replacing existing entries (otherwise downloaded grid art would orphan).
- The Python `--live-dangerously` flag (skip backup) is intentionally not exposed in the GUI. Always-backup is the safe default.

## Future work (not in this PR)

- **QoL polish** noted in the original feature request but not yet wired:
  - Auto-detect Steam path via Windows registry (`HKLM\SOFTWARE\WOW6432Node\Valve\Steam\InstallPath`).
  - Persist settings between runs (via `tauri-plugin-store` or flat JSON in `dirs::config_dir()`).
  - Light/dark theme toggle (currently hardcoded dark).
  - Backup browser — list `shortcuts.vdf-*.bak` files, one-click restore.
  - Confirmation dialog before write.
  - Last-sync timestamp in the header.
- **Progress streaming.** The Apply view currently shows a spinner until completion; wire `tauri::Window::emit` from `apply_changes` to stream per-game events.
- **Cross-platform.** macOS / Linux builds. Steam paths are already platform-aware in `types::default_steam_path`; the Xbox launcher Windows-only by definition.
- **Installer.** `tauri build` produces MSI + NSIS bundles out of the box; just needs icon assets generated (32x32, 128x128 PNGs from a single source).
- **Mobile.** Tauri 2 supports it via the `lib.rs`/`main.rs` split this project already uses. No reason to right now.
