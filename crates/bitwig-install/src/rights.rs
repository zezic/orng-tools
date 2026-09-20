// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Whether this process may write inside an installation.
//!
//! A fact about the machine and about this process together, asked in the same
//! breath as [`crate::running_state`] and for the same reason: both are
//! conditions a modification needs, and both are cheaper to answer once with the
//! installation than to discover part way through a write.
//!
//! **The answer is obtained by writing.** Permissions on the three platforms
//! this runs on are not one model and cannot be read as one - a Windows ACL is
//! not a mode, `Permissions::readonly` reports the DOS read-only attribute and
//! says nothing about an ACL at all, and an effective-access check on Windows
//! means building a token and calling `AuthzAccessCheck`. Creating a file and
//! removing it asks the question the writes themselves will ask, in the terms
//! the platform will answer them in.
//!
//! **This is a proxy and not a promise.** It says a file can be created in each
//! directory a modification writes into. Preparation also renames a file over
//! `bitwig.jar`, which needs delete permission on that entry, and a directory
//! can in principle accept a new file while refusing that. The transaction is
//! still what has to succeed; what this is for is the refusal before the press
//! and the decision to ask for more rights, both of which need an answer before
//! any of it starts.

use std::path::{Path, PathBuf};

use crate::Installation;

/// Whether this process may write inside an installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rights {
    /// Every directory a modification writes into took a file and gave it back.
    Held,
    /// One of them did not. The first that refused, because a caller states one
    /// path to the reader and a list of three would be three ways of saying
    /// that the installation is not this user's to write.
    Withheld {
        directory: PathBuf,
        /// What the platform said, for the report rather than for the decision.
        why: String,
    },
}

impl Rights {
    pub fn are_held(&self) -> bool {
        matches!(self, Rights::Held)
    }
}

/// Ask each directory a modification writes into, by writing in it.
///
/// Every one of them, rather than stopping at the installation root: the root
/// being writable does not make a subdirectory writable, and on macOS the three
/// are not even necessarily siblings - [`Installation`] finds each by looking
/// for it. On Windows they are one inherited ACL under `Program Files` and all
/// three answer the same, which costs two probes nobody needed and buys an
/// answer that is true on the platform where they can differ.
pub fn rights(install: &Installation) -> Rights {
    // The directory holding `bitwig.jar`: preparation writes the patched
    // archive here and renames it into place. Asserted rather than defaulted:
    // an empty path would make the probe land in whatever directory this
    // application was launched from, and answer `Held` for a directory nothing
    // ever looked at.
    let archive = install
        .jar()
        .parent()
        .expect("an installation's archive is inside the installation")
        .to_path_buf();
    // Description bundles, rewritten by every change to the entry list. This is
    // the one that makes the cheap mode reach inside the installation too.
    let localization = install.localization_dir();
    // Library links under `link`, and placed documents under `copy`.
    let library = install.library_dir();

    for directory in [archive, localization, library] {
        if let Err(why) = probe(&directory) {
            return Rights::Withheld { directory, why: why.to_string() };
        }
    }
    Rights::Held
}

