# steamsync

A small, native desktop app that adds your **Epic Games Store** and **Xbox** games to Steam as non-Steam shortcuts — with high-quality cover art from [SteamGridDB](https://www.steamgriddb.com).

Tauri 2 (Rust) + React 18 + antd. Single binary, no Python at runtime, no sidecar.

> **Platform support:** Windows only. The Xbox launcher needs Windows by definition; the rest of the app may compile and run on macOS / Linux, but those targets are **untested and unsupported** — use at your own risk.

```
React UI (antd)  →  #[tauri::command]  →  Rust modules  →  shortcuts.vdf
   TypeScript        async commands       steam/, launchers/      (binary)
```

## Features

- **Finds your installed games** across Epic Games Store and the Xbox app on Windows.
- **Adds them to Steam** as non-Steam shortcuts, with `steamsync` and storefront tags so you can filter in Steam.
- **Cover art preview** — see exactly what each shortcut will look like before you write anything.
- **Parallel art download** from [SteamGridDB](https://www.steamgriddb.com) (8 in flight).
- **Safe writes:** atomic `shortcuts.vdf` updates with timestamped backups. Nothing destructive happens without a confirmation dialog.
- **Auto-detects** your Steam install path via the Windows registry.
- **Settings persist** across launches; **light/dark theme** toggle.

## Install on this PC

Same one-liner pattern as [GitSwitch](https://github.com/CareyScott/GitSwitch): build a release bundle then launch the platform installer.

### Windows

```powershell
npm install
npm run install:app:win
```

That runs `tauri build`, then launches the generated NSIS installer. Click through it and steamsync is in your Start menu.

### macOS / Linux (untested, unsupported)

`install:app` exists in `package.json` as a placeholder one-liner that copies a `.app` bundle into `/Applications/`. It has not been verified — neither the build nor the installed app are tested on these platforms. The Xbox launcher in particular will not work outside Windows.

If you want to experiment, file issues — pull requests welcome — but expect rough edges.

### Just build, don't install

```powershell
npm install
npm run tauri build
```

What lands under `src-tauri/target/release/`:

| File | What it is |
|---|---|
| `steamsync.exe` | Standalone portable binary. Double-click to run, no installer needed. ~12 MB. |
| `bundle/nsis/*-setup.exe` | NSIS installer (used by `install:app:win`). |
| `bundle/msi/*.msi` | MSI installer (alternative for enterprise distribution). |

The first build compiles ~370 Rust crates and takes ~10–15 minutes. Subsequent builds are seconds.

## Develop

```powershell
npm install
npm run tauri dev
```

Hot-reload on file changes.

## Test

```powershell
cd src-tauri
cargo test
```

42 unit tests cover the `shortcuts.vdf` codec (round-trip is byte-exact against Python's reference `vdf` library), every launcher filter rule, the name-matching helpers, and the account / shortcut-aliveness logic.

## Prerequisites

- **Windows 10/11** (only supported target — see top of README)
- Node 20+
- Rust 1.78+
- MSVC C++ Build Tools + WebView2 runtime

## Layout

```
.
├── src/                   React frontend (TypeScript + antd)
│   ├── main.tsx           entrypoint + ConfigProvider theme
│   ├── App.tsx            three-tab Layout: Find → Configure → Add
│   ├── api.ts             typed invoke() wrappers + event listeners
│   ├── types.ts           shapes mirrored 1:1 from src-tauri/src/types.rs
│   └── views/
│       ├── DetectView.tsx     provider-grouped game list with search
│       ├── ConfigureView.tsx  options + SGDB onboarding modal
│       └── ApplyView.tsx      preview grid + confirm + progress
└── src-tauri/             Rust backend
    ├── Cargo.toml
    ├── tauri.conf.json
    ├── capabilities/      Tauri permissions (shell:opener scoped to SGDB)
    ├── icons/
    ├── scripts/           bundled PowerShell for Xbox library enumeration
    └── src/
        ├── main.rs        thin entrypoint, calls lib::run()
        ├── lib.rs         tauri::Builder, command registration
        ├── types.rs       DetectResult, ApplyResult, SyncOptions
        ├── error.rs       typed Error → JSON string for the frontend
        ├── commands.rs    #[tauri::command] handlers
        ├── api.rs         SteamGridDB client + parallel art download
        ├── steam/
        │   ├── account.rs     localconfig.vdf parser
        │   ├── id.rs          CRC32 shortcut-id (bit-exact vs Python)
        │   └── shortcuts.rs   binary shortcuts.vdf codec
        └── launchers/
            ├── egs.rs         Epic Games Store
            └── xbox.rs        Xbox / Microsoft Store
```

## Roadmap

See [ROADMAP.md](./ROADMAP.md). Headline items still open:

- Backup browser (list `shortcuts.vdf-*.bak`, one-click restore)
- macOS / Linux are not on the roadmap (Windows is the only supported target — see top of README)

## Credits

- Algorithm + filter rules originally from [`jaydenmilne/steamsync`](https://github.com/jaydenmilne/steamsync) (AGPLv3). The Python CLI lives there if you need a headless option.
- Cover art via [SteamGridDB](https://www.steamgriddb.com) — community-curated, free with an API key.
- App shape mirrored from [`CareyScott/GitSwitch`](https://github.com/CareyScott/GitSwitch).

## License

[AGPL-3.0-or-later](./LICENSE).
