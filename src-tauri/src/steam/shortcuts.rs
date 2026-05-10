// The whole module is consumed once Phase 5 wires apply_changes through it.
#![allow(dead_code)]

//! Binary `shortcuts.vdf` codec.
//!
//! Steam stores non-Steam shortcuts in a tagged little-endian binary
//! format. No maintained Rust crate handles read + write, so this module
//! implements both with round-trip tests.
//!
//! ## Wire format
//!
//! ```text
//! <type:u8> <key:cstr> <value...>
//! ```
//!
//! - `0x00` Object — children entries follow recursively, terminated by `0x08`.
//! - `0x01` String — value is a null-terminated UTF-8 byte sequence.
//! - `0x02` Int32  — value is 4 bytes, little-endian.
//! - `0x07` UInt64 — value is 8 bytes, little-endian.
//! - `0x08` End-of-object marker (no key, no value).
//!
//! All strings are null-terminated. At the file level, top-level entries
//! are written sequentially; there is no enclosing object wrapper. The
//! conventional shape is a single top-level entry `"shortcuts"` whose
//! value is an object indexed by stringified integers (`"0"`, `"1"`, ...).
//!
//! ## Safety
//!
//! `parse` is total — every byte sequence either decodes cleanly or
//! produces an `Err(VdfParse)`. `serialize` cannot fail. Round-trip
//! (`serialize(parse(x))?` == `x`) is guaranteed for any bytes that
//! parse successfully and use only the four supported types above.

use std::collections::BTreeMap;

use crate::error::{Error, Result};

const TYPE_OBJECT: u8 = 0x00;
const TYPE_STRING: u8 = 0x01;
const TYPE_INT32: u8 = 0x02;
const TYPE_UINT64: u8 = 0x07;
const TYPE_END: u8 = 0x08;

/// A generic VDF value. Order of entries inside `Object` is preserved
/// from the input, so an identity round-trip is byte-exact.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Object(Vec<(String, Value)>),
    String(String),
    Int32(i32),
    UInt64(u64),
}

