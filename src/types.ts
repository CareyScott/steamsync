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
  /** Alternative executables to pick from (local games only). Largest-first; first entry is recommended. */
  exe_candidates: string[];
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
  existing_app_names: string[];
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
  steamgriddb_api_key: string;
  /** Root folders to scan for locally-installed games (one subfolder per game). */
  local_folders: string[];
}

/** Human-readable label for each known storetag value. */
export const SOURCE_LABELS: Record<string, string> = {
  epicstore: "Epic Games Store",
  xbox: "Xbox",
  local: "Local Folders",
};
