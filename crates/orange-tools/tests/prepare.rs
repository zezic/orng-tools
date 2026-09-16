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
use orange_tools::{Backup, Error, GuardState, Installation, OrangeHome, Plan, Step, UserLibrary};

/// A copy of the installed Bitwig that a test may destroy.
///
/// Only `bitwig.jar` and the description bundles are copied, because they are
/// the only things this project writes. Everything else is linked: `libs.jar`
/// and the JVM bundles are 300 MB between them, and are read, never modified.
struct Mirror {
    _temp: tempfile::TempDir,
    install: Installation,
    library: UserLibrary,
    home: OrangeHome,
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

        // The JVM is found by probing the bundle directories, so linking both of
        // them keeps the probe honest instead of hard-coding this machine's.
        let plugins = root.join("Contents/PlugIns");
        std::fs::create_dir_all(&plugins).unwrap();
        for bundle in ["JavaVM-arm64.bundle", "JavaVM-x64.bundle"] {
            let source = real.root().join("Contents/PlugIns").join(bundle);
            if source.is_dir() {
                link(&source, &plugins.join(bundle));
            }
        }

        let install = Installation::at(&root).expect("the mirror is not a valid installation");
        Some(Mirror {
            library: UserLibrary::at(&temp.path().join("Library")),
            home: OrangeHome::at(&temp.path().join(".orange-registry")),
            install,
            _temp: temp,
        })
    }

    fn plan(&self) -> Plan {
        Plan::compute(&self.install, &self.library, &self.home).expect("planning failed")
    }

    /// Apply, collecting the steps that were reported.
    fn apply(&self, plan: Plan) -> Vec<Step> {
        let mut seen = Vec::new();
        plan.apply(|step| seen.push(step)).expect("preparation failed");
        seen
    }

    fn binding(&self) -> Binding {
        Binding::resolve(&Jar::open(&self.install.jar()).unwrap()).unwrap()
    }

    fn guard_state(&self) -> GuardState {
        let jar = Jar::open(&self.install.jar()).unwrap();
        guard::inspect(&jar.entry(&self.binding().guard_entry).unwrap()).unwrap()
    }

    fn archive(&self) -> Vec<u8> {
        std::fs::read(self.install.jar()).unwrap()
    }

    /// Replace the mirror's archive with one holding `edits`.
    fn rewrite_archive(&self, edits: &JarEdits) {
        let staged = self.install.jar().with_extension("rewritten");
        Jar::open(&self.install.jar()).unwrap().rewrite(&staged, edits).unwrap();
        std::fs::rename(&staged, self.install.jar()).unwrap();
    }
}

fn link(target: &Path, at: &Path) {
    #[cfg(unix)]
    std::os::unix::fs::symlink(target, at).unwrap();
    #[cfg(windows)]
    if target.is_dir() {
        std::os::windows::fs::symlink_dir(target, at).unwrap();
    } else {
        std::os::windows::fs::symlink_file(target, at).unwrap();
    }
}

macro_rules! mirror_or_skip {
    () => {
        match Mirror::build() {
            Some(mirror) => mirror,
            None => {
                eprintln!("no Bitwig Studio installed, skipping");
                return;
            }
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
    assert!(backup_dir.starts_with(mirror.home.backups()));

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
        let link = mirror
            .install
            .library_dir()
            .join(kind.library_subdir())
            .join(kind.user_folder());
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

    let backups = Backup::list(&mirror.home).unwrap();
    let [backup] = backups.as_slice() else { panic!("expected one backup, got {backups:?}") };
    backup.restore(&mirror.install).unwrap();

    assert_eq!(original, mirror.archive());
    assert_eq!(mirror.guard_state(), GuardState::Armed);
}

#[test]
fn a_modified_installation_with_no_backup_is_refused() {
    let mirror = mirror_or_skip!();
    mirror.apply(mirror.plan());

    // Losing the backup is the case that matters: without it there is no
    // pristine archive to patch, and patching the patched one would stack.
    std::fs::remove_dir_all(mirror.home.backups()).unwrap();
    let refused = Plan::compute(&mirror.install, &mirror.library, &mirror.home);
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
        unverifiable(&Jar::open(&mirror.install.jar()).unwrap().entry(&binding.registry.entry).unwrap(), &binding),
    );
    mirror.rewrite_archive(&edits);
    let broken = mirror.archive();

    let failure = mirror.plan().apply(|_| {});
    assert!(matches!(failure, Err(Error::VerificationFailed { .. })), "{failure:?}");
    assert_eq!(broken, mirror.archive(), "the installation was modified anyway");
    assert!(!staging_path(&mirror).exists(), "the rejected archive was left behind");
}

/// Where `prepare` stages a replacement archive. Named here rather than exposed,
/// because what matters to a caller is that nothing is left behind.
fn staging_path(mirror: &Mirror) -> PathBuf {
    let mut name = mirror.install.jar().into_os_string();
    name.push(".orange-part");
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
