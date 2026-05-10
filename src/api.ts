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

export interface ArtPreview {
  display_name: string;
  box_art_url: string | null;
}

/** Fetch SGDB box-art URLs for a list of display names. Used to render
 * a thumbnail grid in the Apply view before the user commits. */
export async function fetchArtPreviews(
  apiKey: string,
  displayNames: string[],
): Promise<ArtPreview[]> {
  return await invoke<ArtPreview[]>("fetch_art_previews", {
    apiKey,
    displayNames,
  });
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
