// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Getting a document to where its registered library path resolves.
//!
//! A registered path resolves inside the installation's own `Library`. Two ways
//! to satisfy that:
//!
//! - [`Strategy::Link`] keeps documents in the user library and links the
//!   installation's folder to it. Documents then survive a Bitwig update and
//!   only the preparation has to be repeated. This is the default.
//! - [`Strategy::Copy`] writes into the installation. Self-contained, but a
//!   Bitwig update discards the documents along with everything else.

use std::path::{Path, PathBuf};

use crate::{Installation, Kind, Registration, Result, UserLibrary, fs};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Strategy {
    Link,
    Copy,
}

/// Whether a registered document can actually be found.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Placement {
    /// Resolves through a link into the user library.
    Linked(PathBuf),
    /// Resolves to a copy inside the installation.
    Copied(PathBuf),
    /// The registry names it but nothing is there. The `Missing file` state.
    Unresolved(PathBuf),
}

impl Placement {
    pub fn is_resolved(&self) -> bool {
        !matches!(self, Placement::Unresolved(_))
    }

    pub fn path(&self) -> &Path {
        match self {
            Placement::Linked(p) | Placement::Copied(p) | Placement::Unresolved(p) => p,
        }
    }
}

/// Where a registration currently resolves, if anywhere.
pub fn inspect(install: &Installation, registration: &Registration) -> Placement {
    let target = registration.library_path.resolve(install);
    if !target.exists() {
        return Placement::Unresolved(target);
    }
    let folder = install
        .library_dir()
        .join(registration.kind.library_subdir())
        .join(registration.kind.user_folder());
    match folder.symlink_metadata().map(|m| m.file_type().is_symlink()) {
        Ok(true) => Placement::Linked(target),
        _ => Placement::Copied(target),
    }
}

/// Install a document so that `registration.library_path` resolves.
///
/// Returns where the document ended up. Under [`Strategy::Link`] that is inside
/// the user library; the link itself is installed by [`ensure_link`].
pub fn place(
    install: &Installation,
    library: &UserLibrary,
    registration: &Registration,
    document: &[u8],
    strategy: Strategy,
) -> Result<PathBuf> {
    let destination = match strategy {
        Strategy::Link => library
            .folder(registration.kind.user_folder())
            .join(registration.library_path.file_name()),
        Strategy::Copy => registration.library_path.resolve(install),
    };
    fs::write_new(&destination, document)?;
    Ok(destination)
}

/// Link all three kinds, whether or not anything of that kind is registered.
///
/// Doing this up front is what keeps the cheap path cheap. Linking lazily would
/// mean the first modulator added to a device-only installation had to create a
/// folder inside the installation, which can demand authorisation -- during the
/// operation that is supposed to need neither elevation nor Bitwig closed.
/// Linking all three during preparation makes "entry changes never touch the
/// installation" an invariant rather than a usual case.
///
/// Returns the kinds that were newly linked.
pub fn ensure_all_links(install: &Installation, library: &UserLibrary) -> Result<Vec<Kind>> {
    Kind::ALL
        .into_iter()
        .filter_map(|kind| match ensure_link(install, library, kind) {
            Ok(true) => Some(Ok(kind)),
            Ok(false) => None,
            Err(e) => Some(Err(e)),
        })
        .collect()
}

/// Link the installation's folder for `kind` to the user library's.
///
/// Refuses to replace anything that is not already this link, so a real folder
/// of Bitwig's is never destroyed. Returns whether a link was created.
pub fn ensure_link(install: &Installation, library: &UserLibrary, kind: Kind) -> Result<bool> {
    let target = library.folder(kind.user_folder());
    fs::create_dir_all(&target)?;

    let link = install
        .library_dir()
        .join(kind.library_subdir())
        .join(kind.user_folder());

    if let Ok(metadata) = link.symlink_metadata() {
        if !metadata.file_type().is_symlink() {
            return Err(fs::error(
                &link,
                std::io::Error::new(
                    std::io::ErrorKind::AlreadyExists,
                    "a real directory is already there",
                ),
            ));
        }
        // An existing link to the right place is the goal state, not an error.
        if resolves_to(&link, &target) {
            return Ok(false);
        }
        remove_link(&link)?;
    }

    fs::create_dir_all(link.parent().unwrap_or(&link))?;
    link_dir(&target, &link).map_err(|source| fs::error(&link, source))?;
    Ok(true)
}

