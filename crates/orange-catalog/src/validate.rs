//! The rules that keep identities stable.
//!
//! Orange Catalog's only real job is to guarantee that a UUID means one thing,
//! permanently, across contributors who do not know each other. These checks are
//! that guarantee, expressed once and run both by continuous integration on a
//! pull request and by anyone regenerating the index.
//!
//! Two classes of rule:
//!
//! - **Internal**: is this tree self-consistent? Runs on any checkout.
//! - **Historical**: does this tree keep the promises the last published index
//!   made? Needs that index, and is what stops a merged identity from moving.

use std::collections::BTreeMap;

use uuid::Uuid;

use crate::{AuthorId, Index, Item, Slug};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Severity {
    /// Blocks a merge.
    Error,
    /// Worth a human look, but not wrong.
    Warning,
}

/// Where a problem was found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRef {
    pub author: AuthorId,
    pub slug: Slug,
}

impl std::fmt::Display for ItemRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.author, self.slug)
    }
}

/// Everything that can be wrong with a contribution.
///
/// Typed rather than a message string: the tool reports these, the application
/// may one day explain them, and a new rule should not be expressible as a
/// free-text sentence nobody can match on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Problem {
    /// The manifest claims an author that is not the directory it sits in.
    AuthorMismatch { item: ItemRef, declared: AuthorId },
    /// The document's extension and the directory disagree about kind.
    KindMismatch { item: ItemRef },
    /// Two items claim the same identity. One of them has to change.
    DuplicateIdentity { uuid: Uuid, items: Vec<ItemRef> },
    /// Two items would appear under one name in Bitwig's flat browser.
    DuplicateName { name: String, items: Vec<ItemRef> },
    /// `supersedes` names an identity the catalog does not publish.
    UnknownSupersedes { item: ItemRef, uuid: Uuid },
    /// An item declares itself as its own replacement.
    SelfSupersedes { item: ItemRef },
    /// A published identity moved to a different item. Never allowed: projects
    /// resolve devices by identity, so this silently breaks them.
    IdentityMoved { uuid: Uuid, was: ItemRef, now: ItemRef },
    /// A published item changed its identity. Same breakage, other direction.
    IdentityChanged { item: ItemRef, was: Uuid, now: Uuid },
    /// The document changed without the version being raised, so no consumer
    /// can tell an update happened.
    ContentChangedWithoutVersionBump { item: ItemRef, version: String },
    /// The version went backwards or stood still across a content change.
    VersionWentBackwards { item: ItemRef, was: String, now: String },
}

impl Problem {
    pub fn severity(&self) -> Severity {
        match self {
            // A name collision is ugly in the browser but breaks nothing, and
            // the application can resolve it at install time by renaming.
            Problem::DuplicateName { .. } => Severity::Warning,
            _ => Severity::Error,
        }
    }

    pub fn item(&self) -> Option<&ItemRef> {
        match self {
            Problem::AuthorMismatch { item, .. }
            | Problem::KindMismatch { item }
            | Problem::UnknownSupersedes { item, .. }
            | Problem::SelfSupersedes { item }
            | Problem::IdentityChanged { item, .. }
            | Problem::ContentChangedWithoutVersionBump { item, .. }
            | Problem::VersionWentBackwards { item, .. } => Some(item),
            Problem::IdentityMoved { now, .. } => Some(now),
            Problem::DuplicateIdentity { .. } | Problem::DuplicateName { .. } => None,
        }
    }
}

impl std::fmt::Display for Problem {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Problem::AuthorMismatch { item, declared } => {
                write!(f, "{item}: manifest says author {declared}, directory says {}", item.author)
            }
            Problem::KindMismatch { item } => {
                write!(f, "{item}: the document is not the kind its extension claims")
            }
            Problem::DuplicateIdentity { uuid, items } => {
                write!(f, "{uuid} is claimed by {}", join(items))
            }
            Problem::DuplicateName { name, items } => {
                write!(f, "{name:?} is used by {}; Bitwig's browser is flat", join(items))
            }
            Problem::UnknownSupersedes { item, uuid } => {
                write!(f, "{item}: supersedes {uuid}, which the catalog does not publish")
            }
            Problem::SelfSupersedes { item } => write!(f, "{item}: supersedes itself"),
            Problem::IdentityMoved { uuid, was, now } => {
                write!(f, "{uuid} was published by {was} and is now claimed by {now}")
            }
            Problem::IdentityChanged { item, was, now } => {
                write!(f, "{item}: published as {was}, now {now}; a published identity is permanent")
            }
            Problem::ContentChangedWithoutVersionBump { item, version } => {
                write!(f, "{item}: the document changed but the version is still {version}")
            }
            Problem::VersionWentBackwards { item, was, now } => {
                write!(f, "{item}: version went from {was} to {now}")
            }
        }
    }
}

