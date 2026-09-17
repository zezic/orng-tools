// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Proving an index is the one the catalog published.
//!
//! The index is the trust anchor: every row carries a digest and a downloaded
//! document is checked against it, so whoever serves the index decides what gets
//! installed into a DAW. With a release asset alone that is GitHub, and with a
//! mirror at `orng.tools` it would be the mirror too. A signature takes both out
//! of the trust path: the key never leaves the workflow that publishes, and a
//! mirror that rewrites a digest produces a file nothing will verify.
//!
//! **Detached, not embedded.** A signature inside the file would have to be
//! excluded from what it covers, which means signing a canonicalisation rather
//! than the bytes, and a canonicalisation is a second serializer to agree on -
//! the classic shape of a signature-stripping bug. Keeping the signature in a
//! second asset means the bytes signed, the bytes served and the bytes parsed
//! are the same bytes, with nothing to normalise. It also costs older readers
//! nothing: `index.json` does not change, so a build that predates this reads it
//! exactly as before.
//!
//! **ed25519**, because the signature is 64 bytes, the public key is 32, there
//! are no parameters to choose wrongly, and verification needs no more than what
//! [`ed25519_dalek`] compiles into the application.

use ed25519_dalek::Signer;

use crate::{Error, Result};

/// Wire name of the only algorithm this reader knows.
///
/// Written into the signature file and checked on the way back in. A reader that
/// meets something else refuses it instead of interpreting the digits under it,
/// which is what lets the scheme be replaced later without a build in the field
/// mistaking one for the other.
const ALGORITHM: &str = "ed25519";

/// The half that verifies. Public, so it ships in the application and is quoted
/// in workflows and release notes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PublicKey(ed25519_dalek::VerifyingKey);

/// The half that signs. Held only by the publishing workflow.
#[derive(Clone)]
pub struct SecretKey(ed25519_dalek::SigningKey);

/// A detached signature over the index as served.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signature(ed25519_dalek::Signature);

impl SecretKey {
    /// Make a key.
    ///
    /// An ed25519 secret key is thirty-two uniform random bytes and nothing is
    /// derived from anything else, so the operating system's random source is
    /// the whole of it. A machine that cannot produce randomness cannot be
    /// trusted to produce a key either, which is why this crashes rather than
    /// reporting.
    #[must_use]
    pub fn generate() -> Self {
        let mut seed = [0u8; ed25519_dalek::SECRET_KEY_LENGTH];
        getrandom::fill(&mut seed).expect("the operating system has a random source");
        SecretKey(ed25519_dalek::SigningKey::from_bytes(&seed))
    }

    /// Read a key out of whatever the caller was handed.
    ///
    /// Hex because a key has to survive a GitHub secret and an environment
    /// variable, both of which carry text.
    pub fn from_hex(text: &str) -> Result<Self> {
        let bytes = from_hex::<{ ed25519_dalek::SECRET_KEY_LENGTH }>(text, "signing key")?;
        // Every thirty-two byte string is a valid secret key, so there is
        // nothing left to reject here.
        Ok(SecretKey(ed25519_dalek::SigningKey::from_bytes(&bytes)))
    }

    /// The form that goes into a secret store. Never log this.
    #[must_use]
    pub fn to_hex(&self) -> String {
        crate::hex(self.0.as_bytes())
    }

    #[must_use]
    pub fn public_key(&self) -> PublicKey {
        PublicKey(self.0.verifying_key())
    }

    /// Sign the index exactly as it will be served.
    ///
    /// Takes bytes rather than an [`Index`](crate::Index) on purpose: a parsed
    /// index re-serialized is not guaranteed to be the file anyone downloads,
    /// and signing it would mean the signature covers something nobody sees.
    #[must_use]
    pub fn sign(&self, index: &[u8]) -> Signature {
        Signature(self.0.sign(index))
    }
}

/// Redacted by hand rather than by trusting the one underneath.
///
/// `ed25519_dalek` hides its own key material in `Debug` today; this says the
/// same thing here, where the guarantee is needed, so that it cannot be lost to
/// a dependency bump.
impl std::fmt::Debug for SecretKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SecretKey(<redacted>)")
    }
}

impl PublicKey {
    pub fn from_hex(text: &str) -> Result<Self> {
        let bytes = from_hex::<{ ed25519_dalek::PUBLIC_KEY_LENGTH }>(text, "public key")?;
        // Unlike a secret key, most thirty-two byte strings are not a point on
        // the curve, so this one really can be malformed.
        ed25519_dalek::VerifyingKey::from_bytes(&bytes).map(PublicKey).map_err(|_| {
            Error::BadKeyMaterial { what: "public key", why: "not a point on the ed25519 curve" }
        })
    }

    #[must_use]
    pub fn to_hex(&self) -> String {
        crate::hex(self.0.as_bytes())
    }

    /// Prove that this key signed these exact bytes.
    ///
    /// `verify_strict` rather than `verify`: it refuses the small-order public
    /// keys under which one signature verifies against several keys. Nothing in
    /// this project needs that ambiguity, and refusing it costs a comparison.
    pub fn verify(&self, index: &[u8], signature: &Signature) -> Result<()> {
        self.0.verify_strict(index, &signature.0).map_err(|_| Error::SignatureMismatch)
    }
}

impl std::fmt::Display for PublicKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_hex())
    }
}

