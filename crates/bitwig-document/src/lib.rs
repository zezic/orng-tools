// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Reading and re-identifying Bitwig `.bwdevice`, `.bwmodulator` and
//! `.bwmodule` documents.
//!
//! Scope is deliberately narrow: the identity a registry entry needs, and the
//! ability to give a document a new UUID or a new name. The document body is
//! walked for its structure but never modelled.
//!
//! Bitwig writes three serializations and all of them turn up in the wild, so
//! all three are read. In every one the identity is a fixed-width value, which
//! is what lets a new UUID be spliced in without re-serializing the document. A
//! name is not, and is spliced all the same: no section addresses anything by
//! offset, so only the header has to be told what moved.

mod cipher;
mod kind;
pub mod ramona;
mod text;
mod version;

use std::path::Path;

use uuid::Uuid;

pub use cipher::{KeyError, SectionKey};
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
    #[error("{0:?} cannot be written as a document's name")]
    UnrepresentableName(String),
    #[error("{0} cannot be a file name")]
    UnplaceableName(String),
    #[error("a document of {0} bytes is past what its header can address")]
    TooLarge(usize),
    #[error(
        "this document is encrypted, which is Bitwig's own factory content; \
         reading it needs the section key from an installation"
    )]
    Encrypted,
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
    /// Held so a rewrite can re-encrypt under the same key it was read with.
    /// `None` for everything that is not factory content.
    key: Option<SectionKey>,
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
    /// Read a document that is not encrypted.
    ///
    /// Custom content - everything a user makes and everything the catalog
    /// carries - is text or plain binary. Factory content is encrypted and
    /// needs [`Document::read_with_key`].
    pub fn read(path: &Path) -> Result<Self> {
        Self::read_inner(path, None)
    }

    /// The same, for factory content, under the installation's own key.
    pub fn read_with_key(path: &Path, key: &SectionKey) -> Result<Self> {
        Self::read_inner(path, Some(key))
    }

    fn read_inner(path: &Path, key: Option<&SectionKey>) -> Result<Self> {
        let kind = Kind::from_path(path)
            .ok_or_else(|| Error::UnknownKind(path.display().to_string()))?;
        let raw = std::fs::read(path)
            .map_err(|source| Error::Io { path: path.display().to_string(), source })?;
        Self::parse_inner(kind, raw, key)
    }

    /// Parse a document that is not encrypted.
    pub fn parse(kind: Kind, raw: Vec<u8>) -> Result<Self> {
        Self::parse_inner(kind, raw, None)
    }

    /// The same, for factory content, under the installation's own key.
    pub fn parse_with_key(kind: Kind, raw: Vec<u8>, key: &SectionKey) -> Result<Self> {
        Self::parse_inner(kind, raw, Some(key))
    }

    fn parse_inner(kind: Kind, raw: Vec<u8>, key: Option<&SectionKey>) -> Result<Self> {
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
                // Fails closed. Without the key the sections decrypt to noise
                // that the scanner would report as a malformed document, which
                // says nothing about what is actually wrong.
                if encrypted && key.is_none() {
                    return Err(Error::Encrypted);
                }
                let meta_end = header
                    .object_offset
                    .checked_sub(BINARY_PADDING)
                    .ok_or(Error::Truncated { at: HEADER_LEN })?;
                let meta = SectionSpan::of(&raw, HEADER_LEN, meta_end, encrypted)?;
                let body = SectionSpan::of(&raw, header.object_offset, raw.len(), encrypted)?;
                let meta_fields = Scanner::new(&meta.plaintext(&raw, key)).scan_object()?;
                let body_fields = Scanner::new(&body.plaintext(&raw, key)).scan_object()?;
                (
                    Identity::from_binary(&meta_fields)?,
                    Layout::Binary { meta, body, meta_fields, body_fields },
                )
            }
        };

        Ok(Document { kind, serialization, identity, raw, layout, key: key.cloned() })
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
                let section_key = self.key.as_ref();
                let mut plain = meta.plaintext(&raw, section_key);
                splice_uuid(&mut plain, meta_fields, &FieldKey::Name(F_UUID.into()), new)?;
                splice_uuid_in_text(&mut plain, meta_fields, &FieldKey::Name(F_ID.into()), old, new)?;
                meta.write_back(&mut raw, &plain, section_key);

                let mut plain = body.plaintext(&raw, section_key);
                splice_uuid(&mut plain, body_fields, &FieldKey::Id(BODY_UUID), new)?;
                body.write_back(&mut raw, &plain, section_key);
            }
        }

        Document::parse_inner(self.kind, raw, self.key.as_ref())
    }

    /// Produce the document under a new display name.
    ///
    /// Only the metadata's `device_name` is rewritten, because that is the one
    /// Bitwig reads: loading a document sets the contents' name from it, over
    /// the copy the body carries (6.1, `document.core.master.device.MAR` and
    /// `ccL`, both through `file.format.Ii.Jdl`), and the header, the browser
    /// and the description key all take it from there. `preset_name` is read by
    /// nothing, and a name inside the body's own curves or panels is the
    /// author's content rather than the document's identity.
    ///
    /// A name is not fixed width, so unlike [`Document::with_uuid`] this moves
    /// every byte after it. Nothing inside a section addresses another by
    /// offset; the header does, and is moved with them.
    #[must_use = "returns the rewritten document rather than mutating in place"]
    pub fn with_name(&self, new: &str) -> Result<Document> {
        if new.is_empty() || new.chars().any(char::is_control) {
            return Err(Error::UnrepresentableName(new.to_owned()));
        }
        let raw = match &self.layout {
            Layout::Text { fields } => {
                // Written between quotes and read back without unescaping, so
                // a quote or a backslash would come back as something else.
                if new.contains(['"', '\\']) {
                    return Err(Error::UnrepresentableName(new.to_owned()));
                }
                let field = fields.get(F_NAME).ok_or(Error::MissingField(F_NAME))?;
                let span = field.offset..field.offset + field.len;
                let mut raw = self.raw.clone();
                raw.splice(span, new.bytes());
                raw
            }
            Layout::Binary { meta, meta_fields, .. } => {
                let key = self.key.as_ref();
                let Some(Value::Text { text, offset, wide }) =
                    meta_fields.get(&FieldKey::Name(F_NAME.into()))
                else {
                    return Err(Error::MissingField(F_NAME));
                };
                // The length prefix and then the characters, in the width the
                // old name was written in unless the new one needs wider.
                let old = 4 + encode_text(text, *wide).len();
                let wide = *wide || new.chars().any(|c| u32::from(c) > 0xFF);
                let units = if wide { new.encode_utf16().count() } else { new.chars().count() };
                let mut name = ((units as u32) | if wide { 0x8000_0000 } else { 0 })
                    .to_be_bytes()
                    .to_vec();
                name.extend(encode_text(new, wide));

                let mut plain = meta.plaintext(&self.raw, key);
                plain.splice(*offset..offset + old, name);
                let (start, end) = meta.payload;
                let mut raw = self.raw[..start].to_vec();
                raw.extend(meta.encoded(&plain, key));
                raw.extend_from_slice(&self.raw[end..]);
                raw
            }
        };
        let mut raw = raw;
        let moved = raw.len() as i64 - self.raw.len() as i64;
        Header::parse(&raw)?.moved_by(moved, &mut raw)?;
        Document::parse_inner(self.kind, raw, self.key.as_ref())
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

    fn plaintext(&self, raw: &[u8], key: Option<&SectionKey>) -> Vec<u8> {
        let bytes = &raw[self.payload.0..self.payload.1];
        match (&self.nonce, key) {
            (Some(nonce), Some(key)) => cipher::transform(key, nonce, bytes),
            // A span carries a nonce only where `of` was told the section is
            // encrypted, and parsing refuses that without a key, so the
            // remaining case is a plain section.
            _ => bytes.to_vec(),
        }
    }

    fn write_back(&self, raw: &mut [u8], plain: &[u8], key: Option<&SectionKey>) {
        raw[self.payload.0..self.payload.1].copy_from_slice(&self.encoded(plain, key));
    }

    /// `plain` as this section stores it, at whatever length it now is. The
    /// cipher runs from the start of the payload, so a section that grew is
    /// encrypted exactly as one Bitwig wrote at that length.
    fn encoded(&self, plain: &[u8], key: Option<&SectionKey>) -> Vec<u8> {
        match (&self.nonce, key) {
            (Some(nonce), Some(key)) => cipher::transform(key, nonce, plain),
            _ => plain.to_vec(),
        }
    }
}

