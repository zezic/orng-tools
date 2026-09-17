// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading and re-identifying Bitwig `.bwdevice`, `.bwmodulator` and
//! `.bwmodule` documents.
//!
//! Scope is deliberately narrow: the identity a registry entry needs, and the
//! ability to give a document a new UUID. The document body is walked for its
//! structure but never modelled.
//!
//! Bitwig writes three serializations and all of them turn up in the wild, so
//! all three are read. In every one the identity is a fixed-width value, which
//! is what lets a new UUID be spliced in without re-serializing the document.

mod cipher;
mod kind;
pub mod ramona;
mod text;
mod version;

use std::path::Path;

use uuid::Uuid;

pub use kind::{Kind, descriptions_key};
pub use version::BitwigVersion;
use ramona::{FieldKey, Fields as BinaryFields, Scanner, Value};

/// Bytes between the metadata and body sections of a binary document: 5000
/// spaces and a newline.
const BINARY_PADDING: usize = 5001;
const HEADER_LEN: usize = 42;

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("not a Bitwig document (bad magic)")]
    BadMagic,
    #[error("unsupported serialization format {0}")]
    UnsupportedFormat(u32),
    #[error("{0} is not a Bitwig device, modulator or Grid module")]
    UnknownKind(String),
    #[error("document truncated at offset {at}")]
    Truncated { at: usize },
    #[error("implausible length {len} at offset {at}")]
    BadLength { at: usize, len: i32 },
    #[error("unknown value tag {tag} at offset {at}")]
    UnknownTag { at: usize, tag: u8 },
    #[error("metadata field {0} is missing")]
    MissingField(&'static str),
    #[error("replacement is {got} bytes, must be {expected}")]
    LengthChanged { expected: usize, got: usize },
}

pub type Result<T> = std::result::Result<T, Error>;

const F_UUID: &str = "device_uuid";
const F_ID: &str = "device_id";
const F_NAME: &str = "device_name";
const F_DESCRIPTION: &str = "device_description";
const F_CATEGORY: &str = "device_category";
const F_CREATOR: &str = "creator";

/// The identity is stored twice; the body numbers the field the metadata names.
const BODY_UUID: i32 = 6369;

/// How a document's sections are encoded.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Serialization {
    /// Relaxed JSON. Written by development builds and by some authoring tools.
    Text,
    /// Ramona binary, no encryption. Also what `.bwpreset` uses.
    PlainBinary,
    /// Ramona binary behind the Dag stream cipher. Factory content.
    EncryptedBinary,
}

impl Serialization {
    fn from_header(value: u32) -> Result<Self> {
        match value {
            1 => Ok(Serialization::Text),
            2 => Ok(Serialization::PlainBinary),
            4 => Ok(Serialization::EncryptedBinary),
            other => Err(Error::UnsupportedFormat(other)),
        }
    }
}

/// What a registry entry needs to know about a document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Identity {
    pub uuid: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub category: Option<String>,
    pub creator: Option<String>,
}

/// A document opened far enough to read or change its identity.
#[derive(Debug, Clone)]
pub struct Document {
    kind: Kind,
    serialization: Serialization,
    identity: Identity,
    /// Original bytes, kept so a rewrite can splice and re-encrypt in place.
    raw: Vec<u8>,
    layout: Layout,
}

#[derive(Debug, Clone)]
enum Layout {
    Text {
        /// Offsets are absolute; the metadata section starts at the header.
        fields: text::Fields,
    },
    Binary {
        meta: SectionSpan,
        body: SectionSpan,
        meta_fields: BinaryFields,
        body_fields: BinaryFields,
    },
}

impl Document {
    pub fn read(path: &Path) -> Result<Self> {
        let kind = Kind::from_path(path)
            .ok_or_else(|| Error::UnknownKind(path.display().to_string()))?;
        let raw = std::fs::read(path)
            .map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::parse(kind, raw)
    }

