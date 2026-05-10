# Roadmap

## What's done

All five phases of the original Python → native Rust port are in. The app reads launcher libraries, writes `shortcuts.vdf`, downloads cover art, and ships as a single Rust binary.

- **Foundation:** Tauri 2 + React 18 scaffold, Steam account enumeration via text VDF, CRC32 shortcut-id bit-identical to Python's algorithm (reference values baked into tests).
- **Binary `shortcuts.vdf` codec:** hand-rolled reader + writer for Steam's tagged little-endian format. Preserves unknown fields and insertion order so identity round-trips are byte-exact. Cross-validated against Python `vdf.binary_dumps`.
- **Native launchers:** Epic Games Store (`.item` JSON manifests) and Xbox (PowerShell + XML manifest parsing). Same filter logic as upstream Python steamsync.
- **SteamGridDB integration:** autocomplete-based fuzzy matching for game names, parallel art download (8 in flight), four art types per game (vertical grid, hero, logo, big-picture).
- **Apply path:** account picker, atomic backup + write of `shortcuts.vdf`, optional cover art download, all driven by a single typed Tauri command.
- **UX polish:** auto-detect Steam path via Windows registry, settings persistence via localStorage, light/dark theme toggle, first-run welcome modal, SteamGridDB onboarding modal, cover-art preview grid, confirmation dialog before any write, live progress events via `Window::emit`.

## Open

- **Backup browser.** List `shortcuts.vdf-*.bak` files alongside their timestamps and offer a one-click restore.
- **Cross-platform builds.** Default Steam path is already platform-aware; the Xbox launcher is Windows-only by definition, but EGS could work on macOS/Linux for the (rare) Wine setups. Bundle/install scripts need verification on macOS and a Linux target added.
- **Icon assets.** `src-tauri/icons/icon.ico` is in place for Windows. macOS / Linux bundling wants 32×32 and 128×128 PNGs — generate from a single source PNG via `npx @tauri-apps/cli icon`.
- **Progress streaming for art download.** Apply progress events are emitted for the per-game lookup loop but not for the inner SGDB call. Easy refactor; nice to have.

## Not planned

- itch.io / legendary support. Out of scope on purpose — niche, and the original Python CLI still supports them for users who need it.
- Auto-update. Defer until the project sees real downloads.
- Telemetry. Explicitly not happening.
- Mobile builds. Tauri 2 supports it via the `lib.rs`/`main.rs` split already in place, but there's no reason to target it for this domain.