fn join(items: &[ItemRef]) -> String {
    items.iter().map(ItemRef::to_string).collect::<Vec<_>>().join(", ")
}

/// The outcome of a validation run.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Report {
    pub problems: Vec<Problem>,
}

impl Report {
    /// Whether a merge may proceed.
    pub fn is_mergeable(&self) -> bool {
        !self.problems.iter().any(|p| p.severity() == Severity::Error)
    }

    pub fn errors(&self) -> impl Iterator<Item = &Problem> {
        self.problems.iter().filter(|p| p.severity() == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &Problem> {
        self.problems.iter().filter(|p| p.severity() == Severity::Warning)
    }
}

/// Check a tree against itself.
pub fn check(items: &[Item]) -> Report {
    let mut problems = Vec::new();
    let reference = |item: &Item| ItemRef { author: item.author.clone(), slug: item.slug.clone() };

    for item in items {
        if item.manifest.author != item.author {
            problems.push(Problem::AuthorMismatch {
                item: reference(item),
                declared: item.manifest.author.clone(),
            });
        }
        if item.manifest.supersedes.contains(&item.identity.uuid) {
            problems.push(Problem::SelfSupersedes { item: reference(item) });
        }
    }

    problems.extend(duplicates(items, |item| item.identity.uuid, |uuid, group| {
        Problem::DuplicateIdentity { uuid, items: group }
    }));
    problems.extend(duplicates(items, |item| item.identity.name.clone(), |name, group| {
        Problem::DuplicateName { name, items: group }
    }));

    // Supersede targets must exist, so that the application can always show a
    // user what replaced the thing they have.
    let published: std::collections::BTreeSet<Uuid> =
        items.iter().map(|item| item.identity.uuid).collect();
    for item in items {
        for uuid in &item.manifest.supersedes {
            if !published.contains(uuid) && *uuid != item.identity.uuid {
                problems.push(Problem::UnknownSupersedes {
                    item: reference(item),
                    uuid: *uuid,
                });
            }
        }
    }

    Report { problems }
}

/// Check a tree against what was last published.
///
/// This is where permanence is enforced. Without the previous index there is no
/// history to break, so a first run is vacuously clean.
pub fn check_against(items: &[Item], previous: &Index) -> Report {
    let mut problems = Vec::new();
    let reference = |item: &Item| ItemRef { author: item.author.clone(), slug: item.slug.clone() };

    let by_slug: BTreeMap<(&AuthorId, &Slug), &crate::IndexEntry> = previous
        .items
        .iter()
        .map(|entry| ((&entry.author, &entry.slug), entry))
        .collect();
    let by_uuid = previous.by_uuid();

    for item in items {
        let here = reference(item);

        // An identity that was published elsewhere may not be adopted here.
        if let Some(entry) = by_uuid.get(&item.identity.uuid)
            && (entry.author != item.author || entry.slug != item.slug)
        {
            problems.push(Problem::IdentityMoved {
                uuid: item.identity.uuid,
                was: ItemRef { author: entry.author.clone(), slug: entry.slug.clone() },
                now: here.clone(),
            });
        }

        let Some(entry) = by_slug.get(&(&item.author, &item.slug)) else {
            continue; // New item; nothing has been promised about it yet.
        };

        if entry.uuid != item.identity.uuid {
            problems.push(Problem::IdentityChanged {
                item: here.clone(),
                was: entry.uuid,
                now: item.identity.uuid,
            });
        }

        let content_changed = entry.digest != item.digest();
        match item.manifest.version.cmp(&entry.version) {
            std::cmp::Ordering::Less => problems.push(Problem::VersionWentBackwards {
                item: here,
                was: entry.version.to_string(),
                now: item.manifest.version.to_string(),
            }),
            std::cmp::Ordering::Equal if content_changed => {
                problems.push(Problem::ContentChangedWithoutVersionBump {
                    item: here,
                    version: item.manifest.version.to_string(),
                })
            }
            _ => {}
        }
    }

    Report { problems }
}

/// Group items by a key and report every key claimed more than once.
fn duplicates<K, F, M>(items: &[Item], key: F, make: M) -> Vec<Problem>
where
    K: Ord,
    F: Fn(&Item) -> K,
    M: Fn(K, Vec<ItemRef>) -> Problem,
{
    let mut groups: BTreeMap<K, Vec<ItemRef>> = BTreeMap::new();
    for item in items {
        groups
            .entry(key(item))
            .or_default()
            .push(ItemRef { author: item.author.clone(), slug: item.slug.clone() });
    }
    groups
        .into_iter()
        .filter(|(_, group)| group.len() > 1)
        .map(|(k, group)| make(k, group))
        .collect()
}
