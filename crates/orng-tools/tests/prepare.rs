// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Preparation, end to end, against a throwaway copy of a real installation.
//!
//! The copy is what makes these worth running: the archive is the real one, the
//! JVM that verifies it is the real one, and the only thing that is not real is
//! the directory being written to. They skip, rather than fail, when no Bitwig
//! is installed.

use std::path::{Path, PathBuf};

use bitwig_classfile::edit::{self, Instr};
use bitwig_classfile::{Jar, JarEdits};
use bitwig_document::Kind;
use bitwig_registry::{Binding, guard};
use orng_tools::{
    Backup, Destination, Document, Error, GuardState, Helper, Installation, LibraryPath, Manifest,
    OrngHome, Placement, Plan, Provenance, Registration, Step, Strategy, UserLibrary, placement,
    prepare,
};

/// A copy of the installed Bitwig that a test may destroy.
///
/// Only `bitwig.jar` and the description bundles are copied, because they are
/// the only things this project writes. Everything else is linked: `libs.jar`
/// and the JVM bundles are 300 MB between them, and are read, never modified.
struct Mirror {
    _temp: tempfile::TempDir,
    /// The mirror, its throwaway user library and its throwaway home, as one
    /// value - so a test cannot hand preparation one installation and placement
    /// another.
    to: Destination,
}

impl Mirror {
    fn build() -> Option<Self> {
        let real = Installation::discover().ok()?;
        if !real.jar().is_file() {
            return None;
        }

        let temp = tempfile::tempdir().expect("no temp directory");
        let root = temp.path().join("Bitwig Studio.app");
        let java = root.join("Contents/Java");
        let resources = root.join("Contents/Resources");
        let localization = resources.join("localization");
        std::fs::create_dir_all(&java).unwrap();
        std::fs::create_dir_all(&localization).unwrap();

        std::fs::copy(real.jar(), java.join("bitwig.jar")).expect("could not copy the archive");
        link(&real.libs_jar(), &java.join("libs.jar"));

        for kind in Kind::ALL {
            std::fs::create_dir_all(resources.join("Library").join(kind.library_subdir())).unwrap();
            let bundle = kind.descriptions_bundle();
            let source = real.localization_dir().join(bundle);
            if source.is_file() {
                std::fs::copy(source, localization.join(bundle)).unwrap();
            }
        }

        // The JVM is found by probing, so the mirror has to put one where the
        // probe will look - and where that is differs by platform: a bundle
        // under Contents/PlugIns on macOS, `jre` on Windows and Linux. Naming
        // the macOS ones left the mirror without a JVM anywhere else, which is
        // why none of these tests could run outside it.
        //
        // So mirror whatever the real installation actually uses, at the same
        // place relative to its root. That keeps the probe honest without this
        // test knowing any layout at all.
        let java = real.bundled_java().expect("the installation has no bundled JVM");
        let home = java.parent().and_then(Path::parent).expect("java lives in <home>/bin");
        let relative = home.strip_prefix(real.root()).expect("the JVM is inside the installation");
        let at = root.join(relative);
        std::fs::create_dir_all(at.parent().expect("the JVM is not the root")).unwrap();
        link_dir(home, &at);

        let install = Installation::at(&root).expect("the mirror is not a valid installation");
        Some(Mirror {
            to: Destination {
                install,
                library: UserLibrary::at(&temp.path().join("Library")),
                home: OrngHome::at(temp.path()),
                placement: Strategy::Link,
            },
            _temp: temp,
        })
    }

    fn plan(&self) -> Plan {
        self.plan_with(Strategy::Link)
    }

    fn plan_with(&self, placement: Strategy) -> Plan {
        Plan::compute(&self.destination(placement)).expect("planning failed")
    }

    fn destination(&self, placement: Strategy) -> Destination {
        Destination { placement, ..self.to.clone() }
    }

