use std::{fmt, io, error};
use serde_json::Error as SerdeError;

#[derive(Debug)]
pub enum Error {
    Parse(SerdeError),
    NoLayer,
    Io(io::Error),
    UnsupportedFormat(String),
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Parse(err) => write!(f, "Failed to parse JSON: {}", err),
            Error::NoLayer => write!(f, "No valid layer found"),
            Error::Io(err) => write!(f, "I/O error: {}", err),
            Error::UnsupportedFormat(ext) => write!(f, "Unsupported file format: {}", ext),
        }
    }
}

impl From<io::Error> for Error {
    fn from(err: io::Error) -> Self {
        Error::Io(err)
    }
}

impl From<SerdeError> for Error {
    fn from(err: SerdeError) -> Self {
        Error::Parse(err)
    }
}

impl error::Error for Error {}