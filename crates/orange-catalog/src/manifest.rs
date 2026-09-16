//! `orange.toml`: what a document cannot say about itself.
//!
//! Identity, name, kind, description and category live inside the document and
//! are read from there. Restating them here would create two sources of truth
//! that drift, so the fields simply do not exist -- and `deny_unknown_fields`
//! turns an attempt to add them into a failed contribution rather than a field
//! that is silently ignored.

use std::path::Path;

use bitwig_document::BitwigVersion;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{AuthorId, Error, Result};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    /// Increases with every published change to this item.
    pub version: ItemVersion,
    /// Must match the directory the item sits in.
    pub author: AuthorId,
    /// SPDX identifier, or a short name for something that has none.
    pub license: String,
    /// Oldest Bitwig release that can load the document.
    #[serde(with = "serde_bitwig_version")]
    pub min_bitwig: BitwigVersion,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    /// Identities this item replaces.
    ///
    /// A revision that changes the parameter set takes a new identity rather
    /// than reusing the old one, because Bitwig loads a device's structure from
    /// the library by identity -- so reusing it would reach back into projects
    /// that already exist. The old item stays published and points here.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<Uuid>,
}

impl Manifest {
    pub fn read(path: &Path) -> Result<Self> {
        let text = String::from_utf8_lossy(&crate::read(path)?).into_owned();
        toml::from_str(&text)
            .map_err(|source| Error::Manifest { path: path.display().to_string(), source })
    }
}

/// A published revision of one item. Not semver: it orders publications, and the
/// meaning of a change is carried by whether the identity stayed the same.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ItemVersion {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl std::fmt::Display for ItemVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

impl std::str::FromStr for ItemVersion {
    type Err = Error;

    fn from_str(text: &str) -> Result<Self> {
        let parts: Vec<&str> = text.trim().split('.').collect();
        let [major, minor, patch] = parts.as_slice() else {
            return Err(Error::BadVersion(text.to_owned()));
        };
        let number = |part: &str| part.parse().map_err(|_| Error::BadVersion(text.to_owned()));
        Ok(ItemVersion { major: number(major)?, minor: number(minor)?, patch: number(patch)? })
    }
}

impl Serialize for ItemVersion {
    fn serialize<S: serde::Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&self.to_string())
    }
}

impl<'de> Deserialize<'de> for ItemVersion {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> std::result::Result<Self, D::Error> {
        let text = String::deserialize(d)?;
        text.parse().map_err(serde::de::Error::custom)
    }
}

/// `BitwigVersion` lives in another crate, so its serde form is defined here.
pub(crate) mod serde_bitwig_version {
    use super::BitwigVersion;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(
        value: &BitwigVersion,
        s: S,
    ) -> std::result::Result<S::Ok, S::Error> {
        s.serialize_str(&value.to_string())
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(
        d: D,
    ) -> std::result::Result<BitwigVersion, D::Error> {
        let text = String::deserialize(d)?;
        BitwigVersion::parse(&text)
            .ok_or_else(|| serde::de::Error::custom(format!("{text} is not a Bitwig release")))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
        version = "1.2.0"
        author = "caviio"
        license = "CC-BY-4.0"
        min_bitwig = "6.1"
    "#;

    #[test]
    fn reads_what_the_document_cannot_say() {
        let manifest: Manifest = toml::from_str(SAMPLE).unwrap();
        assert_eq!(manifest.version.to_string(), "1.2.0");
        assert_eq!(manifest.author.as_str(), "caviio");
        assert_eq!(manifest.min_bitwig.to_string(), "6.1");
        assert!(manifest.supersedes.is_empty());
    }

    #[test]
    fn refuses_to_restate_the_documents_identity() {
        // The fields are absent by design; an author adding them has misunderstood
        // where identity lives, and must be told rather than silently ignored.
        for extra in ["uuid = \"...\"", "name = \"X\"", "kind = \"device\""] {
            let text = format!("{SAMPLE}\n{extra}");
            assert!(toml::from_str::<Manifest>(&text).is_err(), "accepted {extra}");
        }
    }

    #[test]
    fn rejects_a_version_that_is_not_three_numbers() {
        for bad in ["1.2", "1.2.3.4", "v1.2.3", "1.2.x"] {
            assert!(bad.parse::<ItemVersion>().is_err(), "accepted {bad}");
        }
        assert_eq!(
            "1.2.3".parse::<ItemVersion>().unwrap(),
            ItemVersion { major: 1, minor: 2, patch: 3 }
        );
    }

    #[test]
    fn versions_order_numerically() {
        let v = |s: &str| s.parse::<ItemVersion>().unwrap();
        assert!(v("1.10.0") > v("1.9.0"));
        assert!(v("2.0.0") > v("1.99.99"));
    }
}
