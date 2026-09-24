// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Fixtures the unit tests in this crate share.
//!
//! Everything here is synthetic on purpose, so that the tests built on it run on
//! every platform and on a machine with no Bitwig Studio at all. What is real
//! about a document - that all three serializations are read, that an identity
//! can be rewritten byte-exactly - is `bitwig-document`'s to prove against real
//! files, and it does. What this crate has to prove is that the list, the
//! documents and the description bundles stay in step, and that needs a document
//! with an identity rather than a document from Bitwig.

use std::path::Path;

use uuid::Uuid;

use crate::{Document, Installation, Kind};

/// The document header: magic, the file format version, the serialization
/// format, and the offset the body starts at. Hex ASCII, and 40 bytes at
/// version 1, the one textual documents have been seen at.
const FILE_VERSION: u32 = 1;
const TEXT_FORMAT: u32 = 1;
const HEADER_LEN: usize = 40;

/// A document in the textual serialization, carrying the identity it is given.
///
/// The textual form is the one that can be written by hand: its metadata section
/// is relaxed JSON, so the fields a registration reads are legible in the test
/// that builds them.
pub fn document(kind: Kind, uuid: Uuid, name: &str) -> Document {
    let meta = format!(
        "{{\n  class : \"meta\",\n  data :\n  {{\n\
         \x20   \"device_uuid\" : \"{uuid}\",\n\
         \x20   \"device_name\" : \"{name}\",\n\
         \x20   \"device_category\" : \"Test\",\n\
         \x20   \"creator\" : \"orng tools tests\"\n\
         \x20 }}\n}}\n"
    );
    let body = b"{\n  class : \"device\"\n}\n";
    let offset = HEADER_LEN + meta.len();
    let header = format!("BtWg{FILE_VERSION:04}{TEXT_FORMAT:04}{:04}{offset:08x}{:016}", 0, 0);
    assert_eq!(header.len(), HEADER_LEN);

    let mut raw = header.into_bytes();
    raw.extend_from_slice(meta.as_bytes());
    raw.extend_from_slice(body);
    Document::parse(kind, raw).expect("the synthetic document does not parse")
}

/// An installation only as far as resolving one cares: an archive, and the two
/// content directories beside it. Nothing here is a real Bitwig.
pub fn install(root: &Path) -> Installation {
    std::fs::create_dir_all(root.join("Contents/Java")).unwrap();
    std::fs::write(root.join("Contents/Java/bitwig.jar"), b"").unwrap();
    std::fs::create_dir_all(root.join("Contents/Resources/Library")).unwrap();
    // Both, because resolving an installation insists on finding each rather
    // than assuming one sits beside the other. Where they sit differs by
    // platform; that they exist does not.
    std::fs::create_dir_all(root.join("Contents/Resources/localization")).unwrap();
    Installation::at(root).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The fixture has to be a document, or every test built on it proves only
    /// that a parser rejects the same bytes twice.
    #[test]
    fn the_synthetic_document_reads_back_the_identity_it_was_given() {
        let uuid = Uuid::new_v4();
        let document = document(Kind::Modulator, uuid, "SHAPER");
        assert_eq!(document.identity().uuid, uuid);
        assert_eq!(document.identity().name, "SHAPER");
        assert_eq!(document.kind(), Kind::Modulator);
        assert_eq!(document.serialization(), crate::Serialization::Text);
        assert_eq!(document.identity().suggested_keywords(), ["shaper", "test"]);
    }

    /// The textual form is the one the rename is not proven on by a sample
    /// here, and the one with a name it cannot hold: its reader takes a quoted
    /// run as it stands, so an escaped quote would read back as two characters.
    #[test]
    fn a_text_document_is_renamed_and_refuses_what_it_would_read_back_wrong() {
        let original = document(Kind::Device, Uuid::new_v4(), "SHAPER");
        let renamed = original.with_name("VOLUME SHAPER").unwrap();
        assert_eq!(renamed.identity().name, "VOLUME SHAPER");
        assert_eq!(renamed.identity().uuid, original.identity().uuid);
        assert_eq!(renamed.with_name("SHAPER").unwrap().bytes(), original.bytes());

        for bad in ["SAY \"HI\"", "BACK\\SLASH"] {
            assert!(original.with_name(bad).is_err(), "{bad:?}");
        }
    }
}
