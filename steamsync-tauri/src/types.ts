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
  itch_library: string;
  steam_path: string;
  steam_api_key: string;
}
