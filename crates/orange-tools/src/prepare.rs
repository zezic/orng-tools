// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Preparing an installation: the one operation that modifies it.
//!
//! The edit itself is fixed and does not vary with what is registered, so this
//! runs once per Bitwig build rather than once per registered item. What varies
//! is where the edit lands, which [`bitwig_registry::Binding`] resolves out of
//! the archive in front of us.
//!
//! Two properties are worth stating, because the rest of the module exists to
//! hold them:
//!
//! - **Nothing in the installation changes until the patched archive verifies.**
//!   It is written beside the original under a temporary name and moved into
//!   place by a single rename, so a failure anywhere before that leaves an
//!   installation that was never touched, not one that must be repaired.
//! - **Preparing twice is the same as preparing once.** The source archive is
//!   the backup taken the first time, never the file currently installed, so a
//!   second run cannot patch its own output.
//!
//! The order of the steps is not the caller's to choose. [`Plan::compute`] reads
//! and decides, [`Plan::apply`] writes, and everything between the two is inside
//! one call that consumes the plan.

use std::path::{Path, PathBuf};
use std::process::Command;

use bitwig_classfile::{Jar, JarEdits, pool};
use bitwig_registry::guard;

use crate::backup::staging_path;
use crate::{
    Backup, Binding, BuildId, Error, GuardState, Installation, OrangeHome, Result, RunState,
    UserLibrary, fs, placement, running_state,
};

/// The class that drives verification, as text rather than a compiled artifact.
/// See `java/OrangeVerify.java` for what it does and how to regenerate this.
const VERIFIER_SOURCE: &str = include_str!("../java/OrangeVerify.j");
const VERIFIER_CLASS: &str = "OrangeVerify";

/// The ordered work of a preparation, as the progress display names it.
///
/// Reported as each step begins. A step that is reported has started, not
/// finished; the call returning is what says the last one finished.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// Copy the pristine archive and description bundles aside.
    Backup,
    /// Write the patched archive beside the original.
    Patch,
    /// Load the edited classes under Bitwig's own JVM.
    Verify,
    /// Move the patched archive into place.
    Activate,
    /// Link the installation's library folders to the user library.
    Link,
}

impl Step {
    /// In the order [`Plan::apply`] runs them, so that a step list can be drawn
    /// before the first one starts. The wording of each is the application's,
    /// not this crate's.
    pub const ALL: [Step; 5] =
        [Step::Backup, Step::Patch, Step::Verify, Step::Activate, Step::Link];
}

/// What preparation will do, decided without writing anything.
///
/// Computing one reads the archive and resolves every anchor, so a build that
/// cannot be patched is refused here rather than half way through a write.
#[derive(Debug)]
pub struct Plan {
    install: Installation,
    library: UserLibrary,
    backup: Backup,
    build: BuildId,
    binding: Binding,
    /// The archive the patch is built from: the backup once one exists, so that
    /// repeated preparations all start from the same unmodified bytes.
    source: Jar,
    edits: JarEdits,
    guard: GuardState,
}

impl Plan {
    /// Resolve an installation and work out the edit it needs.
    pub fn compute(
        install: &Installation,
        library: &UserLibrary,
        home: &OrangeHome,
    ) -> Result<Self> {
        let installed = Jar::open(&install.jar())?;
        let binding = Binding::resolve(&installed)?;

        // A backup that cannot be named for its build is a backup that cannot be
        // found again, and restoring the wrong original is worse than refusing.
        let build = binding.build.clone().ok_or(Error::UnrecognisedBuild)?;
        let backup = Backup::location(home, &build);

        // Patch the pristine copy, not whatever is installed now. Resolution is
        // repeated against it because it is a different file, even though it is
        // the same build.
        let (source, binding) = if backup.exists() {
            let jar = Jar::open(&backup.jar())?;
            let binding = Binding::resolve(&jar)?;
            (jar, binding)
        } else {
            (installed, binding)
        };

        let guard_class = source.entry(&binding.guard_entry)?;
        let guard = guard::inspect(&guard_class)?;
        // An installation already modified with no pristine copy to work from:
        // patching it again would stack a second edit on the first.
        if guard == GuardState::Disarmed && !backup.exists() {
            return Err(Error::AlreadyModified(install.jar()));
        }

        let mut edits = JarEdits::new();
        edits.replace(binding.guard_entry.clone(), guard::disarm(&guard_class)?);
        edits.replace(
            binding.registry.entry.clone(),
            pool::make_method_public(
                &source.entry(&binding.registry.entry)?,
                &binding.registry.register_method,
                &binding.registry.register_descriptor,
            )?,
        );

        Ok(Plan {
            install: install.clone(),
            library: library.clone(),
            backup,
            build,
            binding,
            source,
            edits,
            guard,
        })
    }

    /// Which Bitwig this prepares. Named in the confirmation and on the backup.
    pub fn build(&self) -> &BuildId {
        &self.build
    }

    /// The tamper guard as the pristine archive has it. Diagnostic: disarming it
    /// is part of preparation either way.
    pub fn guard(&self) -> GuardState {
        self.guard
    }

