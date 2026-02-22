use std::error::Error;
use std::fmt::{Display, Formatter};
use std::path::PathBuf;

/// Typed error returned by map loading/parsing APIs.
#[derive(Debug)]
pub enum MapError {
    /// File read/write failure with source path.
    Io {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        source: std::io::Error,
    },
    /// JSON parse/deserialization failure with source path.
    Json {
        /// Path containing invalid JSON.
        path: PathBuf,
        /// Underlying JSON error.
        source: serde_json::Error,
    },
    /// Invalid map contract or unsupported format for current version.
    InvalidMap(String),
    /// Non-UTF-8 path encountered where UTF-8 is required by API surface.
    InvalidUtf8Path(PathBuf),
    /// Unsupported explicit property type encountered in JSON.
    UnsupportedPropertyType {
        /// Property name.
        name: String,
        /// Property type string.
        kind: String,
    },
    /// Tile layer references a gid outside known tileset range.
    InvalidTileGid {
        /// Layer name.
        layer: String,
        /// Invalid gid.
        gid: u32,
        /// Maximum valid gid.
        max_gid: u32,
    },
    /// Object tile reference (`gid`) is outside known tileset range.
    InvalidObjectGid {
        /// Layer name.
        layer: String,
        /// Object id.
        object_id: u32,
        /// Invalid gid.
        gid: u32,
        /// Maximum valid gid.
        max_gid: u32,
    },
    /// Texture load failure for a tileset image.
    TextureLoad {
        /// Texture path.
        path: PathBuf,
        /// Error message from backend loader.
        message: String,
    },
}

impl Display for MapError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            MapError::Io { path, source } => {
                write!(f, "I/O error at {}: {}", path.display(), source)
            }
            MapError::Json { path, source } => {
                write!(f, "JSON parse error at {}: {}", path.display(), source)
            }
            MapError::InvalidMap(msg) => write!(f, "Invalid map: {msg}"),
            MapError::InvalidUtf8Path(path) => {
                write!(f, "Path is not valid UTF-8: {}", path.display())
            }
            MapError::UnsupportedPropertyType { name, kind } => {
                write!(
                    f,
                    "Unsupported property type '{}' for property '{}'",
                    kind, name
                )
            }
            MapError::InvalidTileGid {
                layer,
                gid,
                max_gid,
            } => write!(
                f,
                "Invalid tile gid {} in layer '{}'; max known gid is {}",
                gid, layer, max_gid
            ),
            MapError::InvalidObjectGid {
                layer,
                object_id,
                gid,
                max_gid,
            } => write!(
                f,
                "Invalid object tile gid {} in layer '{}' object id {}; max known gid is {}",
                gid, layer, object_id, max_gid
            ),
            MapError::TextureLoad { path, message } => {
                write!(f, "Failed to load texture {}: {}", path.display(), message)
            }
        }
    }
}

impl Error for MapError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            MapError::Io { source, .. } => Some(source),
            MapError::Json { source, .. } => Some(source),
            _ => None,
        }
    }
}
