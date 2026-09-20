// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The directory this project owns, beside the installation it prepares.
//!
//! One root holds everything durable: the entry list a prepared installation
//! reads at startup, and the backups preparation takes before it writes.
//!
//! What parameterises it is the user's home directory, not the root itself, and
//! that is a contract rather than a convenience. The class injected into the
//! installation finds the entry list by joining `user.home` with a fixed name in
//! one line, and it has no way to be told anything else. Keeping the same shape
//! here means the two can only ever disagree if this file is wrong.
//!
//! Nothing here is under the installation. A Bitwig update replaces that
//! directory wholesale, and it would take the backup of the thing it replaced
//! with it.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Root name, relative to the user's home directory. Shared with the injected
/// class, so it is a wire format and not a preference.
const ROOT: &str = ".orng";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrngHome {
    home: PathBuf,
}

impl OrngHome {
    /// The real user's home, whether or not anything has been written under it.
    pub fn discover() -> Result<Self> {
        let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home = std::env::var_os(var).ok_or(Error::Install(bitwig_install::Error::NoHome))?;
        Ok(Self { home: PathBuf::from(home) })
    }

    /// A directory standing in for a home. Tests use it to keep off the real one.
    pub fn at(home: &Path) -> Self {
        Self { home: home.to_path_buf() }
    }

    pub fn root(&self) -> PathBuf {
        self.home.join(ROOT)
    }

    /// The entry list. Read by this app and by the prepared installation.
    pub fn entries(&self) -> PathBuf {
        self.root().join("entries.tsv")
    }

    /// Where preparation keeps a pristine copy of what it replaces, one
    /// directory per Bitwig build.
    pub fn backups(&self) -> PathBuf {
        self.root().join("backups")
    }

    /// What the user has chosen, as against what the machine says.
    ///
    /// Named here because the directory's layout is this crate's, and read
    /// nowhere else: the injected class finds the entry list and knows nothing
    /// about a preference, so unlike [`OrngHome::entries`] this is not a wire
    /// format and its contents are the application's own.
    pub fn settings(&self) -> PathBuf {
        self.root().join("settings.toml")
    }

    /// The last catalog index that verified, so a window opened with no network
    /// has something to browse rather than nothing.
    ///
    /// A directory of two files rather than one, because what is kept is the
    /// bytes as they arrived and the signature over them: reading it back is the
    /// same check the download went through, so a cache edited on disk is
    /// refused exactly as a tampered download is. An index this crate
    /// re-serialised would be one nothing could prove.
    pub fn catalog(&self) -> PathBuf {
        self.root().join("catalog")
    }

    pub fn catalog_index(&self) -> PathBuf {
        self.catalog().join("index.json")
    }

    pub fn catalog_signature(&self) -> PathBuf {
        self.catalog().join("index.json.sig")
    }

    /// What a JVM has to be told `user.home` is for the injected class to find
    /// this entry list. On a real installation that is already true of the JVM
    /// Bitwig starts, which is why nothing sets it outside verification.
    ///
    /// Public because it is also the one value a process that cannot discover
    /// this home has to be handed: an elevated child may be running as another
    /// account entirely, and `USERPROFILE` there is that account's.
    pub fn user_home(&self) -> &Path {
        &self.home
    }
}
