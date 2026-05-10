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

## Status (in-progress port)

This is a multi-phase port from the Python sidecar architecture to native Rust. See [ROADMAP.md](./ROADMAP.md).

| Phase | Status |
|---|---|
| 1. Foundation — Steam account enum, shortcut-id, project scaffold | ✅ done |
| 2. Binary `shortcuts.vdf` read/write codec | 🚧 next |
| 3. Native launchers (EGS, Xbox) | ⏳ pending |
| 4. Steam catalog API + parallel art download | ⏳ pending |
| 5. Wire everything to the UI, polish, installer | ⏳ pending |

Until Phase 2 lands, the **Apply** view returns `NotYetImplemented` — the app will refuse to touch your library rather than risk corrupting `shortcuts.vdf` with an incomplete codec.

## Prerequisites

- Node 20+ (you have 24)
- Rust 1.78+ (you have 1.95)
- Windows: MSVC C++ Build Tools + WebView2 runtime
- macOS / Linux: standard Tauri 2 prerequisites

## Develop

```powershell
cd steamsync-tauri
npm install
npm run tauri:dev
```

First run compiles ~370 Rust crates (10–15 min). Subsequent runs are seconds.

Run the Rust tests:

```powershell
cd src-tauri
cargo test
```

## Build a release

```powershell
npm run tauri:build
# MSI and NSIS installers land in src-tauri/target/release/bundle/
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
