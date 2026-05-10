//! Port of `_get_steam_shortcut_id` from steamsync/defs.py.
//!
//! Steam used to derive a shortcut's appid from CRC32(exe || appname) with
//! the high bit set. Newer Steam clients persist a generated id in
//! shortcuts.vdf, but we still need to produce one on first add so we can
//! drop art into the grid folder before Steam has seen the shortcut.

// Used by Phase 2 (shortcuts.vdf writer) and Phase 3 (launchers).
#[allow(dead_code)]
pub fn shortcut_id_unsigned(exe: &str, app_name: &str) -> u32 {
    let mut hasher = crc32fast::Hasher::new();
    hasher.update(exe.as_bytes());
    hasher.update(app_name.as_bytes());
    hasher.finalize() | 0x8000_0000
}

#[allow(dead_code)]
pub fn shortcut_id_signed(exe: &str, app_name: &str) -> i32 {
    // Reinterpret the bit pattern (equivalent to Python's ctypes.c_int trick).
    shortcut_id_unsigned(exe, app_name) as i32
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Reference values produced by the Python implementation:
    ///
    ///   import binascii, ctypes
    ///   ctypes.c_int(binascii.crc32((exe + name).encode()) | 0x80000000).value
    ///
    /// These are baked in so any drift from the Python algorithm fails loudly.
    /// A shortcut whose id changes loses its grid art.
    #[test]
    fn matches_python_reference() {
        assert_eq!(
            shortcut_id_signed("C:\\Fortnite\\Fortnite.exe", "Fortnite"),
            -1_172_001_224,
        );
        assert_eq!(shortcut_id_signed("", ""), i32::MIN);
        assert_eq!(shortcut_id_signed("a", "b"), -1_635_563_411);
        assert_eq!(shortcut_id_signed("legendary", "Fortnite"), -282_289_763);
    }

    #[test]
    fn unsigned_and_signed_are_bit_equivalent() {
        let s = shortcut_id_signed("a", "b");
        let u = shortcut_id_unsigned("a", "b");
        assert_eq!(s as u32, u);
    }

    #[test]
    fn high_bit_is_always_set() {
        for (exe, name) in [("a", "b"), ("", ""), ("z".repeat(100).as_str(), "x")] {
            assert_ne!(shortcut_id_unsigned(exe, name) & 0x8000_0000, 0);
        }
    }
}
