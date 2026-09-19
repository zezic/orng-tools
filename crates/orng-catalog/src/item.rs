// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! One contributed item, and how the repository tree maps onto it.
//!
//! ```text
//! content/<author>/<slug>/<name>.bwdevice
//! content/<author>/<slug>/orng.toml
//! ```
//!
//! Author-first grouping is what makes per-directory ownership expressible:
//! GitHub has no path-scoped write permission, so ownership is enforced by
//! comparing a pull request's changed paths against an owners file. That check
//! is only as clear as the layout it reads.

use std::path::{Path, PathBuf};

use bitwig_document::{Document, Identity, Kind};
use serde::{Deserialize, Serialize};

use crate::{CONTENT_DIR, Error, MANIFEST_FILE, Manifest, Result};

/// A contributor, and the directory their content lives in.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AuthorId(String);

/// An item's stable directory name within its author's folder.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Slug(String);

/// Both are directory names and both appear in URLs, so the same rule applies:
/// lowercase, digits and dashes, starting with a letter or digit.
fn parse_identifier(text: &str, what: &'static str) -> Result<String> {
    let shaped = !text.is_empty()
        && text.len() <= 64
        && text.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-')
        && !text.starts_with('-')
        && !text.ends_with('-')
        && !text.contains("--");
    shaped
        .then(|| text.to_owned())
        .ok_or_else(|| Error::BadIdentifier(text.to_owned(), what))
}

macro_rules! identifier {
    ($type:ty, $what:literal) => {
        impl $type {
            pub fn new(text: &str) -> Result<Self> {
                parse_identifier(text, $what).map(Self)
            }

            pub fn as_str(&self) -> &str {
                &self.0
            }
        }

        impl TryFrom<String> for $type {
            type Error = Error;

            fn try_from(text: String) -> Result<Self> {
                Self::new(&text)
            }
        }

        impl From<$type> for String {
            fn from(value: $type) -> String {
                value.0
            }
        }

        impl std::fmt::Display for $type {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(&self.0)
            }
        }
    };
}

identifier!(AuthorId, "author id");
identifier!(Slug, "slug");

/// The SHA-256 of a document, as the index publishes it and as a registration
/// records it.
///
/// A newtype rather than a `String` because the only thing ever done with one is
/// compare it against another, and two spellings of the same hash - upper case,
/// or a paste that lost a character - compare unequal while looking identical
/// wherever a person is reading. Refused on the way in for the same reason the
/// [`Revision`](crate::Revision) beside it is: a digest that cannot be one would
/// otherwise sit in a published index quietly never matching.
///
/// One type for both sides on purpose. The catalog states what a document should
/// hash to and the entry list records what a registered one did, and the whole
/// value of either is that they can be held up against each other.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Digest(String);

impl Digest {
    /// Sixty-four lowercase hex digits, and nothing else: [`Digest::of`] is the
    /// only writer here and that is the form it produces.
    pub fn new(text: &str) -> Result<Self> {
        let shaped = text.len() == 64 && text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
        shaped.then(|| Digest(text.to_owned())).ok_or_else(|| Error::BadDigest(text.to_owned()))
    }

