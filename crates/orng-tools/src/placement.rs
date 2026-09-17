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

use uuid::Uuid;

use crate::{
    Destination, Document, Error, Installation, Kind, Registration, Result, UserLibrary, fs,
};

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
///
/// **Refuses to write over a document that is not this one.** Under the linking
/// strategy the destination is the folder Bitwig's own "Save device..." writes
/// into, so a file already sitting there is as likely to be the user's work as
/// an older copy of what is being registered. Only a matching identity licenses
/// a replacement; anything else is refused by name, because the library path is
/// derived from a file name and two unrelated documents can easily share one.
pub fn place(
    to: &Destination,
    registration: &Registration,
    document: &Document,
) -> Result<PathBuf> {
    let destination = target(to, registration);
    if let Some(occupied) = would_replace(to, registration)? {
        return Err(Error::PathOccupied { path: occupied.display().to_string() });
    }
    fs::write_new(&destination, document.bytes())?;
    Ok(destination)
}

/// Where this registration's document goes.
pub fn target(to: &Destination, registration: &Registration) -> PathBuf {
    match to.placement {
        Strategy::Link => to
            .library
            .folder(registration.kind.user_folder())
            .join(registration.library_path.file_name()),
        Strategy::Copy => registration.library_path.resolve(&to.install),
    }
}

/// Whether placing this registration would write over a document that is not it,
/// and if so, which file.
///
/// The check [`place`] makes, offered on its own so the interface can say so
/// before the user presses anything rather than after. One implementation and
/// two callers, because a second one would be free to drift.
pub fn would_replace(to: &Destination, registration: &Registration) -> Result<Option<PathBuf>> {
    let destination = target(to, registration);
    Ok(match occupant_of(&destination)? {
        Occupant::Vacant => None,
        // The same identity is this content, at whatever revision was there
        // before. Replacing it is the point of re-applying an edited document.
        Occupant::Document(uuid) if uuid == registration.uuid => None,
        Occupant::Document(_) | Occupant::Foreign => Some(destination),
    })
}

/// What is already where a document is about to be written.
enum Occupant {
    /// Nothing is there, so nothing can be lost.
    Vacant,
    /// A document, and the identity it carries.
    Document(Uuid),
    /// Something that is not a readable document. It has no identity to compare,
    /// and proving a file is an older copy of this content is the only thing
    /// that licenses replacing it - so an unreadable one is never replaced.
    Foreign,
}

fn occupant_of(path: &Path) -> Result<Occupant> {
    let Some(raw) = fs::read_if_exists(path)? else {
        return Ok(Occupant::Vacant);
    };
    let kind = Kind::from_path(path).expect("a placement target ends in a document extension");
    Ok(match Document::parse(kind, raw) {
        Ok(document) => Occupant::Document(document.identity().uuid),
        Err(_) => Occupant::Foreign,
    })
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
    use crate::testing::{self, document};
    use crate::{Error, LibraryPath, OrngHome, Provenance};
    use uuid::Uuid;

    /// Synthetic on purpose, so this runs on every platform. Linking is the one
    /// part of preparation that differs between them, and the difference is
    /// invisible to whoever is not running the platform that has it.
    fn fake_install(root: &Path) -> Installation {
        testing::install(root)
    }

    /// A machine to place documents on, under `placement`.
    fn machine(temp: &Path, placement: Strategy) -> Destination {
        Destination {
            install: fake_install(&temp.join("install")),
            library: UserLibrary::at(&temp.join("library")),
            home: OrngHome::at(temp),
            placement,
        }
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

    fn registration_for(document: &Document, file_name: &str) -> Registration {
        Registration::from_document(document, file_name).unwrap()
    }

    /// The linking strategy writes into the folder Bitwig's own "Save device..."
    /// writes into, so what is already at the target is as likely to be the
    /// user's work as an older copy of what is being registered. Two unrelated
    /// documents sharing a file name is not a contrived case: the library path
    /// is derived from the file name.
    #[test]
    fn a_document_belonging_to_something_else_is_never_written_over() {
        let temp = tempfile::tempdir().unwrap();
        let to = machine(temp.path(), Strategy::Link);

        let theirs = document(Kind::Device, Uuid::new_v4(), "THEIRS");
        let mine = document(Kind::Device, Uuid::new_v4(), "MINE");
        let occupied = registration_for(&theirs, "SHARED.bwdevice");
        let colliding = registration_for(&mine, "SHARED.bwdevice");

        let at = place(&to, &occupied, &theirs).unwrap();
        // Said before the write is attempted as well as by refusing it, because
        // the interface has to be able to state the collision on the row rather
        // than as a failure after the press.
        assert_eq!(would_replace(&to, &colliding).unwrap().as_deref(), Some(at.as_path()));
        let refused = place(&to, &colliding, &mine);
        assert!(matches!(refused, Err(Error::PathOccupied { .. })), "{refused:?}");
        assert_eq!(std::fs::read(&at).unwrap(), theirs.bytes(), "the other document was replaced");
    }

    /// Re-applying an edited document is the case placing has to stay open to,
    /// and it is exactly the one where the target already holds something: the
    /// previous revision of the same identity.
    #[test]
    fn the_same_identity_is_replaced_rather_than_refused() {
        let temp = tempfile::tempdir().unwrap();
        let to = machine(temp.path(), Strategy::Link);

        let uuid = Uuid::new_v4();
        let first = document(Kind::Device, uuid, "DISPERSER");
        let edited = document(Kind::Device, uuid, "DISPERSER MK2");
        let registration = registration_for(&first, "DISPERSER.bwdevice");

        place(&to, &registration, &first).unwrap();
        assert_eq!(would_replace(&to, &registration).unwrap(), None);
        let at = place(&to, &registration, &edited).expect("the same identity was refused");
        assert_eq!(std::fs::read(&at).unwrap(), edited.bytes());
    }

    /// A file that cannot be read as a document has no identity to compare, and
    /// being able to prove the target is an older copy of this content is the
    /// only thing that licenses replacing it.
    #[test]
    fn a_file_that_is_not_a_document_is_not_assumed_to_be_ours() {
        let temp = tempfile::tempdir().unwrap();
        let to = machine(temp.path(), Strategy::Link);

        let mine = document(Kind::Device, Uuid::new_v4(), "MINE");
        let registration = registration_for(&mine, "MINE.bwdevice");
        let occupied = target(&to, &registration);
        fs::write_new(&occupied, b"not a document at all").unwrap();

        let refused = place(&to, &registration, &mine);
        assert!(matches!(refused, Err(Error::PathOccupied { .. })), "{refused:?}");
        assert_eq!(std::fs::read(&occupied).unwrap(), b"not a document at all");
    }

    /// Copying writes inside the installation and linking writes into the user
    /// library. Getting that backwards is silent: both produce a file, and only
    /// the wrong one disappears with the next Bitwig update.
    #[test]
    fn the_strategy_decides_which_of_the_two_libraries_is_written() {
        let temp = tempfile::tempdir().unwrap();
        let mine = document(Kind::Device, Uuid::new_v4(), "MINE");
        let registration = registration_for(&mine, "MINE.bwdevice");

        let linked = machine(&temp.path().join("linked"), Strategy::Link);
        let at = place(&linked, &registration, &mine).unwrap();
        assert!(at.starts_with(linked.library.root()), "{at:?}");

        let copied = machine(&temp.path().join("copied"), Strategy::Copy);
        let at = place(&copied, &registration, &mine).unwrap();
        assert!(at.starts_with(copied.install.root()), "{at:?}");
        assert_eq!(at, registration.library_path.resolve(&copied.install));
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
