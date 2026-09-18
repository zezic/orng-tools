// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Resolution against a real installation.
//!
//! These are the tests that matter: they prove the anchors find Bitwig's
//! internals without any name being written down. They skip, rather than fail,
//! when no Bitwig is installed.

use bitwig_classfile::Jar;
use bitwig_document::{Document, Kind, Serialization};
use bitwig_install::Installation;
use bitwig_registry::{Binding, GuardState, guard, read_entries};

fn jar() -> Option<Jar> {
    let install = Installation::discover().ok()?;
    let jar = Jar::open(&install.jar()).ok()?;
    Some(jar)
}

/// The environment these tests need, and what to do when it is missing.
///
/// Opting **out** rather than in. A test whose subject is absent used to return
/// and report as passed, which is indistinguishable in a summary line from one
/// that ran - so a machine with no Bitwig quietly tested nothing and said 75
/// passing. Now the absence fails the run unless the caller states that it
/// expects it, which is a thing only continuous integration has any business
/// saying.
const SKIP: &str = "ORNG_SKIP_BITWIG_TESTS";

macro_rules! jar_or_skip {
    () => {
        match jar() {
            Some(jar) => jar,
            None if std::env::var_os(SKIP).is_some() => {
                eprintln!("no Bitwig Studio installed, skipping");
                return;
            }
            None => panic!("no Bitwig Studio installed; set {SKIP}=1 to skip these tests"),
        }
    };
}

#[test]
fn resolves_every_anchor() {
    let jar = jar_or_skip!();
    let binding = Binding::resolve(&jar).expect("resolution failed");

    assert!(binding.registry.class.contains('/'), "{:?}", binding.registry.class);
    assert!(binding.registry.register_descriptor.starts_with("(Ljava/util/UUID;"));
    assert_eq!(binding.registry.kind_fields.len(), 3, "{:?}", binding.registry.kind_fields);
    for kind in Kind::ALL {
        assert!(binding.registry.kind_fields.contains_key(&kind), "missing {kind:?}");
    }
    assert_eq!(binding.entitlement.grant_fields.len(), 3);
    assert!(!binding.entitlement.row_class.is_empty());
    assert!(binding.guard_entry.ends_with(".class"));

    // Not asserted for their values, which change per build. Printed so that a
    // failure on a future release shows what moved.
    println!("registry      {}", binding.registry.class);
    println!("register      {}{}", binding.registry.register_method, binding.registry.register_descriptor);
    println!("kind enum     {}", binding.registry.kind_enum);
    println!("kind fields   {:?}", binding.registry.kind_fields);
    println!("entitlement   {}", binding.entitlement.class);
    println!("grant maps    {:?}", binding.entitlement.grant_fields);
    println!("row class     {}", binding.entitlement.row_class);
    println!("guard         {}", binding.guard_entry);

    // The install bar and About screen name the build, so it has to resolve on
    // a normal installation even though a patch does not depend on it.
    let build = binding.build.expect("build string not found");
    assert!(build.version >= bitwig_document::BitwigVersion::parse("6.0").unwrap());
    assert_eq!(build.revision.len(), 40);
    println!("build         {build}");
}

#[test]
fn reads_the_factory_registry() {
    let jar = jar_or_skip!();
    let binding = Binding::resolve(&jar).expect("resolution failed");
    let bytes = jar.entry(&binding.registry.entry).unwrap();
    let entries = read_entries(&bytes, &binding.registry).expect("reading entries failed");

    // Bitwig 6.1 ships 152 devices, 43 modulators and 233 modules. Exact counts
    // are version-specific, so only the shape is asserted.
    assert!(entries.len() > 300, "only {} entries", entries.len());
    for kind in Kind::ALL {
        let count = entries.iter().filter(|e| e.kind == kind).count();
        assert!(count > 10, "only {count} entries of {kind:?}");
        println!("{kind:?}: {count}");
    }

    // Every entry must name a path that resolves under its kind's directory.
    for entry in &entries {
        assert!(
            entry.library_path.starts_with(entry.kind.library_subdir()),
            "{} is filed under {:?}",
            entry.library_path,
            entry.kind
        );
        assert!(entry.library_path.ends_with(entry.kind.extension()));
        assert!(!entry.name.is_empty());
    }

    // UUIDs identify an entry, so a duplicate would mean the reader mispaired
    // the two phases of the class initialiser.
    let unique: std::collections::BTreeSet<_> = entries.iter().map(|e| e.uuid).collect();
    assert_eq!(unique.len(), entries.len(), "duplicate UUIDs read");
}

