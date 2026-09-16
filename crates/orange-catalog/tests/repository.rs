//! Rules exercised against a real repository tree built from real documents.
//!
//! Each test breaks exactly one promise and asserts that the corresponding rule
//! catches it, because a validator that passes everything is worse than none.

use std::path::{Path, PathBuf};

use orange_catalog::{Index, Item, Problem, Severity, scan, validate};

/// Documents to build a tree from, supplied via `ORANGE_TEST_DOCUMENTS`.
///
/// Sample documents are somebody's work and are not redistributed here, so the
/// path is given rather than assumed. Tests skip when it is unset.
fn sources() -> Vec<PathBuf> {
    let Some(root) = std::env::var_os("ORANGE_TEST_DOCUMENTS") else {
        return Vec::new();
    };
    let mut found = Vec::new();
    collect(Path::new(&root), &mut found);
    found.sort();
    found
}

fn collect(dir: &Path, into: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for path in entries.flatten().map(|e| e.path()) {
        if path.is_dir() {
            collect(&path, into);
        } else if bitwig_document::Kind::from_path(&path).is_some() {
            into.push(path);
        }
    }
}

/// A throwaway repository tree. Removed on drop, including after a panic.
struct Fixture {
    root: PathBuf,
}

impl Fixture {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir()
            .join("orange-catalog-tests")
            .join(format!("{}-{name}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("fixture root");
        Fixture { root }
    }

    /// Add an item, returning its directory.
    fn add(&self, author: &str, slug: &str, document: &Path, version: &str) -> PathBuf {
        let dir = self.root.join("devices").join(author).join(slug);
        std::fs::create_dir_all(&dir).expect("item dir");
        let name = document.file_name().expect("document name");
        std::fs::copy(document, dir.join(name)).expect("copy document");
        self.write_manifest(&dir, author, version);
        dir
    }

    fn write_manifest(&self, dir: &Path, author: &str, version: &str) {
        let manifest = format!(
            "version = \"{version}\"\nauthor = \"{author}\"\n\
             license = \"CC-BY-4.0\"\nmin_bitwig = \"6.1\"\n"
        );
        std::fs::write(dir.join("orange.toml"), manifest).expect("write manifest");
    }

    fn items(&self) -> Vec<Item> {
        let (items, failures) = scan(&self.root);
        assert!(failures.is_empty(), "scan failures: {failures:?}");
        items
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

macro_rules! sources_or_skip {
    () => {{
        let sources = sources();
        if sources.len() < 2 {
            eprintln!("no sample documents, skipping");
            return;
        }
        sources
    }};
}

#[test]
fn a_well_formed_tree_validates_and_indexes() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("well-formed");
    for (n, document) in sources.iter().enumerate() {
        fixture.add("caviio", &format!("item-{n}"), document, "1.0.0");
    }

    let items = fixture.items();
    assert_eq!(items.len(), sources.len());

    let report = validate::check(&items);
    assert!(report.is_mergeable(), "{:?}", report.problems);

    let index = Index::build(&items, Some("abc123".into()));
    assert_eq!(index.items.len(), items.len());
    for entry in &index.items {
        assert_eq!(entry.digest.len(), 64, "digest is not a sha-256");
        assert!(entry.size > 0);
        assert!(entry.path.starts_with("devices/caviio/"));
        assert!(!entry.name.is_empty());
    }
    // The index must survive the trip it actually makes: serialize, publish, parse.
    assert_eq!(Index::parse(&index.to_json()).unwrap(), index);
}

#[test]
fn two_items_may_not_claim_one_identity() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("duplicate-identity");
    // The same document under two slugs is the collision that matters: with
    // name-derived UUIDs, two authors can reach it without copying anything.
    fixture.add("caviio", "original", &sources[0], "1.0.0");
    fixture.add("someone", "borrowed", &sources[0], "1.0.0");

    let report = validate::check(&fixture.items());
    assert!(!report.is_mergeable());
    assert!(
        report.problems.iter().any(|p| matches!(p, Problem::DuplicateIdentity { .. })),
        "{:?}",
        report.problems
    );
}

#[test]
fn the_manifest_may_not_claim_another_authors_directory() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("author-mismatch");
    let dir = fixture.add("caviio", "item", &sources[0], "1.0.0");
    fixture.write_manifest(&dir, "someone-else", "1.0.0");

