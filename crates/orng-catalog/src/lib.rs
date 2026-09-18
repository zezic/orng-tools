// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The ORNG Catalog repository format.
//!
//! ORNG Catalog is not a file host -- content is tens of kilobytes. It is an
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
pub mod owners;
pub mod signing;
pub mod validate;

pub use index::{History, Index, IndexEntry, Revision};
pub use item::{AuthorId, Item, Slug, scan};
pub use manifest::Manifest;
pub use owners::{Authorization, Owner, Owners, Refusal, authorize};
pub use signing::{PublicKey, SecretKey, Signature};
pub use validate::{Problem, Report, Severity};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("{path}: {source}")]
    Manifest { path: String, source: toml::de::Error },
    #[error("{path}: {source}")]
    Document { path: String, source: bitwig_document::Error },
    /// The catalog carries what people make, and an encrypted document is not
    /// that: the `0004` serialization is what Bitwig writes for its own factory
    /// content. Accepting one would be redistributing Bitwig's assets through
    /// this repository, which the project does not do - so this is refused as a
    /// rule rather than worked around with a key.
    #[error(
        "{path} is Bitwig factory content, which the catalog does not carry. \
         Publish a device, modulator or Grid module you made yourself."
    )]
    FactoryContent { path: String },
    #[error("malformed index: {0}")]
    Index(#[from] serde_json::Error),
    #[error("{0} is not a usable {1}: expected lowercase letters, digits and dashes")]
    BadIdentifier(String, &'static str),
    #[error("{0} is not a Bitwig release number")]
    BadVersion(String),
    #[error("{0} is not a commit: expected forty lowercase hex digits")]
    BadRevision(String),
    /// Deliberately holds no copy of what it refused. One thing parsed through
    /// here is the signing key, and an error message goes to a workflow log.
    #[error("malformed {what}: {why}")]
    BadKeyMaterial { what: &'static str, why: &'static str },
    #[error("the signature does not match this index under this key")]
    SignatureMismatch,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Directory holding every contributed item, relative to the repository root.
///
/// One root for all three kinds, grouped by author rather than by kind. A
/// document already states its own kind and the index republishes it, so a path
/// that stated it too would be a third copy to keep in step. Grouping by author
/// is also what per-directory ownership is checked against, and an author owns
/// one prefix rather than three.
pub const CONTENT_DIR: &str = "content";

/// File each item carries beside its document.
pub const MANIFEST_FILE: &str = "orng.toml";

pub(crate) fn read(path: &std::path::Path) -> Result<Vec<u8>> {
    std::fs::read(path).map_err(|source| Error::Io { path: path.display().to_string(), source })
}

/// Lowercase hex, which is the form of every byte string this project publishes:
/// a content digest, a commit, a public key, a signature.
pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
