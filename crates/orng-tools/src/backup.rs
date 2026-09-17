// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Pristine copies of everything preparation and entry updates overwrite.
//!
//! A backup is keyed by the Bitwig build it was taken from, and is written once:
//! the first preparation of a build captures the installation untouched, and
//! every later one finds that copy already there and leaves it alone. Two things
//! follow, and both matter more than the disk space:
//!
//! - Preparation patches the backup rather than the installed file, so running
//!   it twice produces the same archive rather than a doubly-patched one.
//! - Restore always has an unmodified original to put back, no matter how many
//!   times the installation has been prepared since.
//!
//! What is captured is what this project writes: `bitwig.jar`, and the
//! description bundles that entry updates rewrite. Not the installation.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use bitwig_document::Kind;

use crate::{BuildId, Error, Installation, OrngHome, Result, fs};

/// A pristine copy of one build, on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    dir: PathBuf,
}

impl Backup {
    /// Where the backup of `build` belongs, present or not.
    pub fn location(home: &OrngHome, build: &BuildId) -> Self {
        Backup { dir: home.backups().join(directory_name(build)) }
    }

    /// Every backup that has been taken, most recent first.
    ///
    /// Ordered by when it was written rather than by version, because the reason
    /// to look at this list is to restore what was last replaced.
    pub fn list(home: &OrngHome) -> Result<Vec<Backup>> {
        let mut found: Vec<(SystemTime, Backup)> = fs::entries(&home.backups())?
            .into_iter()
            .map(|dir| Backup { dir })
            // A directory with no archive in it is not a backup: an interrupted
            // copy, or something the user left here.
            .filter_map(|backup| Some((backup.taken_at().ok()?, backup)))
            .collect();
        found.sort_by(|(a, _), (b, _)| b.cmp(a));
        Ok(found.into_iter().map(|(_, backup)| backup).collect())
    }

    pub fn directory(&self) -> &Path {
        &self.dir
    }

    /// The unmodified `bitwig.jar`.
    pub fn jar(&self) -> PathBuf {
        self.dir.join("bitwig.jar")
    }

    /// Whether this build has already been captured. A backup exists only once
    /// it is complete, so a half-written one cannot be mistaken for a pristine
    /// copy: the archive is written to a temporary name and renamed last.
    pub fn exists(&self) -> bool {
        self.jar().is_file()
    }

    /// When the copy was taken, for the backup indicator and the restore list.
    pub fn taken_at(&self) -> Result<SystemTime> {
        fs::modified(&self.jar())
    }

    /// Copy the installation's originals here, unless they are already here.
    ///
    /// Taking a backup twice must not replace the first one: by the second call
    /// the installation is already prepared, and copying it would destroy the
    /// only pristine archive there is. Use [`Backup::exists`] to know in advance
    /// which of the two this call will be.
    pub fn take(&self, install: &Installation) -> Result<()> {
        if self.exists() {
            return Ok(());
        }
        fs::create_dir_all(&self.dir)?;

        for (source, name) in bundles(install) {
            // A fresh installation has every bundle, but a locale-stripped one
            // may not, and a bundle that is not there is not lost by restoring.
            if source.is_file() {
                fs::copy(&source, &self.dir.join(name))?;
            }
        }

        // Last, and through a temporary name: the jar's presence is what marks
        // the backup complete, so it must not appear before it is whole.
        let staging = self.dir.join("bitwig.jar.part");
        fs::copy(&install.jar(), &staging)?;
        fs::rename(&staging, &self.jar())
    }

    /// Put the installation back the way this backup found it.
    ///
    /// The archive goes back atomically, so a restore that fails part-way leaves
    /// the installation running the archive it had rather than a truncated one.
    pub fn restore(&self, install: &Installation) -> Result<()> {
        if !self.exists() {
            return Err(Error::BackupIncomplete(self.dir.clone()));
        }

        for (target, name) in bundles(install) {
            let saved = self.dir.join(name);
            if saved.is_file() {
                fs::copy(&saved, &target)?;
            }
        }

        let staging = staging_path(&install.jar());
        fs::copy(&self.jar(), &staging)?;
        fs::rename(&staging, &install.jar())
    }
}

/// Named for the build, so that an installation is matched with its own
/// original and never with the one before the last Bitwig update.
fn directory_name(build: &BuildId) -> String {
    format!("{}-{}", build.version, build.short_revision())
}

/// The description bundles, paired with the name they are kept under. Flattened
/// because the backup is one directory and the bundles have distinct names.
fn bundles(install: &Installation) -> impl Iterator<Item = (PathBuf, &'static str)> {
    Kind::ALL.into_iter().map(|kind| {
        let name = kind.descriptions_bundle();
        (install.localization_dir().join(name), name)
    })
}

/// Where a replacement archive is staged: beside the file it replaces, so that
/// activating it is a rename within one filesystem and therefore atomic.
pub(crate) fn staging_path(target: &Path) -> PathBuf {
    let mut name = target.as_os_str().to_owned();
    name.push(".orng-part");
    PathBuf::from(name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use bitwig_document::BitwigVersion;

    fn build(version: &str, revision: &str) -> BuildId {
        BuildId {
            version: BitwigVersion::parse(version).unwrap(),
            revision: revision.repeat(40 / revision.len()),
        }
    }

    #[test]
    fn a_backup_is_named_for_the_build_it_came_from() {
        let home = OrngHome::at(Path::new("/tmp/orng-test"));
        let backup = Backup::location(&home, &build("6.1", "ab"));
        assert!(backup.directory().ends_with("6.1-abababab"), "{:?}", backup.directory());
    }

    #[test]
    fn two_builds_never_share_a_backup() {
        let home = OrngHome::at(Path::new("/tmp/orng-test"));
        let one = Backup::location(&home, &build("6.1", "ab"));
        let two = Backup::location(&home, &build("6.2", "ab"));
        assert_ne!(one.directory(), two.directory());
    }

    #[test]
    fn staging_sits_beside_the_file_it_replaces() {
        let target = Path::new("/Applications/Bitwig Studio.app/Contents/Java/bitwig.jar");
        let staging = staging_path(target);
        assert_eq!(staging.parent(), target.parent());
        assert_ne!(staging, target);
    }

    #[test]
    fn listing_a_home_that_has_never_been_used_is_empty_not_an_error() {
        let home = OrngHome::at(Path::new("/nonexistent/orng"));
        assert!(Backup::list(&home).unwrap().is_empty());
    }
}
