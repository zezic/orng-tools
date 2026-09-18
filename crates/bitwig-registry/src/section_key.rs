// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Reading the document section key out of an installation.
//!
//! Bitwig encrypts its own factory documents - the `0004` serialization - under
//! a key that belongs to the installation. Nothing in this project may carry
//! that key: it is Bitwig's material, and a copy of it in a source tree is a
//! copy of Bitwig's material in a source tree. So it is read from the
//! installation the user already has, every time, and never written down.
//!
//! **It is not stored as a run of bytes.** The key is a `byte[]` literal built
//! by bytecode - `newarray byte`, then one `bastore` per element - so the bytes
//! sit one every four, interleaved with opcodes. Searching a jar for the key as
//! a contiguous string finds nothing, in any encoding. Reading the array out of
//! the bytecode finds it.
//!
//! Resolution here follows the same rule as every other anchor in this crate:
//! **no obfuscated name is written down.** The class holding the key is named
//! `Tl3` on 6.1 and `q2p` on 5.1.9. What does not move is the package it sits
//! in, which Bitwig does not obfuscate, and the length of the array. What
//! settles it is neither: a candidate is accepted only once it has decrypted a
//! document that then parses.

use std::path::Path;

use bitwig_classfile::Jar;
use bitwig_document::{Document, Kind, SectionKey};

use crate::{Error, Result};

/// The package Bitwig's own document serialization lives in, unobfuscated in
/// every build seen. Narrowing to it turns a jar-wide scan into a handful of
/// classes; it is not what makes the answer right.
const SERIAL_PACKAGE: &str = "com/bitwig/base/serial/file/";

/// The shortest array worth considering. Bitwig's own cipher factory refuses a
/// key shorter than this, with `Key too short`, so anything below it is not a
/// key whatever else it is.
const MIN_ARRAY: usize = 48;

/// Read the section key out of an installation's jar, and prove it.
///
/// `verify_against` is a factory document from the same installation. It is not
/// optional and that is the point: the key that comes back has decrypted a real
/// document and had the result parse, so a build that moved the array, or a
/// second array that happens to be the right length, fails here rather than
/// three layers up as a corrupt-looking document.
///
/// **Call this once per installation, not once per document.** It costs about
/// 80ms - half of it the archive's central directory, which every jar operation
/// here pays - and the key it returns is what every later read takes as an
/// argument. There is deliberately no cache behind it: a key held on disk would
/// be Bitwig's own material in a file this project wrote, which is the thing
/// removing it from the source tree was for, and it would go stale the next
/// time Bitwig updates. Hold it for as long as the resolved installation is
/// held, and drop it with that.
pub fn section_key(jar: &Path, verify_against: &Path) -> Result<SectionKey> {
    let sample = std::fs::read(verify_against).map_err(|source| Error::Io {
        path: verify_against.display().to_string(),
        source,
    })?;
    let kind = Kind::from_path(verify_against)
        .ok_or(Error::NoSectionKey { tried: 0, why: "the sample is not a document" })?;

    let jar = Jar::open(jar)?;
    let mut tried = 0usize;
    let mut found = None;

    jar.visit_classes_under(SERIAL_PACKAGE, |_name, bytes| {
        for array in byte_array_literals(bytes) {
            let Ok(candidate) = SectionKey::new(array) else { continue };
            tried += 1;
            if Document::parse_with_key(kind, sample.clone(), &candidate).is_ok() {
                found = Some(candidate);
                return std::ops::ControlFlow::Break(());
            }
        }
        std::ops::ControlFlow::Continue(())
    })?;

    found.ok_or(Error::NoSectionKey {
        tried,
        why: if tried == 0 {
            "no array long enough to be a key in the serialization package"
        } else {
            "none of the candidates decrypted the sample document"
        },
    })
}