    /// Where the pristine copy will be kept, for the confirmation to state.
    pub fn backup_directory(&self) -> &Path {
        self.backup.directory()
    }

    /// Whether this build has already been captured, so the confirmation can say
    /// that the backup is being reused rather than written.
    pub fn backup_exists(&self) -> bool {
        self.backup.exists()
    }

    /// Carry it out, reporting each step as it begins.
    ///
    /// Consumes the plan: a plan describes one archive at one moment, and
    /// applying it twice would mean the second run decided nothing.
    pub fn apply(self, mut progress: impl FnMut(Step)) -> Result<()> {
        // Bitwig holds the archive open and would keep running the old one.
        if let RunState::Running(processes) = running_state(&self.install) {
            return Err(Error::BitwigRunning(processes));
        }

        progress(Step::Backup);
        self.backup.take(&self.install)?;

        progress(Step::Patch);
        let target = self.install.jar();
        let staging = Staged::write(&self.source, &target, &self.edits)?;

        progress(Step::Verify);
        self.verify(staging.path())?;

        progress(Step::Activate);
        staging.activate(&target)?;

        progress(Step::Link);
        placement::ensure_all_links(&self.install, &self.library)?;
        Ok(())
    }

    /// Load every edited class under Bitwig's own JVM.
    ///
    /// Loading links a class, and linking is what runs the verifier, so a patch
    /// that produces an unloadable class fails here rather than at the next
    /// Bitwig launch. Initialisation is forced too, which on the registry class
    /// means its several hundred registrations actually run.
    fn verify(&self, archive: &Path) -> Result<()> {
        let java = self.install.bundled_java().ok_or(Error::NoBundledJava)?;
        let classes = [
            binary_name(&self.binding.registry.class),
            binary_name(&self.binding.entitlement.class),
            binary_name(self.binding.guard_entry.trim_end_matches(".class")),
        ];

        let driver = write_verifier()?;
        let classpath = join_classpath(&[driver.path(), archive, &self.install.libs_jar()]);

        let output = Command::new(&java)
            .arg("-cp")
            .arg(&classpath)
            .arg(VERIFIER_CLASS)
            .args(classes)
            .output()
            .map_err(|source| fs::error(&java, source))?;

        if output.status.success() {
            return Ok(());
        }
        let mut report = String::from_utf8_lossy(&output.stderr).into_owned();
        report.push_str(&String::from_utf8_lossy(&output.stdout));
        Err(Error::VerificationFailed { report: report.trim().to_owned() })
    }
}

/// A patched archive written beside the one it replaces, not yet in place.
///
/// Dropping it without activating removes it, so a failed verification leaves
/// no half-installed archive behind for the next run to trip over.
struct Staged {
    path: PathBuf,
}

impl Staged {
    fn write(source: &Jar, target: &Path, edits: &JarEdits) -> Result<Self> {
        let staged = Staged { path: staging_path(target) };
        source.rewrite(&staged.path, edits)?;
        Ok(staged)
    }

    fn path(&self) -> &Path {
        &self.path
    }

    /// Replace the installed archive in one rename.
    ///
    /// On success the staging path no longer exists and dropping must not try to
    /// remove what was just installed; on failure the drop is exactly what is
    /// wanted, so the two cases differ only in whether the drop is suppressed.
    fn activate(self, target: &Path) -> Result<()> {
        match std::fs::rename(&self.path, target) {
            Ok(()) => {
                let _installed = std::mem::ManuallyDrop::new(self);
                Ok(())
            }
            Err(source) => Err(fs::error(target, source)),
        }
    }
}

impl Drop for Staged {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.path);
    }
}

/// Assemble the verifier into a directory of its own, for the classpath and
/// nowhere near the installation. Removed when the returned handle is dropped.
fn write_verifier() -> Result<tempfile::TempDir> {
    let dir = tempfile::tempdir()
        .map_err(|source| fs::error(&std::env::temp_dir(), source))?;
    let class = dir.path().join(format!("{VERIFIER_CLASS}.class"));
    fs::write(&class, bitwig_classfile::edit::assemble(VERIFIER_SOURCE)?)?;
    Ok(dir)
}

/// `com/bitwig/x/Y` as `Class.forName` wants it.
fn binary_name(internal: &str) -> String {
    internal.replace('/', ".")
}

fn join_classpath(parts: &[&Path]) -> std::ffi::OsString {
    let separator = if cfg!(windows) { ";" } else { ":" };
    let mut joined = std::ffi::OsString::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            joined.push(separator);
        }
        joined.push(part);
    }
    joined
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_verifier_class_assembles() {
        let bytes = bitwig_classfile::edit::assemble(VERIFIER_SOURCE).unwrap();
        assert_eq!(&bytes[..4], b"\xca\xfe\xba\xbe");
    }

    #[test]
    fn internal_names_become_loadable_ones() {
        assert_eq!(binary_name("com/bitwig/flt/packaging/core/nj2"), "com.bitwig.flt.packaging.core.nj2");
        assert_eq!(binary_name("ELu"), "ELu");
    }
}
