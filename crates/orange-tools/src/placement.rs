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
        if std::fs::read_link(&link).is_ok_and(|current| current == target) {
            return Ok(false);
        }
        fs::remove_file(&link)?;
    }

    fs::create_dir_all(link.parent().unwrap_or(&link))?;
    symlink_dir(&target, &link).map_err(|source| fs::error(&link, source))?;
    Ok(true)
}

/// Windows cannot create a symlink without elevation, but an unprivileged user
/// can create a directory junction, which resolves identically here.
#[cfg(windows)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}

#[cfg(unix)]
fn symlink_dir(target: &Path, link: &Path) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LibraryPath;
    use uuid::Uuid;

    #[test]
    fn a_registration_resolves_under_the_installations_library() {
        let Ok(install) = Installation::discover() else {
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
        };
        // Nothing has been placed, so it must report as unresolved rather than
        // claiming a location that does not exist.
        assert!(!inspect(&install, &registration).is_resolved());
    }
}