/// Every `byte[]` built by a run of `bastore`, longest first.
///
/// Longest first because a key is the long array in a class and the short ones
/// beside it are lengths, magics and padding; trying the plausible one first
/// keeps the usual case to a single decrypt.
///
/// The whole class is scanned rather than its code attributes parsed. The
/// pattern - `newarray byte`, then `dup`, an index push, a value push and
/// `bastore`, repeated with indices running 0..n - is long enough that a false
/// positive would have to be a run of valid pushes each landing on 0x54, and it
/// costs nothing to be wrong: a wrong candidate fails to decrypt.
fn byte_array_literals(class: &[u8]) -> Vec<Vec<u8>> {
    let mut out: Vec<Vec<u8>> = Vec::new();
    let mut i = 0usize;
    while i + 2 < class.len() {
        // newarray, of type byte
        if class[i] != 0xBC || class[i + 1] != 0x08 {
            i += 1;
            continue;
        }
        let mut j = i + 2;
        let mut values: Vec<u8> = Vec::new();
        while j < class.len() && class[j] == 0x59 {
            // dup
            let Some((index, next)) = push(class, j + 1) else { break };
            let Some((value, next)) = push(class, next) else { break };
            if class.get(next) != Some(&0x54) {
                break;
            }
            // Indices run in order, from zero. Anything else is not an array
            // literal being filled and this is not the pattern.
            if index < 0 || index as usize != values.len() {
                break;
            }
            values.push(value as u8);
            j = next + 1;
        }
        if values.len() >= MIN_ARRAY {
            out.push(values);
        }
        i = j.max(i + 2);
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.len()));
    out
}

/// Decode one integer-push instruction, and say where the next one starts.
fn push(code: &[u8], at: usize) -> Option<(i32, usize)> {
    match *code.get(at)? {
        // iconst_m1 through iconst_5
        op @ 0x02..=0x08 => Some((i32::from(op) - 0x03, at + 1)),
        0x10 => Some((i32::from(*code.get(at + 1)? as i8), at + 2)),
        0x11 => {
            let hi = *code.get(at + 1)?;
            let lo = *code.get(at + 2)?;
            Some((i32::from(i16::from_be_bytes([hi, lo])), at + 3))
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The reader has to find an array the way javac writes one.
    #[test]
    fn reads_an_array_literal_out_of_the_bytecode_that_builds_it() {
        let wanted: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
        let mut code = vec![0x10, 64, 0xBC, 0x08]; // bipush 64; newarray byte
        for (index, value) in wanted.iter().enumerate() {
            code.push(0x59); // dup
            code.extend_from_slice(&[0x11, (index >> 8) as u8, index as u8]); // sipush index
            code.extend_from_slice(&[0x10, *value]); // bipush value
            code.push(0x54); // bastore
        }
        assert_eq!(byte_array_literals(&code), vec![wanted]);
    }

    /// The bytes must not be findable as a run, or the whole reason this reads
    /// bytecode rather than searching is wrong.
    #[test]
    fn the_array_is_not_a_contiguous_run_in_the_class() {
        let wanted: Vec<u8> = (0..64u8).map(|i| i.wrapping_mul(7).wrapping_add(3)).collect();
        let mut code = vec![0x10, 64, 0xBC, 0x08];
        for (index, value) in wanted.iter().enumerate() {
            code.push(0x59);
            code.extend_from_slice(&[0x11, (index >> 8) as u8, index as u8]);
            code.extend_from_slice(&[0x10, *value]);
            code.push(0x54);
        }
        assert!(
            !code.windows(wanted.len()).any(|w| w == wanted.as_slice()),
            "the key would have been findable with a plain search"
        );
    }

    /// Short arrays are not keys, and a run that does not fill in order is not
    /// an array literal.
    #[test]
    fn what_is_not_an_array_literal_is_not_read_as_one() {
        let short = {
            let mut code = vec![0xBC, 0x08];
            for index in 0..8u8 {
                code.extend_from_slice(&[0x59, 0x10, index, 0x10, 0xAA, 0x54]);
            }
            code
        };
        assert!(byte_array_literals(&short).is_empty(), "48 is the floor");

        let out_of_order = {
            let mut code = vec![0xBC, 0x08];
            for index in 0..64u8 {
                // Indices descending: a switch table, not an array being filled.
                code.extend_from_slice(&[0x59, 0x10, 63 - index, 0x10, 0xAA, 0x54]);
            }
            code
        };
        assert!(byte_array_literals(&out_of_order).is_empty());
    }
}