/// The 42-byte hex-ASCII document header.
struct Header {
    serialization_format: u32,
    object_offset: usize,
    /// Where a ZIP of embedded resources starts, or zero for none.
    resources_offset: u64,
}

/// Where the header writes the body's offset, and where the resources'.
const OBJECT_OFFSET: std::ops::Range<usize> = 16..24;
const RESOURCES_OFFSET: std::ops::Range<usize> = 24..40;

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
            object_offset: hex(OBJECT_OFFSET)? as usize,
            resources_offset: hex(RESOURCES_OFFSET)?,
        })
    }

    /// Rewrite the offsets in `raw`'s header for a metadata section that grew
    /// by `by` bytes, or shrank. Everything the header points at lies after
    /// the metadata, so everything it points at moved.
    fn moved_by(&self, by: i64, raw: &mut [u8]) -> Result<()> {
        let object = self.object_offset as i64 + by;
        let written = format!("{object:08x}");
        if written.len() != OBJECT_OFFSET.len() {
            return Err(Error::TooLarge(raw.len()));
        }
        raw[OBJECT_OFFSET].copy_from_slice(written.as_bytes());
        // Zero is "none", and stays none.
        if self.resources_offset != 0 {
            let resources = self.resources_offset as i64 + by;
            raw[RESOURCES_OFFSET].copy_from_slice(format!("{resources:016x}").as_bytes());
        }
        Ok(())
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

    /// The key the factory samples are encrypted under, for the tests that
    /// read them.
    ///
    /// This crate cannot go and get it: it needs no installation, which is what
    /// keeps it free of every layer above. `bitwig_registry::section_key` reads
    /// it out of one, and that is where the extraction is tested. Here it
    /// arrives by hand, in the environment, and the factory samples are dropped
    /// when it does not.
    fn section_key() -> Option<SectionKey> {
        let hex = std::env::var("ORNG_SECTION_KEY").ok()?;
        Some(SectionKey::from_hex(&hex).expect("ORNG_SECTION_KEY is not a section key"))
    }

    /// Open a sample whether or not it turns out to be encrypted.
    fn open(path: &Path, key: Option<&SectionKey>) -> Result<Document> {
        match key {
            Some(key) => Document::read_with_key(path, key),
            None => Document::read(path),
        }
    }

    /// Returning from a test that found nothing to test is how a suite reports
    /// seventy-nine passes while exercising none of them. Only a runner, which
    /// has no Bitwig and no samples, has any business asking for that.
    fn skip_or_fail() {
        assert!(
            std::env::var_os("ORNG_SKIP_BITWIG_TESTS").is_some(),
            "no sample documents; point ORNG_TEST_DOCUMENTS at some, \
             or set ORNG_SKIP_BITWIG_TESTS=1 to skip these tests"
        );
        eprintln!("no sample documents, skipping");
    }

    #[test]
    fn reads_every_serialization_in_the_wild() {
        let key = section_key();
        let factory = if key.is_some() { factory_samples(12) } else { Vec::new() };
        let all: Vec<_> = custom_samples().into_iter().chain(factory).collect();
        if all.is_empty() {
            skip_or_fail();
            return;
        }
        let mut seen = std::collections::BTreeSet::new();
        for path in &all {
            let doc = open(path, key.as_ref())
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
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
        let key = section_key();
        let factory = if key.is_some() { factory_samples(4) } else { Vec::new() };
        let all: Vec<_> = custom_samples().into_iter().chain(factory).collect();
        if all.is_empty() {
            skip_or_fail();
            return;
        }
        for path in &all {
            let doc = open(path, key.as_ref()).unwrap();
            let new = Uuid::new_v4();
            let rewritten = doc.with_uuid(new).unwrap();

            assert_eq!(rewritten.identity().uuid, new, "{}", path.display());
            assert_eq!(rewritten.identity().name, doc.identity().name);
            assert_eq!(rewritten.bytes().len(), doc.bytes().len());

            let restored = rewritten.with_uuid(doc.identity().uuid).unwrap();
            assert_eq!(restored.bytes(), doc.bytes(), "{} did not round-trip", path.display());
        }
    }

    /// Longer and shorter, so the header is moved both ways, and back again to
    /// prove nothing else was touched on the way.
    #[test]
    fn renaming_moves_the_body_and_nothing_else() {
        let key = section_key();
        let factory = if key.is_some() { factory_samples(4) } else { Vec::new() };
        let all: Vec<_> = custom_samples().into_iter().chain(factory).collect();
        if all.is_empty() {
            skip_or_fail();
            return;
        }
        for path in &all {
            let doc = open(path, key.as_ref()).unwrap();
            let body = |d: &Document| {
                d.bytes()[Header::parse(d.bytes()).unwrap().object_offset..].to_vec()
            };
            for new in ["X", "A MUCH LONGER NAME THAN ANY SAMPLE CARRIES"] {
                let renamed = doc.with_name(new).unwrap();
                let (was, now) = (doc.identity(), renamed.identity());
                assert_eq!(now.name, new, "{}", path.display());
                assert_eq!(Identity { name: was.name.clone(), ..now.clone() }, *was);
                assert_eq!(body(&renamed), body(&doc), "{}", path.display());

                let restored = renamed.with_name(&was.name).unwrap();
                assert_eq!(restored.bytes(), doc.bytes(), "{} did not round-trip", path.display());
            }
        }
    }

    #[test]
    fn a_name_the_document_cannot_hold_is_refused() {
        let doc = binary(&[]);
        for bad in ["", "TWO\nLINES"] {
            assert!(matches!(doc.with_name(bad), Err(Error::UnrepresentableName(_))), "{bad:?}");
        }
    }

    /// A plain binary document carrying `resources` after its body, built by
    /// hand so the binary path is proven where there are no samples.
    fn binary(resources: &[u8]) -> Document {
        fn string(out: &mut Vec<u8>, text: &str) {
            out.extend((text.len() as u32).to_be_bytes());
            out.extend(text.bytes());
        }
        fn named(out: &mut Vec<u8>, name: &str, tag: u8) {
            out.extend(1i32.to_be_bytes());
            string(out, name);
            out.push(tag);
        }
        let uuid = Uuid::from_u128(0x6d2a2f1e_0a4f_4d8e_9a6c_1d2e3f405162);

        let mut meta = 7i32.to_be_bytes().to_vec();
        named(&mut meta, F_UUID, 21);
        meta.extend(uuid.as_bytes());
        named(&mut meta, F_NAME, 8);
        string(&mut meta, "SHAPER");
        named(&mut meta, F_CREATOR, 8);
        string(&mut meta, "Caviio");
        meta.extend(0i32.to_be_bytes());

        let mut body = 7i32.to_be_bytes().to_vec();
        body.extend(BODY_UUID.to_be_bytes());
        body.push(21);
        body.extend(uuid.as_bytes());
        body.extend(0i32.to_be_bytes());

        let object = HEADER_LEN + meta.len() + BINARY_PADDING;
        let after = object + body.len();
        let resources_at = if resources.is_empty() { 0 } else { after };
        let mut raw = format!("BtWg00030002000c{object:08x}{resources_at:016x}00").into_bytes();
        raw.extend(meta);
        raw.extend(std::iter::repeat_n(b' ', BINARY_PADDING - 1));
        raw.push(b'\n');
        raw.extend(body);
        raw.extend(resources);
        Document::parse(Kind::Modulator, raw).unwrap()
    }

    #[test]
    fn a_binary_rename_moves_the_resources_with_the_body() {
        let resources = b"PK\x03\x04 an archive of whatever the document embeds";
        let doc = binary(resources);
        let renamed = doc.with_name("VOLUME SHAPER").unwrap();
        assert_eq!(renamed.identity().name, "VOLUME SHAPER");
        assert_eq!(renamed.identity().creator.as_deref(), Some("Caviio"));

        let header = Header::parse(renamed.bytes()).unwrap();
        let at = usize::try_from(header.resources_offset).unwrap();
        assert_eq!(&renamed.bytes()[at..], resources, "the header lost its resources");
        assert_eq!(renamed.with_name("SHAPER").unwrap().bytes(), doc.bytes());
    }

    /// Latin-1 is one byte a character and anything wider is UTF-16, so a name
    /// past Latin-1 has to change the width it is written in, not be mangled
    /// into it.
    #[test]
    fn a_name_past_latin_1_is_written_wide() {
        let renamed = binary(&[]).with_name("\u{424}\u{43e}\u{440}\u{43c}\u{430}").unwrap();
        assert_eq!(renamed.identity().name, "\u{424}\u{43e}\u{440}\u{43c}\u{430}");
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
