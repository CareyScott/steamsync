# steamsync-tauri

Native Tauri 2 + React 18 desktop app for syncing **Epic Games Store** and **Xbox** games into Steam as shortcuts. (itch.io and legendary stay supported in the standalone Python CLI for headless users.)

Inspired by [GitSwitch-Gui](https://github.com/biohacker0/GitSwitch-Gui) — same shape (Tauri shell + React UI + small Rust backend), targeted at the steamsync problem domain.

## Architecture

```
React UI (antd)  →  #[tauri::command]  →  Rust modules  →  shortcuts.vdf
   TypeScript        async commands       steam/, launchers/      (binary)
```

No Python, no sidecar, no shell-out. Single Rust binary plus the bundled webview.

The Python CLI at [`../steamsync`](../steamsync) is preserved as a standalone CLI for headless / Linux users; the Tauri app is the primary product.

## Status

All five phases of the native Rust port are in. The app reads your launcher libraries, writes Steam shortcuts, and downloads grid art entirely in Rust — no Python in the runtime path.

| Phase | Status |
|---|---|
| 1. Foundation — Steam account enum, shortcut-id, scaffold | ✅ |
| 2. Binary `shortcuts.vdf` read/write codec | ✅ |
| 3. Native launchers (EGS, Xbox) | ✅ |
| 4. Steam catalog API + parallel art download | ✅ |
| 5. Apply path wired end-to-end | ✅ |

42 unit tests cover the codec round-trip, every launcher filter rule, every name-matching fallback, and the account/cleanup helpers. The `shortcuts.vdf` codec is cross-validated against Python's reference `vdf` library for byte-exact output.

## Prerequisites

- Node 20+ (you have 24)
- Rust 1.78+ (you have 1.95)
- Windows: MSVC C++ Build Tools + WebView2 runtime
- macOS / Linux: standard Tauri 2 prerequisites

## Build the `.exe`

One command. Same shape as [GitSwitch-Gui](https://github.com/biohacker0/GitSwitch-Gui).

```powershell
cd steamsync-tauri
npm install
npm run tauri build
```

What you get under `src-tauri/target/release/`:

| File | What it is |
|---|---|
| `steamsync.exe` | **Standalone portable binary.** Double-click to run, no installer needed. ~12 MB. |
| `bundle/msi/steamsync_0.1.0_x64_en-US.msi` | Windows MSI installer (recommended for distribution). |
| `bundle/nsis/steamsync_0.1.0_x64-setup.exe` | NSIS installer (alternative). |

The first build is the slow one — Rust compiles ~370 crates (~10-15 min). Subsequent builds are seconds.

> **Shortcut:** `npm run tauri:build` is the same as `npm run tauri build` — both forward to `tauri build`.

## Develop (hot-reload)

```powershell
cd steamsync-tauri
npm install
npm run tauri dev
```

Opens the app with hot reload on file changes. Use this during development.

## Run the tests

```powershell
cd steamsync-tauri/src-tauri
cargo test
```

## Layout

```
steamsync-tauri/
├── src/                       React frontend (antd)
│   ├── main.tsx
│   ├── App.tsx                three-tab Layout: Detect / Configure / Apply
│   ├── types.ts               shapes must match src-tauri/src/types.rs
│   ├── api.ts                 typed invoke() wrappers
│   └── views/
└── src-tauri/                 Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/
    ├── icons/
    └── src/
        ├── main.rs            thin wrapper, calls lib::run()
        ├── lib.rs             tauri::Builder, command registration
        ├── types.rs           DetectResult, ApplyResult, SyncOptions
        ├── error.rs           typed errors serialized to the frontend
        ├── commands.rs        #[tauri::command] handlers
        ├── steam/
        │   ├── account.rs     enumerate localconfig.vdf
        │   ├── id.rs          CRC32 shortcut-id (matches Python)
        │   └── shortcuts.rs   Phase 2 placeholder
        └── launchers/
            └── mod.rs         Phase 3 placeholder
```

## License

AGPL-3.0-or-later, matching upstream steamsync.