    /// Hash some bytes. The one place this project computes a content digest.
    pub fn of(bytes: &[u8]) -> Self {
        use sha2::{Digest as _, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Digest(crate::hex(&hasher.finalize()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Digest {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::new(&text)
    }
}

impl From<Digest> for String {
    fn from(value: Digest) -> String {
        value.0
    }
}

impl std::fmt::Display for Digest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// An item as it exists in the repository tree: what the manifest declares, plus
/// what its document says about itself.
#[derive(Debug, Clone)]
pub struct Item {
    pub author: AuthorId,
    pub slug: Slug,
    pub manifest: Manifest,
    pub kind: Kind,
    pub identity: Identity,
    /// Repository-relative path of the document, with forward slashes.
    pub document_path: String,
    pub document: Vec<u8>,
}

impl Item {
    /// Read one item from its directory.
    pub fn read(root: &Path, author: AuthorId, slug: Slug) -> Result<Self> {
        let dir = root.join(CONTENT_DIR).join(author.as_str()).join(slug.as_str());
        let manifest = Manifest::read(&dir.join(MANIFEST_FILE))?;

        let document_file = find_document(&dir)?;
        let bytes = crate::read(&document_file)?;
        let kind = Kind::from_path(&document_file).expect("find_document only yields known kinds");
        // No key is offered, and none is needed: everything the catalog carries
        // is text or plain binary. An encrypted document is Bitwig's own, and
        // is refused by name rather than by the parse failure it would become.
        let parsed = Document::parse(kind, bytes.clone()).map_err(|source| match source {
            bitwig_document::Error::Encrypted => {
                Error::FactoryContent { path: document_file.display().to_string() }
            }
            source => Error::Document { path: document_file.display().to_string(), source },
        })?;

        let name = document_file.file_name().unwrap_or_default().to_string_lossy();
        Ok(Item {
            document_path: format!("{}/{name}", item_dir(&author, &slug)),
            author,
            slug,
            manifest,
            kind,
            identity: parsed.identity().clone(),
            document: bytes,
        })
    }

    /// Repository-relative directory of the item, with forward slashes.
    ///
    /// The unit both ownership and history are expressed in: the owners check
    /// matches a pull request's paths against it, and git is asked what last
    /// changed it.
    pub fn dir(&self) -> String {
        item_dir(&self.author, &self.slug)
    }

    /// Content hash, as published in the index and checked after download.
    pub fn digest(&self) -> Digest {
        Digest::of(&self.document)
    }
}

/// The one place the tree's shape is written down, so a path built for git and a
/// path published in the index cannot drift apart.
fn item_dir(author: &AuthorId, slug: &Slug) -> String {
    format!("{CONTENT_DIR}/{author}/{slug}")
}

/// The single document in an item directory.
///
/// Exactly one: a directory holding two devices has no single identity, and the
/// index could not describe it.
fn find_document(dir: &Path) -> Result<PathBuf> {
    let entries = std::fs::read_dir(dir)
        .map_err(|source| Error::Io { path: dir.display().to_string(), source })?;
    let mut found: Vec<PathBuf> = entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| Kind::from_path(p).is_some())
        .collect();
    found.sort();
    match found.len() {
        1 => Ok(found.remove(0)),
        _ => Err(Error::Io {
            path: dir.display().to_string(),
            source: std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!("expected exactly one document, found {}", found.len()),
            ),
        }),
    }
}

/// Read every item in a repository checkout.
///
/// Returns the items that parsed and the failures separately: one broken
/// contribution must not hide the state of the rest, which is what a validator
/// run needs to report.
pub fn scan(root: &Path) -> (Vec<Item>, Vec<Error>) {
    let mut items = Vec::new();
    let mut failures = Vec::new();

    let content = root.join(CONTENT_DIR);
    let Ok(authors) = std::fs::read_dir(&content) else {
        return (items, failures);
    };

    for author_dir in authors.flatten().filter(|e| e.path().is_dir()) {
        let author = match AuthorId::new(&author_dir.file_name().to_string_lossy()) {
            Ok(author) => author,
            Err(e) => {
                failures.push(e);
                continue;
            }
        };
        let Ok(slugs) = std::fs::read_dir(author_dir.path()) else { continue };
        for slug_dir in slugs.flatten().filter(|e| e.path().is_dir()) {
            let slug = match Slug::new(&slug_dir.file_name().to_string_lossy()) {
                Ok(slug) => slug,
                Err(e) => {
                    failures.push(e);
                    continue;
                }
            };
            match Item::read(root, author.clone(), slug) {
                Ok(item) => items.push(item),
                Err(e) => failures.push(e),
            }
        }
    }

    items.sort_by(|a, b| (&a.author, &a.slug).cmp(&(&b.author, &b.slug)));
    (items, failures)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identifiers_are_directory_and_url_safe() {
        assert!(AuthorId::new("example").is_ok());
        assert!(Slug::new("glue-comp").is_ok());
        assert!(Slug::new("glue-comp-2").is_ok());

        for bad in ["Example", "glue comp", "glue_comp", "-lead", "trail-", "a--b", ""] {
            assert!(Slug::new(bad).is_err(), "accepted {bad:?}");
        }
    }

    /// The published value, against a hash taken somewhere else entirely:
    /// `printf '' | shasum -a 256`. A digest this project computes differently
    /// from the rest of the world is one nothing can be checked against.
    #[test]
    fn a_digest_is_a_sha_256_in_lowercase_hex() {
        let empty = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert_eq!(Digest::of(b"").as_str(), empty);
        assert_eq!(Digest::of(b""), Digest::new(empty).unwrap());
    }

    /// Refused on the way in, because the only thing a digest is for is being
    /// compared: one that cannot be a hash would never match and never say why.
    #[test]
    fn a_digest_that_cannot_be_one_is_refused_rather_than_stored() {
        let real = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
        assert!(Digest::new(real).is_ok());
        // Upper case is the one that matters: it is a real hash of the real
        // bytes, written the other way round, and it compares unequal.
        assert!(Digest::new(&real.to_uppercase()).is_err());
        assert!(Digest::new(&real[..63]).is_err());
        assert!(Digest::new(&format!("{real}0")).is_err());
        assert!(Digest::new("").is_err());
        assert!(Digest::new(&"z".repeat(64)).is_err());
    }
}