    /// The installation's folder for `kind`, as it is on disk.
    fn library_folder(&self, kind: Kind) -> PathBuf {
        self.to.install
            .library_dir()
            .join(kind.library_subdir())
            .join(kind.user_folder())
    }

    /// Apply, collecting the steps that were reported.
    fn apply(&self, plan: Plan) -> Vec<Step> {
        let mut seen = Vec::new();
        plan.apply(|step| seen.push(step)).expect("preparation failed");
        seen
    }

    fn binding(&self) -> Binding {
        Binding::resolve(&Jar::open(&self.to.install.jar()).unwrap()).unwrap()
    }

    fn guard_state(&self) -> GuardState {
        let jar = Jar::open(&self.to.install.jar()).unwrap();
        guard::inspect(&jar.entry(&self.binding().guard_entry).unwrap()).unwrap()
    }

    /// Put an entry list where the injected class will look for it.
    fn write_entries(&self, entries: &str) {
        let path = self.to.home.entries();
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, entries).unwrap();
    }

    fn archive(&self) -> Vec<u8> {
        std::fs::read(self.to.install.jar()).unwrap()
    }

    /// Replace the mirror's archive with one holding `edits`.
    fn rewrite_archive(&self, edits: &JarEdits) {
        let staged = self.to.install.jar().with_extension("rewritten");
        Jar::open(&self.to.install.jar()).unwrap().rewrite(&staged, edits).unwrap();
        std::fs::rename(&staged, self.to.install.jar()).unwrap();
    }
}

fn link(target: &Path, at: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, at).unwrap();
    #[cfg(windows)]
    std::os::windows::fs::symlink_file(target, at).unwrap();
}

/// Link a directory, the way the crate itself does.
///
/// A junction on Windows rather than a symbolic link, for the same reason
/// `placement` uses one: a symbolic link needs a privilege an ordinary account
/// does not hold, and a test that only passes when run elevated is a test that
/// says nothing about how the product behaves.
fn link_dir(target: &Path, at: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, at).unwrap();
    #[cfg(windows)]
    junction::create(target, at).unwrap();
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

macro_rules! mirror_or_skip {
    () => {
        match Mirror::build() {
            Some(mirror) => mirror,
            None if std::env::var_os(SKIP).is_some() => {
                eprintln!("no Bitwig Studio installed, skipping");
                return;
            }
            None => panic!("no Bitwig Studio installed; set {SKIP}=1 to skip these tests"),
        }
    };
}

#[test]
fn prepares_a_real_installation_and_the_result_loads() {
    let mirror = mirror_or_skip!();
    let plan = mirror.plan();

    assert_eq!(plan.guard(), GuardState::Armed, "a stock archive should be armed");
    assert!(!plan.backup_exists(), "nothing has been prepared yet");
    let backup_dir = plan.backup_directory().to_path_buf();
    assert!(backup_dir.starts_with(mirror.to.home.backups()));

    // Every step, in order. Verification happens inside this call under Bitwig's
    // own JVM, so reaching Activate means the patched archive loaded and ran the
    // initialiser of every class it edited.
    let steps = mirror.apply(plan);
    assert_eq!(steps, Step::ALL, "steps ran out of order or were skipped");

    assert_eq!(mirror.guard_state(), GuardState::Disarmed);
    assert!(backup_dir.join("bitwig.jar").is_file(), "no backup was written");
    assert!(!staging_path(&mirror).exists(), "the staging archive was left behind");

    // All three kinds are linked whether or not anything of that kind is
    // registered, so that entry updates never write into the installation.
    for kind in Kind::ALL {
        let link = mirror.library_folder(kind);
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink(), "{kind:?} not linked");
    }
}

