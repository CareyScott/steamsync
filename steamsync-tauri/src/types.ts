export interface Game {
  app_name: string;
  display_name: string;
  executable_path: string;
  install_folder: string;
  launch_arguments: string;
  icon: string;
  uri: string | null;
  storetag: string;
  shortcut_id: number | null;
}

/** Sync status of a game relative to the current shortcuts.vdf. */
export type GameStatus = "new" | "synced" | "broken" | "unknown";

export interface SteamAccount {
  steamid: string;
  username: string;
}

export interface DetectResult {
  games: Game[];
  accounts: SteamAccount[];
  default_steam_path: string;
  sources: string[];
  error?: string;
}

export interface ApplyResult {
  added?: number;
  removed?: number;
  wrote_shortcuts?: boolean;
  steamid?: string;
  username?: string;
  error?: string;
}

export interface SyncOptions {
  steamid: string;
  sources: string[];
  use_uri: boolean;
  replace_existing: boolean;
  remove_missing: boolean;
  download_art: boolean;
  egs_manifests: string;
  steam_path: string;
  steam_api_key: string;
}

/** Human-readable label for each known storetag value. */
export const SOURCE_LABELS: Record<string, string> = {
  epicstore: "Epic Games Store",
  xbox: "Xbox",
};
