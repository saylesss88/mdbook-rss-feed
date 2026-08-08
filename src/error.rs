//! Crate-wide error type.

use std::path::PathBuf;
use thiserror::Error;

/// Errors produced while collecting articles or building feeds.
#[derive(Debug, Error)]
pub enum FeedError {
    /// Failed to read a file from disk.
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    /// Walking the `src` directory failed.
    #[error("failed to walk directory {path}: {source}")]
    WalkDir {
        path: PathBuf,
        #[source]
        source: walkdir::Error,
    },
}

pub type Result<T> = std::result::Result<T, FeedError>;
