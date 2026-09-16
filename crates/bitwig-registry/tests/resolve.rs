//! Resolution against a real installation.
//!
//! These are the tests that matter: they prove the anchors find Bitwig's
//! internals without any name being written down. They skip, rather than fail,
//! when no Bitwig is installed.

use bitwig_classfile::Jar;
use bitwig_document::Kind;
use bitwig_install::Installation;
use bitwig_registry::{Binding, GuardState, guard, read_entries};

fn jar() -> Option<Jar> {
    let install = Installation::discover().ok()?;
    let jar = Jar::open(&install.jar()).ok()?;
    Some(jar)
}

macro_rules! jar_or_skip {
    () => {
        match jar() {
            Some(jar) => jar,
            None => {
                eprintln!("no Bitwig Studio installed, skipping");
                return;
            }
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
    assert!(build.version >= bitwig_registry::BitwigVersion::parse("6.0").unwrap());
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