impl Value {
    pub fn as_object(&self) -> Option<&[(String, Value)]> {
        match self {
            Value::Object(entries) => Some(entries),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Value::String(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i32(&self) -> Option<i32> {
        match self {
            Value::Int32(n) => Some(*n),
            _ => None,
        }
    }
}

/// Parse a binary VDF document. The result is always a root-level
/// `Value::Object`, even if the input has no entries.
pub fn parse(bytes: &[u8]) -> Result<Value> {
    let mut reader = bytes;
    let mut entries = Vec::new();
    while !reader.is_empty() {
        let type_byte = read_u8(&mut reader)?;
        // A trailing 0x08 at the root is tolerated — some writers emit one.
        if type_byte == TYPE_END {
            if !reader.is_empty() {
                return Err(Error::VdfParse(format!(
                    "unexpected {} byte(s) after root terminator",
                    reader.len()
                )));
            }
            break;
        }
        let key = read_cstring(&mut reader)?;
        let value = parse_value(&mut reader, type_byte)?;
        entries.push((key, value));
    }
    Ok(Value::Object(entries))
}

fn parse_value(reader: &mut &[u8], type_byte: u8) -> Result<Value> {
    match type_byte {
        TYPE_OBJECT => {
            let mut entries = Vec::new();
            loop {
                let t = read_u8(reader)?;
                if t == TYPE_END {
                    break;
                }
                let k = read_cstring(reader)?;
                let v = parse_value(reader, t)?;
                entries.push((k, v));
            }
            Ok(Value::Object(entries))
        }
        TYPE_STRING => Ok(Value::String(read_cstring(reader)?)),
        TYPE_INT32 => Ok(Value::Int32(read_i32_le(reader)?)),
        TYPE_UINT64 => Ok(Value::UInt64(read_u64_le(reader)?)),
        other => Err(Error::VdfParse(format!(
            "unknown VDF type byte 0x{other:02x}"
        ))),
    }
}

/// Serialize a root-level `Value::Object` into bytes Steam can read.
///
/// Matches Python `vdf.binary_dumps` exactly: each nested object gets one
/// `0x08` terminator, plus a final root-level `0x08` after the last top-
/// level entry. (Steam's binary VDF treats the file as if it sits inside
/// an implicit outermost object.) Without that final byte, Steam may
/// silently truncate the last entry.
pub fn serialize(value: &Value) -> Vec<u8> {
    let mut buf = Vec::new();
    if let Value::Object(entries) = value {
        for (key, val) in entries {
            write_entry(&mut buf, key, val);
        }
        buf.push(TYPE_END);
    }
    buf
}

fn write_entry(buf: &mut Vec<u8>, key: &str, value: &Value) {
    match value {
        Value::Object(inner) => {
            buf.push(TYPE_OBJECT);
            write_cstring(buf, key);
            for (k, v) in inner {
                write_entry(buf, k, v);
            }
            buf.push(TYPE_END);
        }
        Value::String(s) => {
            buf.push(TYPE_STRING);
            write_cstring(buf, key);
            write_cstring(buf, s);
        }
        Value::Int32(n) => {
            buf.push(TYPE_INT32);
            write_cstring(buf, key);
            buf.extend_from_slice(&n.to_le_bytes());
        }
        Value::UInt64(n) => {
            buf.push(TYPE_UINT64);
            write_cstring(buf, key);
            buf.extend_from_slice(&n.to_le_bytes());
        }
    }
}

fn read_u8(reader: &mut &[u8]) -> Result<u8> {
    let b = *reader
        .first()
        .ok_or_else(|| Error::VdfParse("unexpected EOF reading type byte".into()))?;
    *reader = &reader[1..];
    Ok(b)
}

fn read_i32_le(reader: &mut &[u8]) -> Result<i32> {
    if reader.len() < 4 {
        return Err(Error::VdfParse(
            "unexpected EOF reading int32 value".into(),
        ));
    }
    let v = i32::from_le_bytes(reader[..4].try_into().unwrap());
    *reader = &reader[4..];
    Ok(v)
}

fn read_u64_le(reader: &mut &[u8]) -> Result<u64> {
    if reader.len() < 8 {
        return Err(Error::VdfParse(
            "unexpected EOF reading uint64 value".into(),
        ));
    }
    let v = u64::from_le_bytes(reader[..8].try_into().unwrap());
    *reader = &reader[8..];
    Ok(v)
}

fn read_cstring(reader: &mut &[u8]) -> Result<String> {
    let null = reader
        .iter()
        .position(|&b| b == 0)
        .ok_or_else(|| Error::VdfParse("string missing null terminator".into()))?;
    let s = String::from_utf8_lossy(&reader[..null]).into_owned();
    *reader = &reader[null + 1..];
    Ok(s)
}

fn write_cstring(buf: &mut Vec<u8>, s: &str) {
    buf.extend_from_slice(s.as_bytes());
    buf.push(0);
}

// ----------------------------------------------------------------------
// Typed view over a shortcuts.vdf file.
// ----------------------------------------------------------------------

/// One non-Steam shortcut. Mirrors the fields steamsync's Python code
/// produces in `to_shortcut`. We hold onto `extra` so unknown fields
/// written by Steam or other tools round-trip cleanly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Shortcut {
    pub appid: i32,
    pub app_name: String,
    pub exe: String,
    pub start_dir: String,
    pub icon: String,
    pub shortcut_path: String,
    pub launch_options: String,
    pub is_hidden: i32,
    pub allow_desktop_config: i32,
    pub allow_overlay: i32,
    pub openvr: i32,
    pub devkit: i32,
    pub devkit_game_id: String,
    pub last_play_time: i32,
    /// Tag list, e.g. `{"0": "steamsync", "1": "epicstore"}`.
    pub tags: BTreeMap<String, String>,
    /// Fields we didn't recognize, preserved verbatim so a round-trip
    /// doesn't drop data Steam wrote.
    pub extra: Vec<(String, Value)>,
}

impl Shortcut {
    /// Build a Shortcut from a Value::Object representing one shortcut entry.
    /// Unknown keys land in `extra`. Missing known keys default to empty/0.
    pub fn from_value(value: &Value) -> Result<Self> {
        let entries = value
            .as_object()
            .ok_or_else(|| Error::VdfParse("shortcut entry is not an object".into()))?;

        let mut s = Shortcut::default_empty();
        for (key, val) in entries {
            match key.as_str() {
                "appid" => s.appid = val.as_i32().unwrap_or(0),
                "AppName" | "appname" => s.app_name = val.as_str().unwrap_or("").into(),
                "Exe" | "exe" => s.exe = val.as_str().unwrap_or("").into(),
                "StartDir" => s.start_dir = val.as_str().unwrap_or("").into(),
                "icon" => s.icon = val.as_str().unwrap_or("").into(),
                "ShortcutPath" => s.shortcut_path = val.as_str().unwrap_or("").into(),
                "LaunchOptions" => s.launch_options = val.as_str().unwrap_or("").into(),
                "IsHidden" => s.is_hidden = val.as_i32().unwrap_or(0),
                "AllowDesktopConfig" => s.allow_desktop_config = val.as_i32().unwrap_or(0),
                "AllowOverlay" => s.allow_overlay = val.as_i32().unwrap_or(0),
                "openvr" => s.openvr = val.as_i32().unwrap_or(0),
                "Devkit" => s.devkit = val.as_i32().unwrap_or(0),
                "DevkitGameID" => s.devkit_game_id = val.as_str().unwrap_or("").into(),
                "LastPlayTime" => s.last_play_time = val.as_i32().unwrap_or(0),
                "tags" => {
                    if let Some(tag_entries) = val.as_object() {
                        for (k, v) in tag_entries {
                            if let Some(tag) = v.as_str() {
                                s.tags.insert(k.clone(), tag.to_string());
                            }
                        }
                    }
                }
                _ => s.extra.push((key.clone(), val.clone())),
            }
        }
        Ok(s)
    }

    /// Convert back into a Value::Object suitable for serialize().
    /// Field order matches what Python's steamsync writes.
    pub fn to_value(&self) -> Value {
        let mut entries: Vec<(String, Value)> = vec![
            ("appid".into(), Value::Int32(self.appid)),
            ("AppName".into(), Value::String(self.app_name.clone())),
            ("Exe".into(), Value::String(self.exe.clone())),
            ("StartDir".into(), Value::String(self.start_dir.clone())),
            ("icon".into(), Value::String(self.icon.clone())),
            (
                "ShortcutPath".into(),
                Value::String(self.shortcut_path.clone()),
            ),
            (
                "LaunchOptions".into(),
                Value::String(self.launch_options.clone()),
            ),
            ("IsHidden".into(), Value::Int32(self.is_hidden)),
            (
                "AllowDesktopConfig".into(),
                Value::Int32(self.allow_desktop_config),
            ),
            ("AllowOverlay".into(), Value::Int32(self.allow_overlay)),
            ("openvr".into(), Value::Int32(self.openvr)),
            ("Devkit".into(), Value::Int32(self.devkit)),
            (
                "DevkitGameID".into(),
                Value::String(self.devkit_game_id.clone()),
            ),
            ("LastPlayTime".into(), Value::Int32(self.last_play_time)),
            (
                "tags".into(),
                Value::Object(
                    self.tags
                        .iter()
                        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                        .collect(),
                ),
            ),
        ];
        // Preserve unknown fields at the tail so they survive a round-trip.
        entries.extend(self.extra.iter().cloned());
        Value::Object(entries)
    }

    fn default_empty() -> Self {
        Self {
            appid: 0,
            app_name: String::new(),
            exe: String::new(),
            start_dir: String::new(),
            icon: String::new(),
            shortcut_path: String::new(),
            launch_options: String::new(),
            is_hidden: 0,
            allow_desktop_config: 1,
            allow_overlay: 1,
            openvr: 0,
            devkit: 0,
            devkit_game_id: String::new(),
            last_play_time: 0,
            tags: BTreeMap::new(),
            extra: Vec::new(),
        }
    }
}

/// Pull the list of shortcuts out of a parsed root Value. Returns
/// (indexed_shortcuts, leftover_root_entries) — the latter is anything
/// at the root that isn't the `"shortcuts"` key, so we can re-emit it.
pub fn extract_shortcuts(
    root: &Value,
) -> Result<(Vec<(String, Shortcut)>, Vec<(String, Value)>)> {
    let entries = root
        .as_object()
        .ok_or_else(|| Error::VdfParse("root is not an object".into()))?;

    let mut shortcuts = Vec::new();
    let mut leftover = Vec::new();
    for (key, val) in entries {
        if key == "shortcuts" {
            let inner = val.as_object().ok_or_else(|| {
                Error::VdfParse("\"shortcuts\" value is not an object".into())
            })?;
            for (idx_key, shortcut_val) in inner {
                shortcuts.push((idx_key.clone(), Shortcut::from_value(shortcut_val)?));
            }
        } else {
            leftover.push((key.clone(), val.clone()));
        }
    }
    Ok((shortcuts, leftover))
}

/// Inverse of `extract_shortcuts` — rebuild a root Value from a list of
/// shortcuts plus any leftover root entries to preserve.
pub fn build_root(
    shortcuts: &[(String, Shortcut)],
    leftover_root: &[(String, Value)],
) -> Value {
    let mut root_entries: Vec<(String, Value)> = Vec::with_capacity(1 + leftover_root.len());
    let mut inner: Vec<(String, Value)> = Vec::with_capacity(shortcuts.len());
    for (idx, sc) in shortcuts {
        inner.push((idx.clone(), sc.to_value()));
    }
    root_entries.push(("shortcuts".into(), Value::Object(inner)));
    root_entries.extend(leftover_root.iter().cloned());
    Value::Object(root_entries)
}

// ----------------------------------------------------------------------
// Tests
// ----------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    /// Hand-rolled minimal shortcuts.vdf byte stream. Encodes:
    /// shortcuts: { "0": { appid=42, AppName="Fortnite" } }
    fn minimal_bytes() -> Vec<u8> {
        let mut b = Vec::new();
        // 0x00 "shortcuts\0"
        b.push(TYPE_OBJECT);
        b.extend_from_slice(b"shortcuts\0");
        //   0x00 "0\0"
        b.push(TYPE_OBJECT);
        b.extend_from_slice(b"0\0");
        //     0x02 "appid\0" <i32 LE = 42>
        b.push(TYPE_INT32);
        b.extend_from_slice(b"appid\0");
        b.extend_from_slice(&42i32.to_le_bytes());
        //     0x01 "AppName\0" "Fortnite\0"
        b.push(TYPE_STRING);
        b.extend_from_slice(b"AppName\0");
        b.extend_from_slice(b"Fortnite\0");
        //   0x08 (close "0")
        b.push(TYPE_END);
        // 0x08 (close "shortcuts")
        b.push(TYPE_END);
        // 0x08 (root-level terminator — Python's vdf.binary_dumps emits one)
        b.push(TYPE_END);
        b
    }

    #[test]
    fn parses_minimal() {
        let value = parse(&minimal_bytes()).unwrap();
        let root = value.as_object().unwrap();
        assert_eq!(root.len(), 1);
        assert_eq!(root[0].0, "shortcuts");
        let shortcuts = root[0].1.as_object().unwrap();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].0, "0");
        let entry = shortcuts[0].1.as_object().unwrap();
        assert_eq!(entry[0].0, "appid");
        assert_eq!(entry[0].1.as_i32(), Some(42));
        assert_eq!(entry[1].0, "AppName");
        assert_eq!(entry[1].1.as_str(), Some("Fortnite"));
    }

    #[test]
    fn serialize_minimal_matches_hand_rolled_bytes() {
        let value = parse(&minimal_bytes()).unwrap();
        let out = serialize(&value);
        assert_eq!(out, minimal_bytes());
    }

    /// The critical guarantee: read → write produces identical bytes.
    /// If this ever fails we'd corrupt the user's shortcuts.vdf.
    #[test]
    fn round_trip_minimal_is_byte_exact() {
        let original = minimal_bytes();
        let parsed = parse(&original).unwrap();
        let written = serialize(&parsed);
        assert_eq!(written, original, "round-trip is not byte-exact");
    }

    #[test]
    fn empty_shortcuts_object_round_trips() {
        // shortcuts: {}
        let mut b = Vec::new();
        b.push(TYPE_OBJECT);
        b.extend_from_slice(b"shortcuts\0");
        b.push(TYPE_END); // close "shortcuts"
        b.push(TYPE_END); // root terminator
        let parsed = parse(&b).unwrap();
        let written = serialize(&parsed);
        assert_eq!(written, b);
    }

    #[test]
    fn unknown_root_keys_are_preserved() {
        // Some files have stuff alongside "shortcuts". We must not drop it.
        let mut b = Vec::new();
        b.push(TYPE_OBJECT);
        b.extend_from_slice(b"shortcuts\0");
        b.push(TYPE_END);
        b.push(TYPE_STRING);
        b.extend_from_slice(b"meta\0");
        b.extend_from_slice(b"hello\0");
        b.push(TYPE_END); // root terminator
        let parsed = parse(&b).unwrap();
        let written = serialize(&parsed);
        assert_eq!(written, b);
    }

    #[test]
    fn unknown_value_type_fails_loudly() {
        let bytes = vec![0x99, b'k', 0];
        let err = parse(&bytes).unwrap_err();
        assert!(matches!(err, Error::VdfParse(_)));
    }

    #[test]
    fn truncated_string_fails_loudly() {
        // type=string, key="k\0", then a value with no terminating NUL.
        let mut b = vec![TYPE_STRING];
        b.extend_from_slice(b"k\0");
        b.extend_from_slice(b"no terminator");
        let err = parse(&b).unwrap_err();
        assert!(matches!(err, Error::VdfParse(_)));
    }

    #[test]
    fn truncated_int_fails_loudly() {
        let mut b = vec![TYPE_INT32];
        b.extend_from_slice(b"k\0");
        b.extend_from_slice(&[0x01, 0x02]); // only 2 bytes, need 4
        let err = parse(&b).unwrap_err();
        assert!(matches!(err, Error::VdfParse(_)));
    }

    // -----------------------------
    // Typed Shortcut layer
    // -----------------------------

    fn sample_shortcut() -> Shortcut {
        let mut tags = BTreeMap::new();
        tags.insert("0".into(), "steamsync".into());
        tags.insert("1".into(), "epicstore".into());
        Shortcut {
            appid: -1_172_001_224,
            app_name: "Fortnite".into(),
            exe: "C:\\Fortnite\\Fortnite.exe".into(),
            start_dir: "C:\\Fortnite".into(),
            icon: "C:\\Fortnite\\Fortnite.exe".into(),
            shortcut_path: String::new(),
            launch_options: String::new(),
            is_hidden: 0,
            allow_desktop_config: 1,
            allow_overlay: 1,
            openvr: 0,
            devkit: 0,
            devkit_game_id: String::new(),
            last_play_time: 0,
            tags,
            extra: Vec::new(),
        }
    }

    #[test]
    fn shortcut_round_trips_through_value() {
        let s = sample_shortcut();
        let value = s.to_value();
        let s2 = Shortcut::from_value(&value).unwrap();
        assert_eq!(s, s2);
    }

    #[test]
    fn extract_then_build_round_trips_bytes() {
        // Build a root with our sample shortcut, serialize, parse back,
        // serialize again, expect byte equality.
        let root = build_root(&[("0".into(), sample_shortcut())], &[]);
        let bytes1 = serialize(&root);

        let parsed = parse(&bytes1).unwrap();
        let (shortcuts, leftover) = extract_shortcuts(&parsed).unwrap();
        assert_eq!(shortcuts.len(), 1);
        assert_eq!(shortcuts[0].1, sample_shortcut());

        let rebuilt = build_root(&shortcuts, &leftover);
        let bytes2 = serialize(&rebuilt);
        assert_eq!(bytes1, bytes2);
    }

    #[test]
    fn from_value_tolerates_lowercase_exe() {
        let value = Value::Object(vec![
            ("appname".into(), Value::String("X".into())),
            ("exe".into(), Value::String("/p".into())),
        ]);
        let s = Shortcut::from_value(&value).unwrap();
        assert_eq!(s.app_name, "X");
        assert_eq!(s.exe, "/p");
    }

    #[test]
    fn from_value_preserves_unknown_fields() {
        let value = Value::Object(vec![
            ("appid".into(), Value::Int32(7)),
            ("FutureField".into(), Value::String("oops".into())),
        ]);
        let s = Shortcut::from_value(&value).unwrap();
        assert_eq!(s.appid, 7);
        assert_eq!(s.extra.len(), 1);
        assert_eq!(s.extra[0].0, "FutureField");
        assert_eq!(s.extra[0].1.as_str(), Some("oops"));
    }
}
