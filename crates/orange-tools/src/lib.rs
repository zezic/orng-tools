// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3, as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

//! Registering custom Bitwig content with an installation.
//!
//! The durable state is a list of [`Registration`]s that the app owns. An
//! installation is prepared once to read that list at startup; after that,
//! adding or removing content only rewrites the list. A Bitwig update resets the
//! installation but never the list, so re-applying costs one preparation rather
//! than one operation per registered item.
//!
//! Layering, bottom up: `bitwig-install` finds things, `bitwig-document` reads
//! identities, `bitwig-classfile` edits bytecode, `bitwig-registry` locates
//! Bitwig's internals, and this crate composes them.

pub mod backup;
pub mod descriptions;
mod fs;
pub mod home;
mod inject;
pub mod manifest;
pub mod placement;
pub mod prepare;

use std::path::PathBuf;

use uuid::Uuid;

pub use backup::Backup;
pub use bitwig_document::{BitwigVersion, Document, Identity, Kind, Serialization};
pub use bitwig_install::{AppData, Installation, RunState, UserLibrary, running_state};
pub use bitwig_registry::{Anchor, Binding, BuildId, Entry, GuardState};
pub use home::OrangeHome;
pub use placement::{Placement, Strategy};
pub use manifest::Manifest;
pub use prepare::{Plan, Step};

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("io error on {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error(transparent)]
    Install(#[from] bitwig_install::Error),
    #[error(transparent)]
    Document(#[from] bitwig_document::Error),
    #[error(transparent)]
    Classfile(#[from] bitwig_classfile::Error),
    #[error(transparent)]
    Registry(#[from] bitwig_registry::Error),
    #[error("{field} may not contain a tab or newline: {value:?}")]
    UnrepresentableField { field: &'static str, value: String },
    #[error("malformed entry list at line {line}: {reason}")]
    MalformedManifest { line: usize, reason: &'static str },
    #[error("this build does not state its version, so a backup could not be named for it")]
    UnrecognisedBuild,
    #[error("{0} has already been modified and there is no backup to prepare from")]
    AlreadyModified(PathBuf),
    #[error("quit Bitwig Studio before preparing the installation (running: {})", .0.join(", "))]
    BitwigRunning(Vec<String>),
    #[error("this installation ships no Java runtime, so the patch cannot be verified")]
    NoBundledJava,
    #[error("the patched archive did not load; the installation was left alone:\n{report}")]
    VerificationFailed { report: String },
    #[error("the backup at {0} is incomplete")]
    BackupIncomplete(PathBuf),
    #[error("the injected class names {placeholder} {found} times, expected once")]
    HelperPlaceholder { placeholder: &'static str, found: usize },
    #[error("{class} has no {method} to add the call to")]
    NoSuchMethod { class: String, method: String },
    #[error("the method to add the call to does not end in a plain return")]
    NotStraightLine,
}

pub type Result<T> = std::result::Result<T, Error>;

/// A library path relative to the installation's `Library` directory, in the
/// form Bitwig's registry stores: `devices/My Devices/NAME.bwdevice`.
///
/// Validated on construction so that a traversal or a separator that would break
/// the entry list can never reach the registry.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LibraryPath(String);

impl LibraryPath {
    /// The conventional location for registered content: the kind's directory
    /// inside the installation, under the folder linked to the user library.
    pub fn for_document(kind: Kind, file_name: &str) -> Result<Self> {
        Self::new(format!("{}/{}/{}", kind.library_subdir(), kind.user_folder(), file_name))
    }

    pub fn new(path: impl Into<String>) -> Result<Self> {
        let path = path.into();
        let rejected = path.is_empty()
            || path.starts_with('/')
            || path.contains("..")
            || path.contains('\\')
            || path.contains(['\t', '\n', '\r']);
        if rejected {
            return Err(Error::UnrepresentableField { field: "library path", value: path });
        }
        Ok(LibraryPath(path))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Resolve against an installation's `Library` directory.
    pub fn resolve(&self, install: &Installation) -> PathBuf {
        self.0.split('/').fold(install.library_dir(), |acc, part| acc.join(part))
    }

    /// The directory component, as the registry sees it.
    pub fn parent(&self) -> &str {
        self.0.rsplit_once('/').map(|(head, _)| head).unwrap_or("")
    }

    pub fn file_name(&self) -> &str {
        self.0.rsplit_once('/').map(|(_, tail)| tail).unwrap_or(&self.0)
    }
}

/// One piece of custom content, as this app records it.
///
/// This is the durable unit. It outlives any particular installation, which is
/// what lets the same identity be restored after a Bitwig update.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Registration {
    pub uuid: Uuid,
    pub kind: Kind,
    pub name: String,
    pub library_path: LibraryPath,
    /// Shown under the entry in Bitwig's browser.
    pub description: String,
    /// Words that find the entry when typed into the browser.
    pub keywords: Vec<String>,
}

impl Registration {
    /// Derive a registration from a document, with defaults good enough that a
    /// user who edits nothing still gets a searchable entry.
    pub fn from_document(document: &Document, file_name: &str) -> Result<Self> {
        let identity = document.identity();
        let kind = document.kind();
        Ok(Registration {
            uuid: identity.uuid,
            kind,
            name: validated("name", &identity.name)?,
            library_path: LibraryPath::for_document(kind, file_name)?,
            description: identity
                .description
                .clone()
                .unwrap_or_else(|| format!("Custom {}", kind.label().to_lowercase())),
            keywords: identity.suggested_keywords(),
        })
    }
}

/// Reject values that cannot survive a round trip through the entry list.
fn validated(field: &'static str, value: &str) -> Result<String> {
    if value.is_empty() || value.contains(['\t', '\n', '\r']) {
        return Err(Error::UnrepresentableField { field, value: value.to_owned() });
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn library_paths_reject_escapes() {
        assert!(LibraryPath::new("devices/My Devices/A.bwdevice").is_ok());
        assert!(LibraryPath::new("/etc/passwd").is_err());
        assert!(LibraryPath::new("devices/../../x").is_err());
        assert!(LibraryPath::new("devices/a\tb").is_err());
    }

    #[test]
    fn library_path_for_a_document_matches_bitwigs_shape() {
        let path = LibraryPath::for_document(Kind::Modulator, "SHAPER.bwmodulator").unwrap();
        assert_eq!(path.as_str(), "modulators/My Modulators/SHAPER.bwmodulator");
        assert_eq!(path.parent(), "modulators/My Modulators");
        assert_eq!(path.file_name(), "SHAPER.bwmodulator");
    }

}
