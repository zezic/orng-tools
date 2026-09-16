// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Who may change what, and the check that enforces it.
//!
//! GitHub has no path-scoped write permission. `CODEOWNERS` looks like it does,
//! but it only routes review and can require an approval; it grants nothing. So
//! nobody has write access to the catalog, every change arrives as a fork pull
//! request, and this decides whether the account that opened it may touch the
//! paths it touches.
//!
//! Two things make that sound, and both are easy to get subtly wrong:
//!
//! - **`owners.toml` is read from the base branch, never from the pull request.**
//!   Reading the pull request's own copy would let a contributor add themselves
//!   as owner of someone else's directory in the commit that edits it. That is
//!   the caller's job; this module only reads what it is handed, and says so
//!   here because there is nowhere else to say it.
//! - **An author id is a lookup key, never a credential.** It resolves to a
//!   GitHub account and the check compares the numeric id GitHub authenticated.
//!   A login can be changed by its holder and the freed name claimed by someone
//!   else; a numeric id is immutable and never reused.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::{AuthorId, CONTENT_DIR, Error, Result};

/// The account that may change one author's directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Owner {
    /// For a human reading the file. Never compared.
    pub github: String,
    /// What the check compares. Immutable and never reused, unlike the login.
    pub id: u64,
}

/// `owners.toml`: every author the catalog knows, and who holds them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Owners(BTreeMap<AuthorId, Owner>);

impl Owners {
    pub fn parse(text: &str) -> Result<Self> {
        toml::from_str(text)
            .map_err(|source| Error::Manifest { path: OWNERS_FILE.to_owned(), source })
    }

    pub fn get(&self, author: &AuthorId) -> Option<&Owner> {
        self.0.get(author)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// File holding the mapping, at the repository root.
pub const OWNERS_FILE: &str = "owners.toml";

/// What a pull request is allowed to do.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Authorization {
    /// Every changed path is in a directory this account owns.
    Allowed,
    /// Nothing is wrong, but at least one directory has no owner yet. A first
    /// contribution cannot be self-approved, and it is also exactly when someone
    /// should look at the work.
    NeedsReview { new_authors: Vec<AuthorId> },
    /// At least one path this account may not change.
    Refused { reasons: Vec<Refusal> },
}

/// Why one path was refused. One per path, so a contributor is told about every
/// problem at once rather than one per pushed fix.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Refusal {
    /// Outside the content tree: workflows, the owners file, the generated
    /// index. Locked by a repository ruleset as well; refused here so that a
    /// ruleset that is ever relaxed does not quietly become the only guard.
    OutsideContent { path: String },
    /// Inside the content tree but not shaped like an item.
    Malformed { path: String },
    /// Someone else's directory.
    NotOwned { path: String, author: AuthorId, owner: u64 },
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Refusal::OutsideContent { path } => {
                write!(f, "{path}: only maintainers may change anything outside {CONTENT_DIR}/")
            }
            Refusal::Malformed { path } => {
                write!(f, "{path}: expected {CONTENT_DIR}/<author>/<slug>/<file>")
            }
            Refusal::NotOwned { path, author, owner } => {
                write!(f, "{path}: {author} belongs to account {owner}")
            }
        }
    }
}

/// Decide whether `actor` may change every path in `changed`.
///
/// `changed` is every path the pull request touches, added, modified, deleted or
/// renamed, with both sides of a rename. A path that is not reported cannot be
/// checked.
pub fn authorize(owners: &Owners, changed: &[&str], actor: u64) -> Authorization {
    let mut refusals = Vec::new();
    let mut new_authors = Vec::new();

    for path in changed {
        match author_of(path) {
            Err(refusal) => refusals.push(refusal),
            Ok(author) => match owners.get(&author) {
                Some(owner) if owner.id == actor => {}
                Some(owner) => refusals.push(Refusal::NotOwned {
                    path: (*path).to_owned(),
                    author,
                    owner: owner.id,
                }),
                // No owner yet, so there is nobody to be refused by. A human
                // decides, and deciding creates the owner.
                None => {
                    if !new_authors.contains(&author) {
                        new_authors.push(author);
                    }
                }
            },
        }
    }

    // A refusal outranks a review: a pull request that touches one directory it
    // may not change is refused whatever else is in it.
    if !refusals.is_empty() {
        return Authorization::Refused { reasons: refusals };
    }
    if !new_authors.is_empty() {
        return Authorization::NeedsReview { new_authors };
    }
    Authorization::Allowed
}