    pub fn parse(kind: Kind, raw: Vec<u8>) -> Result<Self> {
        let header = Header::parse(&raw)?;
        let serialization = Serialization::from_header(header.serialization_format)?;

        let (identity, layout) = match serialization {
            Serialization::Text => {
                let section = raw
                    .get(HEADER_LEN..header.object_offset)
                    .ok_or(Error::Truncated { at: HEADER_LEN })?;
                let mut fields = text::scan(section);
                // Rebase onto the whole file so splices need no adjustment.
                for field in fields.values_mut() {
                    field.offset += HEADER_LEN;
                }
                (Identity::from_text(&fields)?, Layout::Text { fields })
            }
            _ => {
                let encrypted = serialization == Serialization::EncryptedBinary;
                let meta_end = header
                    .object_offset
                    .checked_sub(BINARY_PADDING)
                    .ok_or(Error::Truncated { at: HEADER_LEN })?;
                let meta = SectionSpan::of(&raw, HEADER_LEN, meta_end, encrypted)?;
                let body = SectionSpan::of(&raw, header.object_offset, raw.len(), encrypted)?;
                let meta_fields = Scanner::new(&meta.plaintext(&raw)).scan_object()?;
                let body_fields = Scanner::new(&body.plaintext(&raw)).scan_object()?;
                (
                    Identity::from_binary(&meta_fields)?,
                    Layout::Binary { meta, body, meta_fields, body_fields },
                )
            }
        };

        Ok(Document { kind, serialization, identity, raw, layout })
    }

    pub fn kind(&self) -> Kind {
        self.kind
    }

    pub fn serialization(&self) -> Serialization {
        self.serialization
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn bytes(&self) -> &[u8] {
        &self.raw
    }

    /// Produce the document under a new UUID.
    ///
    /// Every copy of the identity is updated. A UUID is fixed width in both the
    /// binary and the textual form, so each is spliced in place and the file
    /// layout is untouched.
    ///
    /// Destructive to projects that already reference the old identity, which is
    /// why this is never applied implicitly.
    #[must_use = "returns the rewritten document rather than mutating in place"]
    pub fn with_uuid(&self, new: Uuid) -> Result<Document> {
        let old = self.identity.uuid;
        let mut raw = self.raw.clone();

        match &self.layout {
            Layout::Text { fields } => {
                for key in [F_UUID, F_ID] {
                    let Some(field) = fields.get(key) else { continue };
                    let updated = field.value.replace(&old.to_string(), &new.to_string());
                    ramona::splice(&mut raw, field.offset, field.len, updated.as_bytes())?;
                }
            }
            Layout::Binary { meta, body, meta_fields, body_fields } => {
                let mut plain = meta.plaintext(&raw);
                splice_uuid(&mut plain, meta_fields, &FieldKey::Name(F_UUID.into()), new)?;
                splice_uuid_in_text(&mut plain, meta_fields, &FieldKey::Name(F_ID.into()), old, new)?;
                meta.write_back(&mut raw, &plain);

                let mut plain = body.plaintext(&raw);
                splice_uuid(&mut plain, body_fields, &FieldKey::Id(BODY_UUID), new)?;
                body.write_back(&mut raw, &plain);
            }
        }

        Document::parse(self.kind, raw)
    }
}

fn splice_uuid(plain: &mut [u8], fields: &BinaryFields, key: &FieldKey, new: Uuid) -> Result<()> {
    match fields.get(key) {
        Some(Value::Uuid { offset, .. }) => ramona::splice(plain, *offset, 16, new.as_bytes()),
        _ => Ok(()), // Absent in this document; nothing to keep consistent.
    }
}

/// Rewrite a UUID embedded in a text field, e.g. `module:<uuid>`. The textual
/// form is always 36 characters, so the field length is preserved.
fn splice_uuid_in_text(
    plain: &mut [u8],
    fields: &BinaryFields,
    key: &FieldKey,
    old: Uuid,
    new: Uuid,
) -> Result<()> {
    let Some(Value::Text { text, offset, wide }) = fields.get(key) else {
        return Ok(());
    };
    let updated = text.replace(&old.to_string(), &new.to_string());
    if updated == *text {
        return Ok(());
    }
    // The recorded offset points at the 4-byte length prefix.
    let encoded = encode_text(&updated, *wide);
    let len = encode_text(text, *wide).len();
    ramona::splice(plain, offset + 4, len, &encoded)
}

fn encode_text(text: &str, wide: bool) -> Vec<u8> {
    if wide {
        text.encode_utf16().flat_map(|u| u.to_be_bytes()).collect()
    } else {
        text.chars().map(|c| c as u8).collect()
    }
}

impl Identity {
    fn from_binary(fields: &BinaryFields) -> Result<Self> {
        let uuid = match fields.get(&FieldKey::Name(F_UUID.into())) {
            Some(Value::Uuid { uuid, .. }) => *uuid,
            _ => return Err(Error::MissingField(F_UUID)),
        };
        let text = |name: &'static str| match fields.get(&FieldKey::Name(name.into())) {
            Some(Value::Text { text, .. }) => Some(text.clone()),
            _ => None,
        };
        Self::assemble(uuid, text)
    }

