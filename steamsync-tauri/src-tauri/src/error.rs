use thiserror::Error;

/// All errors that can bubble out of a Tauri command. They serialize to a
/// JSON string for the frontend (Tauri's default behavior for Err(_)).
#[derive(Debug, Error)]
pub enum Error {
    #[error("io: {0}")]
    Io(#[from] std::io::Error),

    #[error("Steam install not found at {0}. Use the Detect view's path field to point at a non-standard install.")]
    SteamPathMissing(String),

    #[error("Failed to parse Steam config: {0}")]
    VdfParse(String),
}

impl serde::Serialize for Error {
    fn serialize<S>(&self, serializer: S) -> std::result::Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;