/// The author whose directory a path belongs to.
///
/// Matched segment by segment. A prefix comparison would let `content/mallory`
/// satisfy an owner of `content/mal`, which is the whole check defeated by a
/// name someone is free to choose.
fn author_of(path: &str) -> std::result::Result<AuthorId, Refusal> {
    let mut segments = path.split('/');
    if segments.next() != Some(CONTENT_DIR) {
        return Err(Refusal::OutsideContent { path: path.to_owned() });
    }
    let malformed = || Refusal::Malformed { path: path.to_owned() };

    let author = segments.next().filter(|s| !s.is_empty()).ok_or_else(malformed)?;
    // An item is a file inside a slug directory, so there is always more to come.
    let slug = segments.next().filter(|s| !s.is_empty()).ok_or_else(malformed)?;
    let file = segments.next().filter(|s| !s.is_empty()).ok_or_else(malformed)?;

    let _ = (slug, file);
    AuthorId::new(author).map_err(|_| malformed())
}

#[cfg(test)]
mod tests {
    use super::*;

    const OWNERS: &str = r#"
        caviio = { github = "j4n1k1", id = 97474499 }
        someone-else = { github = "other", id = 12 }
    "#;

    const CAVIIO: u64 = 97474499;

    fn owners() -> Owners {
        Owners::parse(OWNERS).unwrap()
    }

    #[test]
    fn an_owner_may_change_their_own_item() {
        let changed = ["content/caviio/volshaper/VOLSHAPER.bwdevice", "content/caviio/volshaper/orange.toml"];
        assert_eq!(authorize(&owners(), &changed, CAVIIO), Authorization::Allowed);
    }

    /// The reason this check is compiled and tested rather than written in the
    /// workflow. A prefix match would make every owner's directory reachable by
    /// anyone willing to pick a longer name.
    #[test]
    fn a_longer_name_is_not_the_same_author() {
        let changed = ["content/caviio2/volshaper/X.bwdevice"];
        let outcome = authorize(&owners(), &changed, CAVIIO);
        assert!(
            matches!(outcome, Authorization::NeedsReview { .. }),
            "caviio2 is an unknown author, not caviio: {outcome:?}"
        );

        // And with that author owned by somebody else, it is a refusal rather
        // than something a prefix could wave through.
        let owners = Owners::parse(
            r#"caviio = { github = "j4n1k1", id = 97474499 }
               caviio2 = { github = "mallory", id = 999 }"#,
        )
        .unwrap();
        assert!(matches!(
            authorize(&owners, &changed, CAVIIO),
            Authorization::Refused { .. }
        ));
    }

    #[test]
    fn another_authors_directory_is_refused() {
        let changed = ["content/someone-else/thing/X.bwdevice"];
        let Authorization::Refused { reasons } = authorize(&owners(), &changed, CAVIIO) else {
            panic!("expected a refusal");
        };
        assert!(matches!(reasons.as_slice(), [Refusal::NotOwned { owner: 12, .. }]));
    }

    #[test]
    fn nothing_outside_the_content_tree_is_self_service() {
        for path in ["owners.toml", ".github/workflows/validate.yml", "index.json", "README.md"] {
            let outcome = authorize(&owners(), &[path], CAVIIO);
            assert!(
                matches!(outcome, Authorization::Refused { .. }),
                "{path} was not refused"
            );
        }
    }

    #[test]
    fn an_unknown_author_needs_a_human() {
        let changed = ["content/newcomer/first/X.bwdevice"];
        let Authorization::NeedsReview { new_authors } = authorize(&owners(), &changed, CAVIIO)
        else {
            panic!("expected a review");
        };
        assert_eq!(new_authors, [AuthorId::new("newcomer").unwrap()]);
    }

    /// One bad path spoils the pull request. Otherwise a contributor could hide
    /// a change to someone else's directory behind a legitimate one of their own.
    #[test]
    fn a_refusal_outranks_a_review() {
        let changed = [
            "content/newcomer/first/X.bwdevice",
            "content/someone-else/thing/X.bwdevice",
        ];
        assert!(matches!(
            authorize(&owners(), &changed, CAVIIO),
            Authorization::Refused { .. }
        ));
    }

    #[test]
    fn a_path_that_is_not_an_item_is_refused() {
        for path in ["content", "content/caviio", "content/caviio/volshaper"] {
            assert!(
                matches!(authorize(&owners(), &[path], CAVIIO), Authorization::Refused { .. }),
                "{path} was not refused"
            );
        }
    }

    #[test]
    fn owners_round_trip_through_toml() {
        let parsed = owners();
        assert_eq!(
            parsed.get(&AuthorId::new("caviio").unwrap()),
            Some(&Owner { github: "j4n1k1".into(), id: CAVIIO })
        );
        assert_eq!(parsed.get(&AuthorId::new("nobody").unwrap()), None);
    }
}