#[test]
fn reports_the_tamper_guard_as_armed_on_a_stock_install() {
    let jar = jar_or_skip!();
    let binding = Binding::resolve(&jar).expect("resolution failed");
    let bytes = jar.entry(&binding.guard_entry).unwrap();

    let state = guard::inspect(&bytes).expect("guard site not found");
    println!("guard: {state:?}");
    assert_ne!(state, GuardState::Unknown);

    if state == GuardState::Armed {
        let disarmed = guard::disarm(&bytes).expect("disarm failed");
        assert_eq!(guard::inspect(&disarmed).unwrap(), GuardState::Disarmed);
        // Disarming twice must be a no-op rather than a second edit.
        assert_eq!(guard::disarm(&disarmed).unwrap(), disarmed);
    }
}

#[test]
fn widening_the_register_method_leaves_a_loadable_class() {
    let jar = jar_or_skip!();
    let binding = Binding::resolve(&jar).expect("resolution failed");
    let original = jar.entry(&binding.registry.entry).unwrap();

    let widened = bitwig_classfile::pool::make_method_public(
        &original,
        &binding.registry.register_method,
        &binding.registry.register_descriptor,
    )
    .expect("could not widen the register method");

    // Access flags are two bytes and only their low byte moves, so the edit is
    // one or two bytes and the file length is unchanged.
    assert_eq!(widened.len(), original.len());
    let changed = widened.iter().zip(&original).filter(|(a, b)| a != b).count();
    assert!((1..=2).contains(&changed), "{changed} bytes changed, expected the access flags only");

    // What actually matters: the method a direct call would target is public.
    const ACC_PUBLIC: u16 = 0x0001;
    const ACC_PRIVATE: u16 = 0x0002;
    let class = bitwig_classfile::edit::parse(&widened).unwrap();
    let index = (0..class.methods.len())
        .find(|i| {
            bitwig_classfile::edit::method_signature(&class, *i)
                == Some((binding.registry.register_method.as_str(), binding.registry.register_descriptor.as_str()))
        })
        .expect("register method vanished");
    let access = bitwig_classfile::edit::method_access(&class, index).unwrap();
    assert_eq!(access & ACC_PUBLIC, ACC_PUBLIC);
    assert_eq!(access & ACC_PRIVATE, 0);

    // The registry must still read back whole.
    let entries = read_entries(&widened, &binding.registry).expect("reading entries failed");
    assert_eq!(entries.len(), read_entries(&original, &binding.registry).unwrap().len());
}

/// The section key comes out of the installation, and reads its own content.
///
/// The one test that matters for the key: not that some 128 bytes were found,
/// but that what was found decrypts a document Bitwig itself wrote and that the
/// result parses into an identity. A wrong key yields high-entropy noise, which
/// the scanner rejects, so this cannot pass on the wrong answer.
///
/// Nothing here prints the key. What is asserted is what it *does*.
#[test]
fn the_section_key_comes_out_of_the_installation_and_opens_its_documents() {
    let install = match Installation::discover() {
        Ok(install) => install,
        Err(_) if std::env::var_os(SKIP).is_some() => {
            eprintln!("no Bitwig Studio installed, skipping");
            return;
        }
        Err(e) => panic!("no Bitwig Studio installed ({e}); set {SKIP}=1 to skip these tests"),
    };

    let devices = install.library_dir().join("devices");
    let mut factory: Vec<_> = std::fs::read_dir(&devices)
        .unwrap_or_else(|e| panic!("{}: {e}", devices.display()))
        .filter_map(|entry| entry.ok().map(|e| e.path()))
        .filter(|path| Kind::from_path(path).is_some())
        .collect();
    factory.sort();
    assert!(!factory.is_empty(), "{} holds no factory documents", devices.display());

    let key = bitwig_registry::section_key(&install.jar(), &factory[0])
        .expect("the section key could not be read out of this build");

    // Every one of them, not just the one the search was verified against: a
    // key that opened a single document and nothing else would be a fluke.
    for path in factory.iter().take(40) {
        let document = Document::read_with_key(path, &key)
            .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert!(!document.identity().name.is_empty(), "{} has no name", path.display());
        assert_eq!(
            document.serialization(),
            Serialization::EncryptedBinary,
            "{} is not the encrypted form, so it proves nothing here",
            path.display()
        );
    }

    // And the refusal without it, which is what every layer above relies on.
    let raw = std::fs::read(&factory[0]).expect("read back");
    let kind = Kind::from_path(&factory[0]).expect("a document");
    assert!(
        matches!(Document::parse(kind, raw), Err(bitwig_document::Error::Encrypted)),
        "an encrypted document must refuse to open without a key"
    );
}