/// Whether the link already leads to the directory we want.
///
/// Resolved through the filesystem rather than compared as text. A junction
/// stores its target in a form Windows normalises, so what is read back is not
/// what was written, and a string comparison would report every existing link
/// as wrong and replace it on every run.
fn resolves_to(link: &Path, target: &Path) -> bool {
    match (std::fs::canonicalize(link), std::fs::canonicalize(target)) {
        (Ok(from), Ok(to)) => from == to,
        _ => false,
    }
}

/// Point `link` at the directory `target`.
///
/// Windows gets a **directory junction**, not a symbolic link. A symbolic link
/// there needs `SeCreateSymbolicLinkPrivilege`, which an ordinary account does
/// not hold unless Developer Mode is on, so preparation would fail for most of
/// the people it is for. A junction needs no privilege, is resolved by the
/// filesystem below the application, and is indistinguishable to Bitwig.
///
/// What a junction cannot do is point at a network share, or exist on a volume
/// without reparse points such as exFAT. Both fail loudly, and the answer for
/// either is the `Copy` placement strategy, which makes no links at all
/// (decision 6.3).
#[cfg(windows)]
fn link_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    junction::create(target, link)
}

#[cfg(unix)]
fn link_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

/// Remove a link without following it.
///
/// A directory link is a directory on Windows and an ordinary entry on Unix,
/// and the call that removes one refuses the other.
#[cfg(windows)]
fn remove_link(link: &Path) -> Result<()> {
    fs::remove_dir(link)
}

