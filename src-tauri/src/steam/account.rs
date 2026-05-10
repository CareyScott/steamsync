//! Port of `SteamDatabase.enumerate_steam_accounts` from steameditor.py.
//!
//! Walks `<steam_path>/userdata/<steamid>/config/localconfig.vdf` for every
//! account on this machine. The display name lives at
//! `UserLocalConfigStore.Friends.PersonaName` (case varies between accounts —
//! sometimes "friends", sometimes "Friends").

use std::fs;
use std::path::Path;

use keyvalues_parser::Vdf;

use crate::error::{Error, Result};
use crate::types::SteamAccount;

pub fn enumerate_accounts(steam_path: &Path) -> Result<Vec<SteamAccount>> {
    let userdata = steam_path.join("userdata");
    if !userdata.is_dir() {
        return Err(Error::SteamPathMissing(
            steam_path.to_string_lossy().into_owned(),
        ));
    }

    let mut accounts = Vec::new();
    for entry in fs::read_dir(&userdata)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let steamid = entry.file_name().to_string_lossy().into_owned();
        let localconfig = entry.path().join("config").join("localconfig.vdf");
        if !localconfig.is_file() {
            continue;
        }
        let username = read_persona_name(&localconfig)
            .unwrap_or_else(|_| "(unknown username)".to_string());
        accounts.push(SteamAccount { steamid, username });
    }

    Ok(accounts)
}

fn read_persona_name(path: &Path) -> Result<String> {
    // localconfig.vdf can contain bytes that aren't valid UTF-8 (Steam writes
    // arbitrary game titles in there). Lossy decode mirrors the Python code's
    // `errors="replace"` since we only care about one field.
    let bytes = fs::read(path)?;
    let raw = String::from_utf8_lossy(&bytes);

    let vdf = Vdf::parse(&raw).map_err(|e| Error::VdfParse(e.to_string()))?;
    let store = vdf
        .value
        .get_obj()
        .ok_or_else(|| Error::VdfParse("UserLocalConfigStore is not an object".into()))?;

    // Try both casings — Steam is inconsistent across accounts.
    let friends = store
        .get("Friends")
        .or_else(|| store.get("friends"))
        .and_then(|values| values.first())
        .and_then(|v| v.get_obj())
        .ok_or_else(|| Error::VdfParse("missing Friends/friends".into()))?;

    let persona = friends
        .get("PersonaName")
        .and_then(|values| values.first())
        .and_then(|v| v.get_str())
        .ok_or_else(|| Error::VdfParse("missing PersonaName".into()))?;

    Ok(persona.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn write_localconfig(root: &Path, steamid: &str, name: Option<&str>, key: &str) {
        let user_dir = root.join("userdata").join(steamid).join("config");
        fs::create_dir_all(&user_dir).unwrap();
        let mut f = fs::File::create(user_dir.join("localconfig.vdf")).unwrap();
        write!(f, "\"UserLocalConfigStore\"\n{{\n\t\"{key}\"\n\t{{\n").unwrap();
        if let Some(n) = name {
            writeln!(f, "\t\t\"PersonaName\"\t\t\"{n}\"").unwrap();
        }
        write!(f, "\t}}\n}}\n").unwrap();
    }

    #[test]
    fn returns_steam_path_missing_when_no_userdata() {
        let tmp = TempDir::new().unwrap();
        // No userdata dir created.
        let err = enumerate_accounts(tmp.path()).unwrap_err();
        assert!(matches!(err, Error::SteamPathMissing(_)));
    }

    #[test]
    fn reads_friends_capitalized() {
        let tmp = TempDir::new().unwrap();
        write_localconfig(tmp.path(), "76561198000000001", Some("Alice"), "Friends");
        let accounts = enumerate_accounts(tmp.path()).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].steamid, "76561198000000001");
        assert_eq!(accounts[0].username, "Alice");
    }

    #[test]
    fn reads_friends_lowercase() {
        let tmp = TempDir::new().unwrap();
        write_localconfig(tmp.path(), "76561198000000002", Some("bob"), "friends");
        let accounts = enumerate_accounts(tmp.path()).unwrap();
        assert_eq!(accounts[0].username, "bob");
    }

    #[test]
    fn falls_back_to_unknown_when_persona_missing() {
        let tmp = TempDir::new().unwrap();
        write_localconfig(tmp.path(), "76561198000000003", None, "Friends");
        let accounts = enumerate_accounts(tmp.path()).unwrap();
        assert_eq!(accounts[0].username, "(unknown username)");
    }

    #[test]
    fn skips_account_dirs_without_localconfig() {
        let tmp = TempDir::new().unwrap();
        fs::create_dir_all(tmp.path().join("userdata").join("not_an_account")).unwrap();
        write_localconfig(tmp.path(), "76561198000000004", Some("Real"), "Friends");
        let accounts = enumerate_accounts(tmp.path()).unwrap();
        assert_eq!(accounts.len(), 1);
        assert_eq!(accounts[0].username, "Real");
    }
}
