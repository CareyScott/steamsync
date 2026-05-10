import { invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";
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

export async function autoDetectSteamPath(): Promise<string | null> {
  return await invoke<string | null>("auto_detect_steam_path");
}

/** Live progress events emitted from `apply_changes`. */
export type ApplyEvent =
  | { stage: "detecting"; launcher: string }
  | { stage: "writing-shortcuts" }
  | {
      stage: "downloading-art";
      game: string;
      current: number;
      total: number;
    };

export async function onApplyProgress(
  callback: (event: ApplyEvent) => void,
): Promise<UnlistenFn> {
  return await listen<ApplyEvent>("apply-progress", (event) =>
    callback(event.payload),
  );
}
