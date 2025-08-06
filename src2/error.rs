use std::fmt;
use std::io;
use nanoserde::DeJsonErr;
/// Error type for the basic map loader
#[derive(Debug)]
pub enum Error {
    /// JSON parse error
    Parse(DeJsonErr),
    /// No layers were found in the map JSON
    NoLayer,
    /// A layer's data length does not match width * height or map dimensions are zero
    InvalidLayerSize(String),
    /// File I/O error
    Io(io::Error),
    /// Unsupported file format (non-JSON)
    UnsupportedFormat(String),
}

impl From<DeJsonErr> for Error {
    fn from(err: DeJsonErr) -> Self {
        Error::Parse(err)
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(e) => write!(f, "JSON parse error: {}", e),
            Error::NoLayer => write!(f, "No layers found in map JSON"),
            Error::InvalidLayerSize(name) => write!(f, "Invalid layer size for layer '{}': data length does not match map dimensions", name),
            Error::Io(e) => write!(f, "I/O error: {}", e),
            Error::UnsupportedFormat(ext) => write!(f, "Unsupported file format: {}", ext),
        }
    }
}

impl std::error::Error for Error {}