impl Signature {
    /// Read a signature file.
    ///
    /// One line, `ed25519 <128 hex digits>`, because a release asset is
    /// something people curl and paste into an issue, and because naming the
    /// algorithm is what lets a later one be told apart from this one.
    pub fn parse(text: &str) -> Result<Self> {
        let malformed = || Error::BadKeyMaterial {
            what: "signature",
            why: "expected one line reading: ed25519 <128 lowercase hex digits>",
        };
        let mut fields = text.split_whitespace();
        let (Some(ALGORITHM), Some(digits), None) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(malformed());
        };
        let bytes = from_hex::<{ ed25519_dalek::SIGNATURE_LENGTH }>(digits, "signature")?;
        Ok(Signature(ed25519_dalek::Signature::from_bytes(&bytes)))
    }
}

impl std::fmt::Display for Signature {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{ALGORITHM} {}", crate::hex(&self.0.to_bytes()))
    }
}

/// Exactly `N` bytes of lowercase hex, or a refusal.
///
/// Lowercase because every hex field this project publishes is lowercase - a
/// digest, a commit - and one rule is easier to state than two. The offending
/// text is never repeated into the error: one caller of this is a secret key,
/// and an error message ends up in a log.
fn from_hex<const N: usize>(text: &str, what: &'static str) -> Result<[u8; N]> {
    let malformed =
        || Error::BadKeyMaterial { what, why: "expected lowercase hex of the right length" };
    let nibble = |c: u8| match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        _ => None,
    };

    let (pairs, rest) = text.as_bytes().as_chunks::<2>();
    if pairs.len() != N || !rest.is_empty() {
        return Err(malformed());
    }

    let mut bytes = [0u8; N];
    for (byte, [high, low]) in bytes.iter_mut().zip(pairs) {
        *byte = nibble(*high).ok_or_else(malformed)? << 4 | nibble(*low).ok_or_else(malformed)?;
    }
    Ok(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;

    const INDEX: &[u8] = br#"{"schema": 2, "items": []}"#;

    #[test]
    fn a_signature_survives_the_file_it_is_published_as() {
        let key = SecretKey::generate();
        let signature = key.sign(INDEX);

        // What the workflow writes, and what the application reads back.
        let published = signature.to_string();
        assert!(published.starts_with("ed25519 "), "{published}");
        assert_eq!(Signature::parse(&published).unwrap(), signature);

        // A trailing newline is what a file written by a shell has.
        assert_eq!(Signature::parse(&format!("{published}\n")).unwrap(), signature);

        let published_key = key.public_key().to_hex();
        let key = PublicKey::from_hex(&published_key).unwrap();
        assert_eq!(key.to_hex(), published_key);
        assert!(key.verify(INDEX, &signature).is_ok());
    }

    #[test]
    fn a_secret_key_survives_the_environment_variable_it_arrives_in() {
        let key = SecretKey::generate();
        let recovered = SecretKey::from_hex(&key.to_hex()).unwrap();
        assert_eq!(recovered.public_key(), key.public_key());
        assert_eq!(recovered.sign(INDEX), key.sign(INDEX));
    }

    /// The whole point. A mirror that rewrites one digest in the index serves a
    /// file the published signature no longer covers.
    #[test]
    fn a_tampered_index_no_longer_verifies() {
        let key = SecretKey::generate();
        let signature = key.sign(INDEX);
        let public = key.public_key();

        let mut tampered = INDEX.to_vec();
        let at = tampered.len() - 2;
        tampered[at] ^= 1;
        assert!(matches!(public.verify(&tampered, &signature), Err(Error::SignatureMismatch)));

        // Even a byte nobody reads. The signature covers the file, not its
        // meaning, which is why there is no canonicalisation to argue about.
        let whitespace = [INDEX, b" "].concat();
        assert!(matches!(public.verify(&whitespace, &signature), Err(Error::SignatureMismatch)));
    }

    #[test]
    fn another_key_does_not_speak_for_the_catalog() {
        let signature = SecretKey::generate().sign(INDEX);
        let impostor = SecretKey::generate().public_key();
        assert!(matches!(impostor.verify(INDEX, &signature), Err(Error::SignatureMismatch)));
    }

    /// Key material arrives from an environment variable and a downloaded file,
    /// so every shape of nonsense has to be a refusal rather than a panic.
    #[test]
    fn malformed_key_material_is_refused_and_never_panics() {
        let hex = |n: usize| "ab".repeat(n);

        for bad in ["", &hex(31), &hex(33), &"g".repeat(64), &"AB".repeat(32), " "] {
            assert!(SecretKey::from_hex(bad).is_err(), "accepted a signing key of {bad:?}");
            assert!(PublicKey::from_hex(bad).is_err(), "accepted a public key of {bad:?}");
        }

        // The right length and the right alphabet, and still not a key: a
        // public key is a compressed curve point, and this one decompresses to
        // nothing. Every 32-byte string *is* a valid secret key, so the same
        // text is only refused on one side.
        let not_a_point = "02".repeat(32);
        assert!(SecretKey::from_hex(&not_a_point).is_ok());
        assert!(matches!(
            PublicKey::from_hex(&not_a_point),
            Err(Error::BadKeyMaterial { what: "public key", .. })
        ));

        for bad in [
            "",
            "ed25519",
            &hex(64),
            &format!("ed25519 {}", hex(63)),
            &format!("rsa {}", hex(64)),
            &format!("ed25519 {} ed25519 {}", hex(64), hex(64)),
        ] {
            assert!(Signature::parse(bad).is_err(), "accepted a signature of {bad:?}");
        }
    }

    /// A refusal is printed by a command line tool and captured by a workflow
    /// log. The value it refused may be the signing key itself.
    #[test]
    fn a_refusal_never_repeats_the_material_it_refused() {
        let secret = "ab".repeat(31);
        let message = SecretKey::from_hex(&secret).unwrap_err().to_string();
        assert!(!message.contains(&secret), "the error quoted the key back: {message}");
    }
}