#[test]
fn preparing_twice_produces_the_same_archive() {
    let mirror = mirror_or_skip!();
    mirror.apply(mirror.plan());
    let once = mirror.archive();

    // The second run must patch the backup, not its own output: patching the
    // patched archive would stack a second copy of every edit.
    let plan = mirror.plan();
    assert!(plan.backup_exists(), "the first run should have captured the build");
    mirror.apply(plan);

    assert_eq!(once, mirror.archive(), "the second run diverged from the first");
}

#[test]
fn restoring_puts_the_original_back() {
    let mirror = mirror_or_skip!();
    let original = mirror.archive();
    mirror.apply(mirror.plan());
    assert_ne!(original, mirror.archive());

    let backups = Backup::list(&mirror.to.home).unwrap();
    let [backup] = backups.as_slice() else { panic!("expected one backup, got {backups:?}") };
    backup.restore(&mirror.to.install).unwrap();

    assert_eq!(original, mirror.archive());
    assert_eq!(mirror.guard_state(), GuardState::Armed);
}

#[test]
fn a_modified_installation_with_no_backup_is_refused() {
    let mirror = mirror_or_skip!();
    mirror.apply(mirror.plan());

    // Losing the backup is the case that matters: without it there is no
    // pristine archive to patch, and patching the patched one would stack.
    std::fs::remove_dir_all(mirror.to.home.backups()).unwrap();
    let refused = Plan::compute(&mirror.destination(Strategy::Link));
    assert!(matches!(refused, Err(Error::AlreadyModified(_))), "{refused:?}");
}

#[test]
fn a_patch_that_does_not_verify_never_reaches_the_installation() {
    let mirror = mirror_or_skip!();

    // Break the registry class in the one way only a JVM can catch: it still
    // parses, still resolves, and still assembles, but its registration method
    // now pops an empty stack. Nothing before the verify step can tell.
    let binding = mirror.binding();
    let mut edits = JarEdits::new();
    edits.replace(
        binding.registry.entry.clone(),
        unverifiable(&Jar::open(&mirror.to.install.jar()).unwrap().entry(&binding.registry.entry).unwrap(), &binding),
    );
    mirror.rewrite_archive(&edits);
    let broken = mirror.archive();

    let failure = mirror.plan().apply(|_| {});
    assert!(matches!(failure, Err(Error::VerificationFailed { .. })), "{failure:?}");
    assert_eq!(broken, mirror.archive(), "the installation was modified anyway");
    assert!(!staging_path(&mirror).exists(), "the rejected archive was left behind");
}

/// The test this whole module exists for.
///
/// Verification loads the patched registry class and forces its initialiser,
/// which now ends in a call to the injected class. The injected class catches
/// everything it might throw, so the proof is not that the JVM exited zero --
/// it would have done that with a call resolving to nothing. The proof is that
/// it printed no complaint, which verification treats as a failure.
///
/// So a clean run with entries waiting means: the helper was found, its
/// retargeted names resolved to this build's registry, category enum and grant
/// row, and Bitwig's own registration method accepted every row.
///
/// What this one cannot show is that the entry list was read at all -- an
/// installation that found no file would also print nothing. That is what
/// `a_broken_entry_list_costs_the_entries_and_not_the_launch` is for: it can
/// only report if it read the file. The two together cover the chain.
#[test]
fn the_injected_class_registers_the_entry_list() {
    let mirror = mirror_or_skip!();

    let mut manifest = Manifest::default();
    for (name, kind) in
        [("ORNG TEST DEVICE", Kind::Device), ("ORNG TEST SHAPER", Kind::Modulator)]
    {
        manifest.insert(Registration {
            uuid: uuid::Uuid::new_v4(),
            name: name.into(),
            library_path: LibraryPath::for_document(kind, &format!("{name}.{}", kind.extension()))
                .unwrap(),
            description: "written by a test".into(),
            keywords: vec!["orng".into(), "test".into()],
            digest: None,
            // One of each source, so the row the class has to read past the
            // columns it knows is covered too.
            provenance: match kind {
                Kind::Device => Provenance::Local,
                _ => Provenance::Catalog {
                    version: "1.2.0".parse().unwrap(),
                    reviewed_in: orng_tools::Revision::new(
                        "3f9a1c2e8b4d7a61c05f2d93ab7e14c8f6021b5d",
                    )
                    .ok(),
                },
            },
        });
    }
    mirror.write_entries(&manifest.to_tsv());

    mirror.apply(mirror.plan());
}