/// Create a file in `directory` and take it away again.
///
/// The name carries this process and the moment, so that two copies of this
/// application asking at once cannot collide and report each other's answer.
///
/// The file is removed whether or not anything else here succeeds, and a
/// removal that fails is not reported: what was asked is whether the directory
/// takes a write, and it demonstrably did. Leaving a file behind in that case is
/// worse than saying nothing, but it is not a reason to call the directory
/// unwritable.
fn probe(directory: &Path) -> std::io::Result<()> {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_nanos())
        .unwrap_or_default();
    let path = directory.join(format!(".orng-write-probe-{}-{at}", std::process::id()));

    // `create_new` rather than `create`: this must never truncate something that
    // was already there, however unlikely the name makes that.
    let outcome = std::fs::OpenOptions::new().write(true).create_new(true).open(&path).map(drop);
    if outcome.is_ok() {
        let _ = std::fs::remove_file(&path);
    }
    outcome
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A directory that exists and is the user's own takes a probe.
    #[test]
    fn a_writable_directory_answers_yes() {
        let temp = std::env::temp_dir();
        probe(&temp).expect("the temporary directory refused a file");
    }

    /// And the probe leaves nothing behind, which matters because the directory
    /// it really runs against is somebody's Bitwig installation.
    ///
    /// **In a directory of its own**, not the shared temporary one. Counting
    /// what is in `std::env::temp_dir()` means counting the probes every other
    /// test in this binary is writing and removing there at the same moment,
    /// and the name carries the process id, which does not tell two threads of
    /// one process apart. That raced with
    /// [`a_writable_directory_answers_yes`] and failed about a third of runs.
    #[test]
    fn a_probe_removes_what_it_wrote() {
        let temp = tempfile::tempdir().expect("somewhere to probe");
        let alone = temp.path();
        assert_eq!(count_probes(alone), 0, "a new directory already had a probe in it");
        probe(alone).expect("a new directory refused a file");
        assert_eq!(count_probes(alone), 0, "a probe left its file behind");
    }

    /// A directory that is not there cannot be written to, and says so rather
    /// than being reported as writable by a probe nobody checked.
    #[test]
    fn a_directory_that_is_not_there_answers_no() {
        let missing = std::env::temp_dir().join("orng-no-such-directory-59c1f0");
        assert!(!missing.exists(), "the name this test relies on being free is taken");
        probe(&missing).expect_err("a directory that does not exist accepted a file");
    }

    /// A directory that genuinely refuses is genuinely reported.
    ///
    /// The whole point of the module, and the one case the other tests cannot
    /// reach: every directory on a developer's machine is writable by them.
    /// Unix only, because taking the write bit off is how a directory is made
    /// to refuse here - on Windows it takes an ACL, and the case arrives by
    /// itself under `Program Files`.
    #[cfg(unix)]
    #[test]
    fn a_directory_that_refuses_is_reported_with_its_path() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("somewhere to build an installation");
        let install = fake_installation(root.path());
        assert!(rights(&install).are_held(), "a new directory refused a file");

        // Readable and enterable, but not writable - which is what an
        // installation somebody else owns looks like.
        let shut = install.localization_dir();
        let mut mode = std::fs::metadata(&shut).expect("it was just made").permissions();
        mode.set_mode(0o555);
        std::fs::set_permissions(&shut, mode).expect("could not close the directory");

        let answer = rights(&install);
        // Put it back before asserting, so a failure does not leave a directory
        // the temporary directory cannot remove.
        let mut mode = std::fs::metadata(&shut).expect("it is still there").permissions();
        mode.set_mode(0o755);
        std::fs::set_permissions(&shut, mode).expect("could not open it again");

        match answer {
            Rights::Held => panic!("a directory with no write bit accepted a file"),
            Rights::Withheld { directory, .. } => assert_eq!(
                directory, shut,
                "the refusal named a directory other than the one that refused"
            ),
        }
    }

    /// An installation only as far as [`rights`] cares: the three directories a
    /// modification writes into, and an archive to find them by.
    #[cfg(unix)]
    fn fake_installation(root: &Path) -> crate::Installation {
        std::fs::create_dir_all(root.join("Contents/Java")).expect("the archive's directory");
        std::fs::write(root.join("Contents/Java/bitwig.jar"), b"").expect("an archive");
        std::fs::create_dir_all(root.join("Contents/Resources/Library")).expect("a library");
        std::fs::create_dir_all(root.join("Contents/Resources/localization"))
            .expect("the bundles");
        crate::Installation::at(root).expect("what was just built is not an installation")
    }

    fn count_probes(directory: &Path) -> usize {
        std::fs::read_dir(directory)
            .expect("the temporary directory cannot be listed")
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with(".orng-write-probe-"))
            .count()
    }
}
