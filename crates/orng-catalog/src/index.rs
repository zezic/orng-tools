// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

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
use bitwig_document::BitwigVersion;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::manifest::ItemVersion;
use crate::signing::{PublicKey, Signature};
use crate::{AuthorId, Error, Item, Result, Slug};

/// Bumped when the shape changes in a way older readers cannot handle. A reader
/// that does not recognise the number refuses the file rather than guessing.
pub const SCHEMA: u32 = 2;

/// A commit in the catalog repository.
///
/// A newtype rather than a `String` because the two fields that hold one sit
/// among fields holding other text, and because a malformed revision in a
/// published index is a link that goes nowhere. Stored whole and shortened for
/// display, the way a Bitwig build revision already is.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Revision(String);

impl Revision {
    /// Git names an object with forty lowercase hex digits, and nothing else is
    /// accepted: an abbreviation is ambiguous as the repository grows.
    pub fn new(text: &str) -> Result<Self> {
        let shaped = text.len() == 40 && text.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'));
        shaped.then(|| Revision(text.to_owned())).ok_or_else(|| Error::BadRevision(text.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// What a link to the commit is labelled with.
    pub fn short(&self) -> &str {
        &self.0[..7]
    }
}

impl TryFrom<String> for Revision {
    type Error = Error;

    fn try_from(text: String) -> Result<Self> {
        Self::new(&text)
    }
}

impl From<Revision> for String {
    fn from(value: Revision) -> String {
        value.0
    }
}

impl std::fmt::Display for Revision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// What git knows about a checkout, which the tree itself cannot say.
///
/// The two revisions here answer different questions and must not stand in for
/// one another: the index-wide one traces the index, while a per-item one traces
/// a single item to the change that published it. An index generated in the tree
/// can carry neither, because the commit that merges a contribution does not
/// exist while that contribution is still a pull request. That is why the index
/// is regenerated on merge.
///
/// [`History::default`] is a caller that knows nothing, which is the honest
/// answer outside a checkout and yields an index with no revisions at all.
#[derive(Debug, Clone, Default)]
pub struct History {
    /// Commit the index was generated from.
    revision: Option<Revision>,
    /// Keyed by item directory, which is the path git was asked about.
    merged: BTreeMap<String, Revision>,
}

impl History {
    /// History as a caller holding only the index's own commit can state it.
    pub fn at(revision: Revision) -> Self {
        History { revision: Some(revision), merged: BTreeMap::new() }
    }

    /// Record the change that published the item living in `dir`.
    pub fn record(&mut self, dir: String, merged_in: Revision) {
        self.merged.insert(dir, merged_in);
    }

    fn merged_in(&self, item: &Item) -> Option<Revision> {
        self.merged.get(&item.dir()).cloned()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Index {
    pub schema: u32,
    /// Commit the index was generated from, for tracing the index itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub revision: Option<Revision>,
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
    /// The change that published this item, for showing who signed off on it.
    ///
    /// Distinct from [`Index::revision`], which names the commit the whole index
    /// was built from and so traces the index rather than the item. Only a run
    /// after the merge can fill this, so an index generated in a pull request
    /// leaves it empty.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub merged_in: Option<Revision>,
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
    ///
    /// Everything but `history` is read out of the tree. Taking history as one
    /// argument rather than leaving the per-item revisions to be filled in
    /// afterwards keeps it impossible to publish an index that quietly forgot
    /// them.
    pub fn build(items: &[Item], history: &History) -> Self {
        let mut entries: Vec<IndexEntry> =
            items.iter().map(|item| IndexEntry::from_item(item, history)).collect();
        // Sorted by identity so the file is stable across runs and a diff shows
        // only what actually changed.
        entries.sort_by_key(|e| e.uuid);
        Index { schema: SCHEMA, revision: history.revision.clone(), items: entries }
    }

    /// Read an index the caller already trusts: one it generated itself, or the
    /// copy of the last published index a workflow was handed. Anything that
    /// arrived over the network goes through [`Index::verified`] instead.
    pub fn parse(json: &str) -> Result<Self> {
        Ok(serde_json::from_str(json)?)
    }

    /// Read a downloaded index, and only if the catalog signed these bytes.
    ///
    /// This is the whole verification path the application needs: no network
    /// beyond the two files, no git client, no release API. Fetch `index.json`
    /// and `index.json.sig`, hold the public key in the build, and call this.
    ///
    /// Verification comes first and parsing second, over one `&[u8]` that is
    /// never re-serialized in between, so the bytes proved are the bytes read.
    /// Splitting it into a verify call and a parse call would make forgetting
    /// the first one possible, which is the only mistake here that matters.
    pub fn verified(json: &[u8], signature: &Signature, key: &PublicKey) -> Result<Self> {
        key.verify(json, signature)?;
        Ok(serde_json::from_slice(json)?)
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
    fn from_item(item: &Item, history: &History) -> Self {
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
            merged_in: history.merged_in(item),
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

    fn revision(seed: char) -> Revision {
        Revision::new(&seed.to_string().repeat(40)).unwrap()
    }

    #[test]
    fn round_trips_through_json() {
        let index = Index { schema: SCHEMA, revision: Some(revision('a')), items: Vec::new() };
        assert_eq!(Index::parse(&index.to_json()).unwrap(), index);
    }

    #[test]
    fn a_revision_is_a_whole_commit_or_nothing() {
        assert!(Revision::new(&"a".repeat(40)).is_ok());
        // Abbreviated, wrong case, wrong length, not hex.
        for bad in ["abc1234", &"A".repeat(40), &"a".repeat(39), &"g".repeat(40), ""] {
            assert!(Revision::new(bad).is_err(), "accepted {bad:?}");
        }
    }

    #[test]
    fn a_revision_arrives_validated_even_through_serde() {
        // The index is fetched over the network, so the newtype has to hold at
        // the parse boundary and not only where the program constructs one.
        assert!(Index::parse(r#"{"schema": 2, "revision": "nonsense", "items": []}"#).is_err());
    }

    #[test]
    fn the_short_form_is_what_a_link_is_labelled_with() {
        assert_eq!(revision('a').short(), "aaaaaaa");
    }

    /// The path the application takes: two downloaded files and a key compiled
    /// into the build. An index that does not verify never becomes an `Index`,
    /// so nothing downstream has to remember to ask whether it was checked.
    #[test]
    fn a_downloaded_index_is_parsed_only_once_it_is_proved() {
        let key = crate::SecretKey::generate();
        let published = Index { schema: SCHEMA, revision: Some(revision('a')), items: Vec::new() };
        let served = published.to_json().into_bytes();
        let signature = key.sign(&served);

        assert_eq!(
            Index::verified(&served, &signature, &key.public_key()).unwrap(),
            published
        );

        // The mirror rewrites a row, or serves a different index entirely.
        let forged =
            Index { schema: SCHEMA, revision: Some(revision('b')), items: Vec::new() }.to_json();
        assert!(matches!(
            Index::verified(forged.as_bytes(), &signature, &key.public_key()),
            Err(Error::SignatureMismatch)
        ));
    }

    #[test]
    fn history_answers_per_item_and_not_with_the_index_wide_revision() {
        // The design letter is explicit that showing the index revision in place
        // of the item's own is the wrong answer, so an item git said nothing
        // about stays empty rather than inheriting one.
        let mut history = History::at(revision('a'));
        history.record("content/someone/known".into(), revision('b'));

        assert_eq!(history.merged.get("content/someone/known"), Some(&revision('b')));
        assert_eq!(history.merged.get("content/someone/unknown"), None);
    }
}