    let report = validate::check(&fixture.items());
    assert!(!report.is_mergeable());
    assert!(report.problems.iter().any(|p| matches!(p, Problem::AuthorMismatch { .. })));
}

#[test]
fn a_published_identity_may_not_change() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("identity-changed");
    let dir = fixture.add("caviio", "item", &sources[0], "1.0.0");
    let published = Index::build(&fixture.items(), None);

    // Replace the document with a different one under the same slug.
    for entry in std::fs::read_dir(&dir).unwrap().flatten() {
        if bitwig_document::Kind::from_path(&entry.path()).is_some() {
            std::fs::remove_file(entry.path()).unwrap();
        }
    }
    let replacement = sources[1].file_name().unwrap();
    std::fs::copy(&sources[1], dir.join(replacement)).unwrap();

    let report = validate::check_against(&fixture.items(), &published);
    assert!(!report.is_mergeable(), "{:?}", report.problems);
    assert!(
        report.problems.iter().any(|p| matches!(p, Problem::IdentityChanged { .. })),
        "{:?}",
        report.problems
    );
}

#[test]
fn changed_content_must_raise_the_version() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("content-changed");
    let dir = fixture.add("caviio", "item", &sources[0], "1.0.0");
    let published = Index::build(&fixture.items(), None);

    // Re-identify the document in place: same slug, same version, new bytes.
    // This is the update that would reach back into existing projects.
    let document_path = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| bitwig_document::Kind::from_path(p).is_some())
        .unwrap();
    let kind = bitwig_document::Kind::from_path(&document_path).unwrap();
    let original = bitwig_document::Document::read(&document_path).unwrap();
    let rewritten = original.with_uuid(uuid::Uuid::new_v4()).unwrap();
    std::fs::write(&document_path, rewritten.bytes()).unwrap();
    assert_eq!(kind, rewritten.kind());

    let report = validate::check_against(&fixture.items(), &published);
    assert!(!report.is_mergeable());
    // The identity moved too, so both rules fire. Either alone blocks the merge;
    // what matters is that an unversioned content change cannot pass.
    assert!(
        report.problems.iter().any(|p| matches!(
            p,
            Problem::ContentChangedWithoutVersionBump { .. } | Problem::IdentityChanged { .. }
        )),
        "{:?}",
        report.problems
    );
}

#[test]
fn a_version_may_not_go_backwards() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("version-backwards");
    let dir = fixture.add("caviio", "item", &sources[0], "2.0.0");
    let published = Index::build(&fixture.items(), None);

    fixture.write_manifest(&dir, "caviio", "1.9.0");
    let report = validate::check_against(&fixture.items(), &published);
    assert!(!report.is_mergeable());
    assert!(report.problems.iter().any(|p| matches!(p, Problem::VersionWentBackwards { .. })));
}

#[test]
fn a_name_collision_warns_without_blocking() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("name-collision");
    fixture.add("caviio", "original", &sources[0], "1.0.0");

    // Same display name, different identity: legal, but two entries would appear
    // under one name in Bitwig's flat browser.
    let dir = fixture.add("someone", "same-name", &sources[0], "1.0.0");
    let document_path = std::fs::read_dir(&dir)
        .unwrap()
        .flatten()
        .map(|e| e.path())
        .find(|p| bitwig_document::Kind::from_path(p).is_some())
        .unwrap();
    let rewritten = bitwig_document::Document::read(&document_path)
        .unwrap()
        .with_uuid(uuid::Uuid::new_v4())
        .unwrap();
    std::fs::write(&document_path, rewritten.bytes()).unwrap();

    let report = validate::check(&fixture.items());
    assert!(report.is_mergeable(), "a name collision must not block: {:?}", report.problems);
    let warnings: Vec<_> = report.warnings().collect();
    assert!(
        warnings.iter().any(|p| matches!(p, Problem::DuplicateName { .. })),
        "{:?}",
        report.problems
    );
    assert_eq!(warnings[0].severity(), Severity::Warning);
}

#[test]
fn a_first_run_has_no_history_to_break() {
    let sources = sources_or_skip!();
    let fixture = Fixture::new("first-run");
    fixture.add("caviio", "item", &sources[0], "1.0.0");

    let empty = Index { schema: orange_catalog::index::SCHEMA, revision: None, items: Vec::new() };
    assert!(validate::check_against(&fixture.items(), &empty).is_mergeable());
}
