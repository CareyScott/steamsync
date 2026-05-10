import { invoke } from "@tauri-apps/api/core";
import type { ApplyResult, DetectResult, SyncOptions } from "./types";

export async function detectGames(opts: SyncOptions): Promise<DetectResult> {
  return await invoke<DetectResult>("detect_games", { opts });
}

export async function applyChanges(
  opts: SyncOptions,
  selectedAppNames: string[],
): Promise<ApplyResult> {
  return await invoke<ApplyResult>("apply_changes", {
    opts,
    selectedAppNames,
  });
}