#[cfg(unix)]
fn remove_link(link: &Path) -> Result<()> {
    fs::remove_file(link)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{Error, LibraryPath, Provenance};
    use uuid::Uuid;

    /// An installation only as far as linking cares about one: `ensure_link`
    /// wants somewhere to put a link, and `Installation::at` wants a jar and a
    /// `Library` beside it to agree that a directory is one.
    ///
    /// Synthetic on purpose, so this runs on every platform. Linking is the one
    /// part of preparation that differs between them, and the difference is
    /// invisible to whoever is not running the platform that has it.
    fn fake_install(root: &Path) -> Installation {
        std::fs::create_dir_all(root.join("Contents/Java")).unwrap();
        std::fs::write(root.join("Contents/Java/bitwig.jar"), b"").unwrap();
        std::fs::create_dir_all(root.join("Contents/Resources/Library")).unwrap();
        Installation::at(root).unwrap()
    }

    fn link_of(install: &Installation, kind: Kind) -> PathBuf {
        install.library_dir().join(kind.library_subdir()).join(kind.user_folder())
    }

    #[test]
    fn a_library_folder_is_linked_to_the_user_library() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake_install(&temp.path().join("install"));
        let library = UserLibrary::at(&temp.path().join("library"));
        let link = link_of(&install, Kind::Device);
        let target = library.folder(Kind::Device.user_folder());

        assert!(ensure_link(&install, &library, Kind::Device).unwrap(), "no link was made");
        assert!(link.symlink_metadata().unwrap().file_type().is_symlink());

        // A link and not a copy, which is the whole point: what Bitwig writes
        // into its own folder has to land in the user's library.
        std::fs::write(target.join("probe.bwdevice"), b"x").unwrap();
        assert!(link.join("probe.bwdevice").is_file(), "the link does not lead to the library");
    }

    #[test]
    fn linking_twice_recognises_the_link_it_made() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake_install(&temp.path().join("install"));
        let library = UserLibrary::at(&temp.path().join("library"));

        assert!(ensure_link(&install, &library, Kind::Device).unwrap());
        // Comparing the target as text gets this wrong on Windows, where a
        // junction reads back in a normalised form, and the link would then be
        // torn down and rebuilt on every preparation.
        assert!(!ensure_link(&install, &library, Kind::Device).unwrap(), "relinked needlessly");
    }

    #[test]
    fn a_link_to_the_wrong_place_is_replaced_without_following_it() {
        let temp = tempfile::tempdir().unwrap();
        let install = fake_install(&temp.path().join("install"));
        let library = UserLibrary::at(&temp.path().join("library"));
        let link = link_of(&install, Kind::Device);

        let decoy = temp.path().join("decoy");
        std::fs::create_dir_all(&decoy).unwrap();
        std::fs::write(decoy.join("keep.txt"), b"x").unwrap();

        fs::create_dir_all(link.parent().unwrap()).unwrap();
        link_dir(&decoy, &link).unwrap();

        assert!(ensure_link(&install, &library, Kind::Device).unwrap(), "the link was not moved");
        assert!(resolves_to(&link, &library.folder(Kind::Device.user_folder())));
        // Removing a link must not reach through it. On Windows the call that
        // removes a junction is not the one that removes a file.
        assert!(decoy.join("keep.txt").is_file(), "removing the link emptied its target");
    }

    /// Refusing must be a decision, not a side effect of the delete failing.
    ///
    /// The empty case is the one that matters and the one that looks harmless:
    /// a non-empty directory cannot be removed on either platform, so a missing
    /// guard is masked by the error that follows it. An empty directory removes
    /// cleanly, and Bitwig's own folder would be gone.
    #[test]
    fn a_real_folder_of_bitwigs_is_never_replaced() {
        for populated in [true, false] {
            let temp = tempfile::tempdir().unwrap();
            let install = fake_install(&temp.path().join("install"));
            let library = UserLibrary::at(&temp.path().join("library"));
            let link = link_of(&install, Kind::Device);

            fs::create_dir_all(&link).unwrap();
            if populated {
                std::fs::write(link.join("factory.bwdevice"), b"x").unwrap();
            }

            let refused = ensure_link(&install, &library, Kind::Device);
            let Err(Error::Io { source, .. }) = &refused else {
                panic!("expected a refusal, got {refused:?}");
            };
            assert_eq!(
                source.kind(),
                std::io::ErrorKind::AlreadyExists,
                "refused for the wrong reason: {source}"
            );
            assert!(link.is_dir(), "a real directory was destroyed");
            assert!(!link.symlink_metadata().unwrap().file_type().is_symlink());
        }
    }

    #[test]
    fn a_registration_resolves_under_the_installations_library() {
        let Ok(install) = Installation::discover() else {
            if std::env::var_os("ORNG_SKIP_BITWIG_TESTS").is_none() {
                panic!("no Bitwig Studio installed; set ORNG_SKIP_BITWIG_TESTS=1 to skip");
            }
            eprintln!("no Bitwig Studio installed, skipping");
            return;
        };
        let path = LibraryPath::new("devices/My Devices/X.bwdevice").unwrap();
        let resolved = path.resolve(&install);
        assert!(resolved.starts_with(install.library_dir()));
        assert!(resolved.ends_with("devices/My Devices/X.bwdevice"));

        let registration = Registration {
            uuid: Uuid::new_v4(),
            kind: Kind::Device,
            name: "X".into(),
            library_path: path,
            description: String::new(),
            keywords: Vec::new(),
            provenance: Provenance::Local,
        };
        // Nothing has been placed, so it must report as unresolved rather than
        // claiming a location that does not exist.
        assert!(!inspect(&install, &registration).is_resolved());
    }
}
