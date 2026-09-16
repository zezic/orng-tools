// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Orange Catalog repository format.
//!
//! Orange Catalog is not a file host -- content is tens of kilobytes. It is an
//! identity authority: its job is to guarantee that a UUID means one thing,
//! permanently, across contributors who do not know each other. Every rule in
//! [`validate`] exists to hold that guarantee.
//!
//! The same types serve both sides. Continuous integration uses them to reject a
//! bad contribution; the application uses them to read what was published. One
//! implementation, so the two cannot disagree about what a valid item is.

pub mod index;
pub mod item;
pub mod manifest;
pub mod validate;

pub use index::{Index, IndexEntry};
pub use item::{AuthorId, Item, Slug, scan};
pub use manifest::Manifest;
pub use validate::{Problem, Report, Severity};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: {source}")]
    Manifest { path: String, source: toml::de::Error },
    #[error("{path}: {source}")]
    Document { path: String, source: bitwig_document::Error },
    #[error("malformed index: {0}")]
    Index(#[from] serde_json::Error),
    #[error("{0} is not a usable {1}: expected lowercase letters, digits and dashes")]
    BadIdentifier(String, &'static str),
    #[error("{0} is not a Bitwig release number")]
    BadVersion(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Directory holding every contributed item, relative to the repository root.
pub const CONTENT_DIR: &str = "devices";

/// File each item carries beside its document.
pub const MANIFEST_FILE: &str = "orange.toml";

pub(crate) fn read(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Io { path: path.display().to_string(), source })
}
