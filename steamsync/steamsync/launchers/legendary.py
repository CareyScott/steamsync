import subprocess
import json
import os

import steamsync.defs as defs
import steamsync.launchers.launcher as launcher


class LegendaryLauncher(launcher.Launcher):
    """Support for the Legendary launcher

    https://github.com/derrod/legendary"""

    def __init__(self, legendary_command: str = "legendary"):
        self.legendary_command = legendary_command

    def collect_games(self) -> list[defs.GameDefinition]:
        games_dict = {}
        try:
            games_raw_json = self._run_legendary("list-games", "--json")
            installed_raw_json = self._run_legendary("list-installed", "--json")
        except FileNotFoundError:
            print(
                f"Could not find legendary executable: '{self.legendary_command}'. "
                "Pass --legendary-command to point at it, or omit --source legendary."
            )
            return []
        except subprocess.CalledProcessError as e:
            print(f"legendary returned exit code {e.returncode}; skipping.")
            return []

        games_json = json.loads(games_raw_json)
        for entry in games_json:
            # TODO: Map other useful information, like tags?
            key_images = entry.get("metadata", {}).get("keyImages") or []
            art = key_images[0] if key_images else None
            games_dict[entry["app_name"]] = {"art": art}
        games = list()
        parsed_json = json.loads(installed_raw_json)
        for entry in parsed_json:
            app_name = entry["app_name"]
            launch_args = " launch " + app_name
            display_name = entry["title"]
            install_location = entry["install_path"]
            art_url = None
            icon = os.path.join(install_location, entry["executable"])
            if app_name in games_dict:
                art_url = games_dict[app_name]["art"]

            games.append(
                defs.GameDefinition(
                    self.legendary_command,
                    display_name,
                    app_name,
                    install_location,
                    launch_args,
                    art_url,
                    defs.TAG_LEGENDARY,
                    icon,
                )
            )
        return games

    def _run_legendary(self, *args) -> str:
        result = subprocess.run(
            [self.legendary_command, *args],
            check=True,
            capture_output=True,
            text=True,
        )
        return result.stdout

    def get_store_id(self) -> str:
        return defs.TAG_LEGENDARY

    def get_display_name(self) -> str:
        return "legendary"

    def is_installed(self) -> bool:
        return True  # TODO
