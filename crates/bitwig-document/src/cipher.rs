// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The Dag stream cipher guarding document sections.
//!
//! Symmetric XOR: encrypting and decrypting are the same operation, and a
//! same-length edit can be spliced into the plaintext and re-encrypted under
//! the original nonce without disturbing any other byte.
//!
//! **The key is not here.** It belongs to the installation and is read out of
//! it - see `bitwig_registry::section_key`. This crate needs no installation,
//! which is the whole reason it is a separate crate, so the key arrives as an
//! argument or the encrypted form is refused.

use std::fmt;

pub const NONCE_LEN: usize = 16;

/// The shortest key the cipher will take.
///
/// Bitwig's own factory refuses anything shorter, with `Key too short`, and the
/// number is its own rather than a guess: the cipher spends the first sixteen
/// bytes on the pad seed and the rest on the keystream, so a short key silently
/// becomes a weak one instead of failing.
pub const MIN_KEY_LEN: usize = 48;

/// The key a document's sections are encrypted under.
///
/// Opaque on purpose. It is Bitwig's material, not this project's, and nothing
/// here may write it to a log, a snapshot or an error message - so it has no
/// `Display`, and its `Debug` says how long it is and nothing else.
#[derive(Clone, PartialEq, Eq)]
pub struct SectionKey(Vec<u8>);

/// Why a key was refused. Never carries the material it refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyError {
    TooShort { len: usize },
    NotHex,
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            KeyError::TooShort { len } => {
                write!(f, "a section key is at least {MIN_KEY_LEN} bytes; this one is {len}")
            }
            KeyError::NotHex => f.write_str("a section key in hexadecimal, and this is not"),
        }
    }
}

impl std::error::Error for KeyError {}

impl SectionKey {
    pub fn new(bytes: Vec<u8>) -> Result<Self, KeyError> {
        if bytes.len() < MIN_KEY_LEN {
            return Err(KeyError::TooShort { len: bytes.len() });
        }
        Ok(SectionKey(bytes))
    }

    /// The form the key travels in when it is handed to a test or a runner.
    pub fn from_hex(text: &str) -> Result<Self, KeyError> {
        let text = text.trim();
        if !text.len().is_multiple_of(2) {
            return Err(KeyError::NotHex);
        }
        let bytes = (0..text.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&text[i..i + 2], 16).map_err(|_| KeyError::NotHex))
            .collect::<Result<Vec<u8>, KeyError>>()?;
        Self::new(bytes)
    }

    pub fn as_bytes(&self) -> &[u8] {
        &self.0
    }
}

impl fmt::Debug for SectionKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SectionKey({} bytes)", self.0.len())
    }
}

struct Dag<'a> {
    key_tail: &'a [u8],
    stream: Vec<u8>,
    key_idx: usize,
    stream_idx: usize,
    cycle: u32,
}

impl<'a> Dag<'a> {
    fn new(key: &'a SectionKey, nonce: &[u8]) -> Self {
        let key = key.as_bytes();
        let mut stream = Vec::with_capacity(nonce.len() + 16);
        stream.extend_from_slice(nonce);
        stream.extend_from_slice(&key[..16]);
        Dag { key_tail: &key[16..], stream, key_idx: 0, stream_idx: 0, cycle: 0 }
    }

    fn apply_byte(&mut self, input: u8) -> u8 {
        if self.key_idx >= self.key_tail.len() {
            self.key_idx = 0;
            self.cycle += 1;
        }
        if self.stream_idx >= self.stream.len() {
            self.stream_idx = 0;
        }
        let k = self.key_tail[self.key_idx];
        let s = self.stream[self.stream_idx] as u32;
        self.key_idx += 1;
        self.stream_idx += 1;
        let rot = self.cycle & 7;
        let rotated = (s >> rot | s << (8 - rot)) as u8;
        input ^ k ^ rotated
    }

    fn apply(&mut self, data: &[u8]) -> Vec<u8> {
        data.iter().map(|&b| self.apply_byte(b)).collect()
    }
}

/// Transform `data` under `key` and `nonce`. Encryption and decryption are
/// identical.
pub fn transform(key: &SectionKey, nonce: &[u8; NONCE_LEN], data: &[u8]) -> Vec<u8> {
    Dag::new(key, nonce).apply(data)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A key of the right shape and no significance, for the properties that
    /// hold whatever the key is.
    fn a_key() -> SectionKey {
        SectionKey::new((0..64u8).map(|i| i.wrapping_mul(37).wrapping_add(11)).collect())
            .expect("64 bytes is long enough")
    }

    #[test]
    fn transform_is_an_involution() {
        let key = a_key();
        let nonce = [7u8; NONCE_LEN];
        let plain = b"device_uuid and some padding to cross the key cycle boundary".repeat(4);
        assert_eq!(transform(&key, &nonce, &transform(&key, &nonce, &plain)), plain);
    }

    /// The cipher is only symmetric under the key it ran with, which is what
    /// makes decrypting with the wrong key detectable rather than silent.
    #[test]
    fn another_key_does_not_undo_it() {
        let nonce = [7u8; NONCE_LEN];
        let other = SectionKey::new(vec![9u8; 64]).expect("long enough");
        let plain = b"device_uuid".repeat(8);
        assert_ne!(transform(&other, &nonce, &transform(&a_key(), &nonce, &plain)), plain);
    }

    #[test]
    fn a_key_too_short_to_be_one_is_refused() {
        let short = vec![0u8; MIN_KEY_LEN - 1];
        assert_eq!(SectionKey::new(short), Err(KeyError::TooShort { len: MIN_KEY_LEN - 1 }));
    }

    #[test]
    fn a_key_survives_the_hexadecimal_it_arrives_in() {
        let key = a_key();
        let hex: String = key.as_bytes().iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(SectionKey::from_hex(&hex).expect("round trips"), key);
        assert_eq!(SectionKey::from_hex("not hex at all!!"), Err(KeyError::NotHex));
    }

    /// The material must not reach a log or a panic message.
    #[test]
    fn a_key_never_prints_itself() {
        let key = a_key();
        let shown = format!("{key:?}");
        assert_eq!(shown, "SectionKey(64 bytes)");
        for byte in key.as_bytes() {
            assert!(!shown.contains(&format!("{byte}")), "{shown} leaks the material");
        }
    }
}