/// What the application asks on opening, and again after preparing.
///
/// Read back out of the archive rather than remembered, because the answer has
/// to survive the app being closed, the installation being replaced by a Bitwig
/// update, and a preparation that failed half way.
#[test]
fn an_installation_reports_what_has_been_done_to_it() {
    let mirror = mirror_or_skip!();

    let before = prepare::inspect(&mirror.to.install).expect("could not read the mirror");
    assert!(before.is_stock(), "a fresh mirror is not stock: {before:?}");
    assert!(!before.is_prepared());
    assert_eq!(before.helper, Helper::Absent);
    assert_eq!(before.guard, GuardState::Armed);

    mirror.apply(mirror.plan());

    let after = prepare::inspect(&mirror.to.install).expect("could not read the prepared mirror");
    assert!(after.is_prepared(), "not prepared after preparing: {after:?}");
    assert!(!after.is_stock());
    assert_eq!(after.helper, Helper::Present);
    assert_eq!(after.guard, GuardState::Disarmed);
}

/// The state the two facts exist to describe, and the reason `is_prepared` is
/// not just "our class is in there".
///
/// A preparation that stopped between patching and activating leaves the class
/// in an archive whose guard is still armed. That is worse than not being
/// prepared: Bitwig degrades audio in every project rather than merely ignoring
/// the entry list, so nothing may report it as ready.
#[test]
fn our_class_with_the_guard_still_armed_is_not_prepared() {
    let mirror = mirror_or_skip!();

    // The content does not matter. What is being asked is whether the entry is
    // there, and preparation is what would also have disarmed the guard.
    let mut edits = JarEdits::new();
    edits.add("OrngRegistry.class", b"not a class, and does not need to be".to_vec());
    mirror.rewrite_archive(&edits);

    let half = prepare::inspect(&mirror.to.install).expect("could not read the mirror");
    assert_eq!(half.helper, Helper::Present);
    assert_eq!(half.guard, GuardState::Armed);
    assert!(!half.is_prepared(), "an armed guard with our class in it is not prepared");
    assert!(!half.is_stock(), "an archive with our class in it is not stock");
}

/// The other half: the injected class must never stop Bitwig starting.
///
/// It runs inside a class initialiser Bitwig cannot do without, so an entry list
/// it cannot make sense of has to cost the entries and nothing else. A file
/// hand-edited into nonsense is the realistic way that happens.
///
/// Preparation refuses, which is the right answer at prepare time -- a list this
/// installation cannot apply is worth being told about. What matters here is
/// *how* it refuses: with the message the class prints when it catches
/// something, and not with an initialiser error, which is what an exception
/// escaping into Bitwig would look like. That the message is there at all also
/// proves the class read the entry list this preparation was computed against.
#[test]
fn a_broken_entry_list_costs_the_entries_and_not_the_launch() {
    let mirror = mirror_or_skip!();
    let broken = "#orng-registry 2\nnot-a-uuid\tDEVICE\tA\tdevices/A.bwdevice\t\t\t\tlocal\n";
    mirror.write_entries(broken);

    let refused = mirror.plan().apply(|_| {});
    let Err(Error::VerificationFailed { report }) = refused else {
        panic!("expected the broken list to be reported, got {refused:?}");
    };
    assert!(report.contains("could not apply the entry list"), "{report}");
    assert!(
        !report.contains("ExceptionInInitializerError"),
        "the failure escaped into Bitwig's own initialiser:\n{report}"
    );
}