    fn from_text(fields: &text::Fields) -> Result<Self> {
        let raw = fields.get(F_UUID).ok_or(Error::MissingField(F_UUID))?;
        let uuid = Uuid::parse_str(&raw.value).map_err(|_| Error::MissingField(F_UUID))?;
        Self::assemble(uuid, |name| fields.get(name).map(|f| f.value.clone()))
    }

    fn assemble(uuid: Uuid, text: impl Fn(&'static str) -> Option<String>) -> Result<Self> {
        Ok(Identity {
            uuid,
            name: text(F_NAME).ok_or(Error::MissingField(F_NAME))?,
            description: text(F_DESCRIPTION).map(trim_owned).filter(|s| !s.is_empty()),
            category: text(F_CATEGORY).filter(|s| !s.is_empty()),
            creator: text(F_CREATOR).filter(|s| !s.is_empty()),
        })
    }
}

/// Descriptions in real documents carry trailing padding spaces.
fn trim_owned(s: String) -> String {
    s.trim().to_owned()
}

impl Identity {
    /// Search terms proposed for this document: the words of its name, plus the
    /// browser category if it declares one.
    ///
    /// Bitwig matches a browser query against keywords by prefix, so a document
    /// with none is unfindable by typing. These are the defaults that make a
    /// registration usable without the user editing anything.
    pub fn suggested_keywords(&self) -> Vec<String> {
        let mut words: Vec<String> = self
            .name
            .split(|c: char| !c.is_alphanumeric())
            .filter(|word| !word.is_empty())
            .map(str::to_lowercase)
            .collect();
        if let Some(category) = &self.category {
            words.push(category.to_lowercase());
        }
        words.dedup();
        words
    }
}

/// Where a section's payload lives, and how it is protected.
#[derive(Debug, Clone, Copy)]
struct SectionSpan {
    payload: (usize, usize),
    /// `None` for a plain section; the Dag nonce otherwise.
    nonce: Option<[u8; cipher::NONCE_LEN]>,
}

impl SectionSpan {
    fn of(raw: &[u8], start: usize, end: usize, encrypted: bool) -> Result<Self> {
        if end > raw.len() || start >= end {
            return Err(Error::Truncated { at: start });
        }
        if !encrypted {
            return Ok(SectionSpan { payload: (start, end), nonce: None });
        }
        // Encrypted sections prefix a version byte and the nonce.
        let prefix = 1 + cipher::NONCE_LEN;
        if start + prefix > end {
            return Err(Error::Truncated { at: start });
        }
        let mut nonce = [0u8; cipher::NONCE_LEN];
        nonce.copy_from_slice(&raw[start + 1..start + prefix]);
        Ok(SectionSpan { payload: (start + prefix, end), nonce: Some(nonce) })
    }

    fn plaintext(&self, raw: &[u8]) -> Vec<u8> {
        let bytes = &raw[self.payload.0..self.payload.1];
        match &self.nonce {
            Some(nonce) => cipher::transform(nonce, bytes),
            None => bytes.to_vec(),
        }
    }

