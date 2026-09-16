// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Constant pool access without a full class parse.
//!
//! Anchor resolution sweeps every class in a 30k-entry archive, and almost all
//! of them are rejected on their strings alone. Walking just the pool is an
//! order of magnitude cheaper than parsing fields, methods and bytecode.
//!
//! This is also the only correct place to retarget a precompiled helper class:
//! a call site's owner, name and descriptor are all pool strings, so rewriting
//! them redirects the call without touching a single instruction.

use std::collections::HashMap;

use crate::{Error, Result};

const MAGIC: [u8; 4] = [0xCA, 0xFE, 0xBA, 0xBE];
/// Class file header before the pool: magic, minor, major, pool count.
const HEADER_LEN: usize = 10;

const TAG_UTF8: u8 = 1;
const TAG_LONG: u8 = 5;
const TAG_DOUBLE: u8 = 6;

/// A UTF-8 pool entry and where its bytes sit in the class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Utf8Span {
    pub start: usize,
    pub len: usize,
}

/// Byte ranges of every UTF-8 entry, plus where the pool ends.
#[derive(Debug, Clone)]
pub struct PoolIndex {
    pub utf8: Vec<Utf8Span>,
    pub pool_end: usize,
}

/// Index the constant pool, skipping entries by their fixed widths.
pub fn index(class: &[u8]) -> Result<PoolIndex> {
    if class.len() < HEADER_LEN || class[..4] != MAGIC {
        return Err(Error::NotAClass);
    }
    let count = u16::from_be_bytes([class[8], class[9]]) as usize;
    let mut utf8 = Vec::new();
    let mut pos = HEADER_LEN;
    let mut i = 1; // pool indices are 1-based

    while i < count {
        let tag = *class.get(pos).ok_or(Error::TruncatedPool { at: pos })?;
        pos += 1;
        match tag {
            TAG_UTF8 => {
                let len_bytes = class.get(pos..pos + 2).ok_or(Error::TruncatedPool { at: pos })?;
                let len = u16::from_be_bytes([len_bytes[0], len_bytes[1]]) as usize;
                pos += 2;
                if pos + len > class.len() {
                    return Err(Error::TruncatedPool { at: pos });
                }
                utf8.push(Utf8Span { start: pos, len });
                pos += len;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => pos += 4,
            TAG_LONG | TAG_DOUBLE => {
                pos += 8;
                i += 1; // occupies two pool slots
            }
            7 | 8 | 16 | 19 | 20 => pos += 2,
            15 => pos += 3,
            other => return Err(Error::UnknownPoolTag { at: pos - 1, tag: other }),
        }
        i += 1;
    }
    if pos > class.len() {
        return Err(Error::TruncatedPool { at: pos });
    }
    Ok(PoolIndex { utf8, pool_end: pos })
}

/// Every UTF-8 entry as a byte slice.
pub fn strings(class: &[u8]) -> Result<Vec<&[u8]>> {
    Ok(index(class)?
        .utf8
        .into_iter()
        .map(|s| &class[s.start..s.start + s.len])
        .collect())
}

/// Whether the pool holds this exact string. The cheapest possible anchor test.
pub fn contains(class: &[u8], needle: &str) -> bool {
    // A substring miss on the raw bytes rules the class out without parsing.
    if !contains_bytes(class, needle.as_bytes()) {
        return false;
    }
    strings(class).is_ok_and(|all| all.contains(&needle.as_bytes()))
}

fn contains_bytes(haystack: &[u8], needle: &[u8]) -> bool {
    needle.len() <= haystack.len()
        && haystack.windows(needle.len()).any(|w| w == needle)
}

/// Rewrite pool strings, returning the new class and how often each key hit.
///
/// Replacements may differ in length: entries are length-prefixed and nothing
/// after the pool refers to a byte offset, so the tail is copied verbatim.
pub fn replace_strings(
    class: &[u8],
    replacements: &HashMap<&str, String>,
) -> Result<(Vec<u8>, HashMap<String, usize>)> {
    let idx = index(class)?;
    let mut out = Vec::with_capacity(class.len());
    let mut counts: HashMap<String, usize> = HashMap::new();
    let mut copied = 0usize;

    for span in &idx.utf8 {
        let original = &class[span.start..span.start + span.len];
        let Ok(text) = std::str::from_utf8(original) else { continue };
        let Some(replacement) = replacements.get(text) else { continue };

        // Copy up to this entry's length prefix, then emit the new prefix.
        out.extend_from_slice(&class[copied..span.start - 2]);
        let bytes = replacement.as_bytes();
        let len = u16::try_from(bytes.len()).map_err(|_| Error::StringTooLong(bytes.len()))?;
        out.extend_from_slice(&len.to_be_bytes());
        out.extend_from_slice(bytes);
        copied = span.start + span.len;
        *counts.entry(text.to_owned()).or_default() += 1;
    }
    out.extend_from_slice(&class[copied..]);
    Ok((out, counts))
}

/// Flip a method's `ACC_PRIVATE` to `ACC_PUBLIC`, in place, by name.
///
/// A retargeted helper calls the registry's registration method directly. That
/// method is private, so a direct call would fail verification; widening access
/// is a two-byte edit that leaves the bytecode untouched.
pub fn make_method_public(class: &[u8], method_name: &str, descriptor: &str) -> Result<Vec<u8>> {
    const ACC_PUBLIC: u16 = 0x0001;
    const ACC_PRIVATE: u16 = 0x0002;
    const ACC_PROTECTED: u16 = 0x0004;

    let idx = index(class)?;
    let name_idx = ordinal_of(class, method_name.as_bytes()).ok_or(Error::MethodNotFound)?;
    let desc_idx = ordinal_of(class, descriptor.as_bytes()).ok_or(Error::MethodNotFound)?;

    let mut pos = idx.pool_end + 6; // access, this_class, super_class
    let iface_count = read_u16(class, pos)? as usize;
    pos += 2 + iface_count * 2;
    pos = skip_members(class, pos)?; // fields

    let method_count = read_u16(class, pos)? as usize;
    pos += 2;
    for _ in 0..method_count {
        let access = read_u16(class, pos)?;
        let name = read_u16(class, pos + 2)?;
        let desc = read_u16(class, pos + 4)?;
        if name == name_idx && desc == desc_idx {
            let widened = (access & !(ACC_PRIVATE | ACC_PROTECTED)) | ACC_PUBLIC;
            let mut out = class.to_vec();
            out[pos..pos + 2].copy_from_slice(&widened.to_be_bytes());
            return Ok(out);
        }
        pos = skip_attributes(class, pos + 6)?;
    }
    Err(Error::MethodNotFound)
}

/// Pool ordinal of a UTF-8 entry, counting every slot including wide ones.
fn ordinal_of(class: &[u8], needle: &[u8]) -> Option<u16> {
    let count = u16::from_be_bytes([class[8], class[9]]) as usize;
    let mut pos = HEADER_LEN;
    let mut i = 1;
    while i < count {
        let tag = *class.get(pos)?;
        pos += 1;
        match tag {
            TAG_UTF8 => {
                let len = u16::from_be_bytes([*class.get(pos)?, *class.get(pos + 1)?]) as usize;
                pos += 2;
                if class.get(pos..pos + len)? == needle {
                    return u16::try_from(i).ok();
                }
                pos += len;
            }
            3 | 4 | 9 | 10 | 11 | 12 | 17 | 18 => pos += 4,
            TAG_LONG | TAG_DOUBLE => {
                pos += 8;
                i += 1;
            }
            7 | 8 | 16 | 19 | 20 => pos += 2,
            15 => pos += 3,
            _ => return None,
        }
        i += 1;
    }
    None
}

fn read_u16(class: &[u8], pos: usize) -> Result<u16> {
    let bytes = class.get(pos..pos + 2).ok_or(Error::TruncatedPool { at: pos })?;
    Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
}

fn skip_attributes(class: &[u8], mut pos: usize) -> Result<usize> {
    let count = read_u16(class, pos)? as usize;
    pos += 2;
    for _ in 0..count {
        let len = u32::from_be_bytes(
            class
                .get(pos + 2..pos + 6)
                .ok_or(Error::TruncatedPool { at: pos })?
                .try_into()
                .unwrap(),
        ) as usize;
        pos += 6 + len;
    }
    Ok(pos)
}

fn skip_members(class: &[u8], mut pos: usize) -> Result<usize> {
    let count = read_u16(class, pos)? as usize;
    pos += 2;
    for _ in 0..count {
        pos = skip_attributes(class, pos + 6)?;
    }
    Ok(pos)
}
