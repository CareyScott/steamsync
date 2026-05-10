// Prevent the console window from appearing on Windows release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use tauri::AppHandle;
use tauri_plugin_shell::process::CommandEvent;
use tauri_plugin_shell::ShellExt;

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct SyncOptions {
    pub steamid: String,
    pub sources: Vec<String>,
    pub use_uri: bool,
    pub replace_existing: bool,
    pub remove_missing: bool,
    pub download_art: bool,
    pub egs_manifests: String,
    pub itch_library: String,
    pub steam_path: String,
    pub steam_api_key: String,
}

fn build_args(opts: &SyncOptions, mode_flag: &str) -> Vec<String> {
    let mut args: Vec<String> = vec![mode_flag.into()];
    for src in &opts.sources {
        args.push("--source".into());
        args.push(src.clone());
    }
    if !opts.steam_path.is_empty() {
        args.push("--steam-path".into());
        args.push(opts.steam_path.clone());
    }
    if !opts.steamid.is_empty() {
        args.push("--steamid".into());
        args.push(opts.steamid.clone());
    }
    if !opts.egs_manifests.is_empty() {
        args.push("--egs-manifests".into());
        args.push(opts.egs_manifests.clone());
    }
    if !opts.itch_library.is_empty() {
        args.push("--itch-library".into());
        args.push(opts.itch_library.clone());
    }
    if !opts.steam_api_key.is_empty() {
        args.push("--steam-api-key".into());
        args.push(opts.steam_api_key.clone());
    }
    if opts.use_uri {
        args.push("--use-uri".into());
    }
    if opts.replace_existing {
        args.push("--replace-existing".into());
    }
    if opts.remove_missing {
        args.push("--remove-missing".into());
    }
    if opts.download_art {
        args.push("--download-art".into());
    }
    args
}

#[tauri::command]
async fn detect_games(
    app: AppHandle,
    opts: SyncOptions,
) -> Result<serde_json::Value, String> {
    let args = build_args(&opts, "--json-collect");
    run_sidecar(&app, args).await
}

#[tauri::command]
async fn apply_changes(
    app: AppHandle,
    opts: SyncOptions,
    selected_app_names: Vec<String>,
) -> Result<serde_json::Value, String> {
    let mut args = build_args(&opts, "--json-apply");
    let selection = serde_json::json!({ "selected_app_names": selected_app_names });
    args.push("--selection".into());
    args.push(selection.to_string());
    run_sidecar(&app, args).await
}

async fn run_sidecar(
    app: &AppHandle,
    args: Vec<String>,
) -> Result<serde_json::Value, String> {
    let cmd = app
        .shell()
        .sidecar("steamsync")
        .map_err(|e| format!("Failed to locate steamsync sidecar: {e}"))?
        .args(args);

    let (mut rx, _child) = cmd
        .spawn()
        .map_err(|e| format!("Failed to spawn steamsync sidecar: {e}"))?;

    let mut stdout_buf = String::new();
    let mut stderr_buf = String::new();
    let mut exit_code: Option<i32> = None;

    while let Some(event) = rx.recv().await {
        match event {
            CommandEvent::Stdout(bytes) => {
                stdout_buf.push_str(&String::from_utf8_lossy(&bytes));
                stdout_buf.push('\n');
            }
            CommandEvent::Stderr(bytes) => {
                stderr_buf.push_str(&String::from_utf8_lossy(&bytes));
                stderr_buf.push('\n');
            }
            CommandEvent::Terminated(payload) => {
                exit_code = payload.code;
            }
            _ => {}
        }
    }

    let stdout_trimmed = stdout_buf.trim();
    if stdout_trimmed.is_empty() {
        return Err(format!(
            "steamsync produced no output (exit {:?}). stderr:\n{stderr_buf}",
            exit_code
        ));
    }

    serde_json::from_str(stdout_trimmed).map_err(|e| {
        format!(
            "Failed to parse JSON from steamsync (exit {:?}): {e}\nstdout:\n{stdout_buf}\nstderr:\n{stderr_buf}",
            exit_code
        )
    })
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .invoke_handler(tauri::generate_handler![detect_games, apply_changes])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
