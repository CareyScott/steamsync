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
- **Progress streaming for art download.** Apply progress events fire for the per-game outer loop but not for the inner SGDB call. Easy refactor; nice to have.

## Not planned

- **macOS / Linux support.** Windows-only by design — the Xbox launcher relies on PowerShell + `Get-AppxPackage`, which has no cross-platform equivalent. The repo still has `cfg(windows)` gates and a placeholder `install:app` script, but those are scaffolding for hypothetical future contributors, not a commitment.
- **itch.io / legendary support.** Out of scope on purpose — niche, and the original Python CLI at [`jaydenmilne/steamsync`](https://github.com/jaydenmilne/steamsync) still supports them for users who need it.
- **Auto-update.** Defer until the project sees real downloads.
- **Telemetry.** Explicitly not happening.
- **Mobile builds.** No reason to target it for this domain.
