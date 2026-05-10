# steamsync-tauri

A Tauri 2 + React 18 + antd desktop GUI for [steamsync](../steamsync/), inspired by [GitSwitch-Gui](https://github.com/biohacker0/GitSwitch-Gui).

The Python CLI does the work — this app is a sidecar shell that drives it via the new `--json-collect` and `--json-apply` modes.

## Architecture

```
React UI (antd)  →  Tauri command  →  steamsync.exe sidecar  →  shortcuts.vdf
   (TypeScript)      (Rust)             (Python via PyInstaller)
```

Three views:

1. **Detect** — invokes `steamsync --json-collect`, shows a checklist of games.
2. **Configure** — toggles sources / behavior (URI vs exe, replace, remove-missing, art).
3. **Apply** — invokes `steamsync --json-apply --selection '{...}'`, shows the result.

## Prerequisites

- Node 20+ (you have 24).
- Rust 1.78+ (you have 1.95).
- Python 3.10+ and Poetry (to build the sidecar).
- On Windows, the Microsoft C++ Build Tools / WebView2 runtime (Tauri 2 prerequisites).

## First-time setup

```powershell
# 1. Build the steamsync CLI as a single .exe
cd ..\steamsync
poetry install
poetry run exe                     # produces dist\steamsync.exe via PyInstaller

# 2. Copy it into the sidecar slot (Tauri requires the target-triple suffix)
Copy-Item dist\steamsync.exe `
  ..\steamsync-tauri\src-tauri\bin\steamsync-x86_64-pc-windows-msvc.exe

# 3. Install JS deps and run dev mode
cd ..\steamsync-tauri
npm install
npm run tauri:dev
```

For a release build:

```powershell
npm run tauri:build
# Bundle ends up under src-tauri\target\release\bundle\
```

## What's intentionally minimal

This is a scaffold, not a finished app. Things deliberately left out:

- **No global state / Redux / Zustand** — `useState` per-screen, like GitSwitch.
- **No streaming progress events.** The Apply view shows a spinner until the sidecar exits, then renders the result. Wiring incremental events from the sidecar through `Window::emit` is a follow-up.
- **No icons.** `src-tauri/icons/` is empty. `tauri.conf.json` references `icons/icon.ico` etc.; replace with real assets or `npx @tauri-apps/cli icon path/to/source.png` to generate from one source image.
- **No router.** Three antd `Tabs`. If view count grows, add `react-router-dom`.
- **No tests.** Add `vitest` for the React side and `cargo test` for Rust as the sidecar surface grows.

## Why a Python sidecar instead of a native Rust port?

Faster to ship, and the existing launcher logic (EGS, itch, Xbox PS scripts, legendary) keeps working unchanged. The boundary between the Python core and the Rust shell is the JSON contract — when a piece needs porting (e.g. shortcuts.vdf binary codec for performance), the contract stays the same.

See the [migration notes in the parent review](../README.md) for what a full Path A (native Rust) rewrite would look like.

## License

AGPL-3.0-or-later, matching steamsync.
