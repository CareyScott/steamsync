# Native Rust port roadmap

## Phase 1 — Foundation ✅ (this commit)

- Tauri 2 + React 18 scaffold with no sidecar.
- `steam::account::enumerate_accounts` — reads `localconfig.vdf` via `keyvalues-parser`, handles `Friends` / `friends` casing.
- `steam::id::shortcut_id_*` — CRC32 algorithm bit-identical to the Python `_get_steam_shortcut_id`, with reference values baked into tests so drift fails loudly.
- `commands::detect_games` wired up: returns Steam accounts + empty games list.
- `commands::apply_changes` returns `NotYetImplemented` until Phase 2.

## Phase 2 — Binary `shortcuts.vdf` codec (next)

The risky piece. No maintained Rust crate parses Steam's binary VDF format. Plan:

1. Capture a real `shortcuts.vdf` from a live Steam install (the developer's own).
2. Hand-roll a reader using `byteorder` against the format spec (tagged tree of objects, ints, strings, null-terminated).
3. Hand-roll a writer.
4. Round-trip test: `read → write → byte-for-byte equal` against captured fixtures with 0, 1, and many shortcuts. **Until this passes, Apply must stay locked out.**
5. Add tests for the edge cases Python hit: missing `Exe`/`exe` field, missing `LaunchOptions`, etc.

Add deps: `byteorder = "1"`.

## Phase 3 — Native launchers

Port each launcher. Recommended order (simplest first):

1. **EGS** (`launchers/egs.rs`) — glob `.item` files in the manifest dir, parse with `serde_json`. ~2-3h.
2. **legendary** (`launchers/legendary.rs`) — `std::process::Command` wrapper around `legendary list-installed --json`. ~1h.
3. **itch** (`launchers/itch.rs`) — gunzip `receipt.json.gz`, parse the JSON; also parse `.itch.toml` for the launch action. Deps: `flate2`, `toml`. ~3-4h.
4. **Xbox** (`launchers/xbox.rs`) — keep the existing PowerShell script (it's small and works), invoke via `Command::new("powershell.exe")`, parse the JSON output, then `quick-xml` to read `MicrosoftGame.config` for the exe + display name. ~3-4h.

Each gets its own integration test with a fixture directory under `tests/fixtures/`.

## Phase 4 — Steam catalog + art download

- `api::SteamCatalog` — fetch `IStoreService/GetAppList/v1` with **pagination** (Python hardcodes `max_results=50000` and silently truncates the catalog — fix while we're here). Deps: `reqwest` (already in deps once we add it), local cache via `dirs::cache_dir()`.
- `api::guess_appid` — port the regex-based name-matching from `steameditor.py::guess_appid`.
- `api::download_art` — parallel grid art download via `tokio::spawn` (concurrency limit ~8). Python is sequential; this is a free speedup.

Add deps: `tokio = { version = "1", features = ["full"] }`, `reqwest = { version = "0.12", features = ["json"] }`, `regex = "1"`, `futures = "0.3"`.

## Phase 5 — Wire up + polish

- Replace the placeholder bodies in `commands.rs` with real apply logic.
- Tauri `emit` for progress events during long apply runs.
- MSI / NSIS installer config in `tauri.conf.json` (Tauri's default bundle should "just work").
- Top-level repo README repositions the Tauri app as THE install path; CLI relegated to "advanced".
- Cross-platform smoke: macOS, Linux. Steam path defaults already differ in `types::default_steam_path`.

## Out of scope for this port

- Mobile (Android/iOS) — Tauri 2 supports it via the `lib.rs`/`main.rs` split we already have, but there's no point until the desktop app is finished.
- Auto-update — defer until v1.
- Telemetry — explicitly not happening.