/// Copying documents into the installation only means anything if the library
/// folders are left alone.
///
/// A registered path resolves inside the installation's `Library`. Link the
/// folders and that path resolves straight back out into the user library, so a
/// document "copied into the installation" lands in the same file the linked
/// strategy would have used -- the setting reads as a choice and makes none.
#[test]
fn copying_documents_keeps_them_inside_the_installation() {
    let mirror = mirror_or_skip!();

    let plan = mirror.plan_with(Strategy::Copy);
    let promised: Vec<Step> = plan.steps().collect();
    assert_eq!(
        promised,
        [Step::Backup, Step::Patch, Step::Verify, Step::Activate],
        "copying should not link the library folders"
    );
    // What it said it would run is what it ran, so a step list drawn from the
    // plan cannot describe a different transaction from the one that happens.
    assert_eq!(mirror.apply(plan), promised);

    for kind in Kind::ALL {
        let folder = mirror.library_folder(kind);
        let linked = folder
            .symlink_metadata()
            .is_ok_and(|meta| meta.file_type().is_symlink());
        assert!(!linked, "{kind:?} was linked under Copy");
    }

    // The document has to be reachable at its registered path and be a real file
    // inside the installation, not a link out of it.
    let document = a_real_device();
    let registration = Registration {
        uuid: document.identity().uuid,
        name: "ORNG COPIED".into(),
        library_path: LibraryPath::for_document(Kind::Device, "ORNG COPIED.bwdevice").unwrap(),
        description: "written by a test".into(),
        keywords: Vec::new(),
        digest: None,
        provenance: Provenance::Local,
    };
    let written =
        placement::place(&mirror.destination(Strategy::Copy), &registration, &document).unwrap();

    assert!(written.starts_with(mirror.to.install.root()), "{written:?} is outside the installation");
    assert!(
        !written.starts_with(mirror.to.library.root()),
        "{written:?} landed in the user library, so Copy did nothing"
    );
    assert!(matches!(
        placement::inspect(&mirror.to.install, &registration),
        Placement::Copied(_)
    ));
}

/// A real device document, out of the installation these tests already need.
///
/// Placement reads whatever is at its target and compares identities before it
/// writes, so the handful of bytes a test would otherwise invent is refused -
/// and rightly, because it cannot be proved to be an older copy of anything.
fn a_real_device() -> Document {
    let install = Installation::discover().expect("these tests already need an installation");
    let mut found: Vec<PathBuf> = std::fs::read_dir(install.library_dir().join("devices"))
        .expect("the installation has no device library")
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| Kind::from_path(path).is_some())
        .collect();
    // Sorted, so the document is the same one on every run and a failure names
    // a file somebody can go and open.
    found.sort();
    let path = found.first().expect("the installation ships no devices");
    // Factory documents are encrypted, and the key belongs to the installation
    // rather than to this repository, so it is read back out of the same build
    // this document came from.
    let key = bitwig_registry::section_key(&install.jar(), path)
        .expect("the section key could not be read out of this build");
    Document::read_with_key(path, &key).unwrap_or_else(|e| panic!("{}: {e}", path.display()))
}

/// Where `prepare` stages a replacement archive. Named here rather than exposed,
/// because what matters to a caller is that nothing is left behind.
fn staging_path(mirror: &Mirror) -> PathBuf {
    let mut name = mirror.to.install.jar().into_os_string();
    name.push(".orng-part");
    PathBuf::from(name)
}

/// Make the registration method's first instruction `pop`, which underflows the
/// operand stack at method entry and fails verification at link time.
fn unverifiable(class: &[u8], binding: &Binding) -> Vec<u8> {
    edit::edit_class(class, |class| {
        let found = edit::edit_method_code(class, &binding.registry.register_method, |code| {
            code.0[0].1 = Instr::Pop;
        });
        assert!(found, "the registration method vanished");
        Ok(())
    })
    .expect("could not build a broken class")
}
