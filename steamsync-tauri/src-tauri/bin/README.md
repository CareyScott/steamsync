# Sidecar binary location

Tauri expects the steamsync CLI executable at this directory, named with the Rust target triple suffix:

- Windows: `steamsync-x86_64-pc-windows-msvc.exe`
- macOS (Intel): `steamsync-x86_64-apple-darwin`
- macOS (Apple Silicon): `steamsync-aarch64-apple-darwin`
- Linux: `steamsync-x86_64-unknown-linux-gnu`

## Build the sidecar (Windows)

From the repo root:

```powershell
cd ..\steamsync
poetry install
poetry run exe
# Produces dist\steamsync.exe
Copy-Item dist\steamsync.exe ..\steamsync-tauri\src-tauri\bin\steamsync-x86_64-pc-windows-msvc.exe
```

The `bin\*.exe` files are gitignored — every developer rebuilds locally.
