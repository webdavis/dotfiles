use std::{fmt, io};

#[derive(Debug)]
pub enum StoreError {
    Io(io::Error),
    Database(rusqlite::Error),
    UnsafeFile,
    InvalidState(String),
    UnsupportedSchema(u32),
}
impl fmt::Display for StoreError {
    fn fmt(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(error) => write!(out, "state file: {error}"),
            Self::Database(error) => write!(out, "state database: {error}"),
            Self::InvalidState(complaint) => out.write_str(complaint),
            Self::UnsafeFile => out.write_str("state database requires private regular files"),
            Self::UnsupportedSchema(version) => write!(out, "unsupported state schema {version}"),
        }
    }
}
impl std::error::Error for StoreError {}
impl From<io::Error> for StoreError {
    fn from(error: io::Error) -> Self {
        Self::Io(error)
    }
}
impl From<rusqlite::Error> for StoreError {
    fn from(error: rusqlite::Error) -> Self {
        Self::Database(error)
    }
}
