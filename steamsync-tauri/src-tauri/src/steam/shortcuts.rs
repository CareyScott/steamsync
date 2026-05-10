//! Binary `shortcuts.vdf` codec (Phase 2 — not yet implemented).
//!
//! Steam stores non-Steam shortcuts in a binary key-value format. There is
//! no maintained Rust crate that handles read + write, so we'll roll a
//! minimal codec in Phase 2 and validate it with round-trip tests against
//! a real `shortcuts.vdf` captured from a live Steam install.
//!
//! Until then, every Apply path must fail loudly rather than touch the
//! user's library.