    fn write_back(&self, raw: &mut [u8], plain: &[u8]) {
        let encoded = match &self.nonce {
            Some(nonce) => cipher::transform(nonce, plain),
            None => plain.to_vec(),
        };
        raw[self.payload.0..self.payload.1].copy_from_slice(&encoded);
    }
}

/// The 42-byte hex-ASCII document header.
struct Header {
    serialization_format: u32,
    object_offset: usize,
}

impl Header {
    fn parse(data: &[u8]) -> Result<Self> {
        let head = data.get(..HEADER_LEN).ok_or(Error::Truncated { at: 0 })?;
        let text = std::str::from_utf8(head).map_err(|_| Error::BadMagic)?;
        if !text.starts_with("BtWg") {
            return Err(Error::BadMagic);
        }
        let hex = |range: std::ops::Range<usize>| {
            u64::from_str_radix(&text[range], 16).map_err(|_| Error::BadMagic)
        };
        Ok(Header {
            serialization_format: hex(8..12)? as u32,
            object_offset: hex(16..24)? as usize,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Custom documents to test against, from `ORNG_TEST_DOCUMENTS`.
    ///
    /// Sample documents are somebody's work and are not redistributed with this
    /// repository, so the path is supplied rather than assumed. Tests that need
    /// them skip when it is unset.
    fn custom_samples() -> Vec<std::path::PathBuf> {
        let Some(root) = std::env::var_os("ORNG_TEST_DOCUMENTS") else {
            return Vec::new();
        };
        let mut found = Vec::new();
        collect(std::path::Path::new(&root), &mut found);
        found.sort();
        found
    }

    fn collect(dir: &Path, into: &mut Vec<std::path::PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for path in entries.flatten().map(|e| e.path()) {
            if path.is_dir() {
                collect(&path, into);
            } else if Kind::from_path(&path).is_some() {
                into.push(path);
            }
        }
    }

    /// Factory content from the installed Bitwig, for the encrypted form.
    fn factory_samples(limit: usize) -> Vec<std::path::PathBuf> {
        let Ok(install) = bitwig_install::Installation::discover() else {
            return Vec::new();
        };
        let library = install.library_dir();
        Kind::ALL
            .iter()
            .flat_map(|kind| {
                std::fs::read_dir(library.join(kind.library_subdir()))
                    .into_iter()
                    .flatten()
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| Kind::from_path(p).is_some())
                    .take(limit)
            })
            .collect()
    }

    #[test]
    fn reads_every_serialization_in_the_wild() {
        let all: Vec<_> = custom_samples().into_iter().chain(factory_samples(12)).collect();
        if all.is_empty() {
            eprintln!("no sample documents, skipping");
            return;
        }
        let mut seen = std::collections::BTreeSet::new();
        for path in &all {
            let doc = Document::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            assert!(!doc.identity().name.is_empty(), "{} has no name", path.display());
            assert_ne!(doc.identity().uuid, Uuid::nil());
            assert_eq!(doc.kind(), Kind::from_path(path).unwrap());
            seen.insert(format!("{:?}", doc.serialization()));
        }
        // Whatever was available had to cover at least one encoding, and the
        // set is printed so a narrowed run is visible rather than silent.
        println!("exercised {seen:?} across {} documents", all.len());
    }

    #[test]
    fn rewriting_the_uuid_is_reversible_and_length_preserving() {
        let all: Vec<_> = custom_samples().into_iter().chain(factory_samples(4)).collect();
        if all.is_empty() {
            eprintln!("no sample documents, skipping");
            return;
        }
        for path in &all {
            let doc = Document::read(path).unwrap();
            let new = Uuid::new_v4();
            let rewritten = doc.with_uuid(new).unwrap();

            assert_eq!(rewritten.identity().uuid, new, "{}", path.display());
            assert_eq!(rewritten.identity().name, doc.identity().name);
            assert_eq!(rewritten.bytes().len(), doc.bytes().len());

            let restored = rewritten.with_uuid(doc.identity().uuid).unwrap();
            assert_eq!(restored.bytes(), doc.bytes(), "{} did not round-trip", path.display());
        }
    }
}

#[cfg(test)]
mod keyword_tests {
    use super::*;

    fn identity(name: &str, category: Option<&str>) -> Identity {
        Identity {
            uuid: Uuid::nil(),
            name: name.into(),
            description: None,
            category: category.map(str::to_owned),
            creator: None,
        }
    }

    #[test]
    fn keywords_come_from_the_name_and_category() {
        assert_eq!(
            identity("Glue Comp", Some("Dynamics")).suggested_keywords(),
            ["glue", "comp", "dynamics"]
        );
        assert_eq!(identity("DISPERSER", None).suggested_keywords(), ["disperser"]);
    }
}
