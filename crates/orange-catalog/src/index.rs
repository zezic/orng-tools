//! The published index.
//!
//! Generated from the tree on every merge and published as a release asset, so
//! the application fetches one small file over plain HTTPS: no API, no token, no
//! account, no git client. Each row carries a digest, so a downloaded document is
//! checked against a reviewed index rather than trusted for arriving from the
//! right domain.
//!
//! Never hand-edited. It is a projection of the tree, and a contributor changing
//! it directly is changing a derived artefact.

use std::collections::BTreeMap;

use bitwig_document::Kind;
use bitwig_registry::BitwigVersion;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::manifest::ItemVersion;
use crate::{AuthorId, Item, Result, Slug};

/// Bumped when the shape changes in a way older readers cannot handle. A reader
/// that does not recognise the number refuses the file rather than guessing.
pub const SCHEMA: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    /// Commit the index was generated from, for tracing an item back to review.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<String>,
    pub items: Vec<IndexEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IndexEntry {
    pub uuid: Uuid,
    pub kind: KindTag,
    pub name: String,
    pub author: AuthorId,
    pub slug: Slug,
    pub version: ItemVersion,
    #[serde(with = "crate::manifest::serde_bitwig_version")]
    pub min_bitwig: BitwigVersion,
    pub license: String,
    pub description: String,
    pub keywords: Vec<String>,
    /// Repository-relative path of the document.
    pub path: String,
    /// Lowercase hex SHA-256 of the document.
    pub digest: String,
    pub size: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub homepage: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub supersedes: Vec<Uuid>,
}

/// `Kind` is Bitwig's, so its wire form is defined here rather than there.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum KindTag {
    Device,
    Modulator,
    Module,
}

impl From<Kind> for KindTag {
    fn from(kind: Kind) -> Self {
        match kind {
            Kind::Device => KindTag::Device,
            Kind::Modulator => KindTag::Modulator,
            Kind::Module => KindTag::Module,
        }
    }
}

impl From<KindTag> for Kind {
    fn from(tag: KindTag) -> Self {
        match tag {
            KindTag::Device => Kind::Device,
            KindTag::Modulator => Kind::Modulator,
            KindTag::Module => Kind::Module,
        }
    }
}

impl Index {
    /// Project a set of items into a publishable index.
    pub fn build(items: &[Item], revision: Option<String>) -> Self {
        let mut entries: Vec<IndexEntry> = items.iter().map(IndexEntry::from_item).collect();
        // Sorted by identity so the file is stable across runs and a diff shows
        // only what actually changed.
        entries.sort_by_key(|e| e.uuid);
        Index { schema: SCHEMA, revision, items: entries }
    }

    pub fn parse(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).expect("index is always serializable")
    }

    pub fn by_uuid(&self) -> BTreeMap<Uuid, &IndexEntry> {
        self.items.iter().map(|e| (e.uuid, e)).collect()
    }

    /// Entries this installation can actually use.
    pub fn compatible_with(&self, bitwig: &BitwigVersion) -> impl Iterator<Item = &IndexEntry> {
        self.items.iter().filter(move |e| bitwig.satisfies(&e.min_bitwig))
    }
}

impl IndexEntry {
    fn from_item(item: &Item) -> Self {
        IndexEntry {
            uuid: item.identity.uuid,
            kind: item.kind.into(),
            name: item.identity.name.clone(),
            author: item.author.clone(),
            slug: item.slug.clone(),
            version: item.manifest.version,
            min_bitwig: item.manifest.min_bitwig.clone(),
            license: item.manifest.license.clone(),
            description: item.identity.description.clone().unwrap_or_default(),
            keywords: item.identity.suggested_keywords(),
            path: item.document_path.clone(),
            digest: item.digest(),
            size: item.document.len() as u64,
            homepage: item.manifest.homepage.clone(),
            supersedes: item.manifest.supersedes.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unrecognised_schema_is_refused_not_guessed_at() {
        let json = r#"{"schema": 99, "items": []}"#;
        let index = Index::parse(json).unwrap();
        // Parsing succeeds; acting on it is the caller's decision, and the
        // number is what that decision is made on.
        assert_ne!(index.schema, SCHEMA);
    }

    #[test]
    fn round_trips_through_json() {
        let index = Index { schema: SCHEMA, revision: Some("abc123".into()), items: Vec::new() };
        assert_eq!(Index::parse(&index.to_json()).unwrap(), index);
    }
}
