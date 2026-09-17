// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Filesystem operations that name the file they failed on.
//!
//! Every error this crate reports about a file says which file, because the
//! failures worth reporting are the ones a user has to go and look at: a
//! permission denied inside an application bundle, a backup on a full disk. Done
//! at each call site that is a closure per call and a chance to name the wrong
//! path; done here the path can only be the one the operation used.

use std::path::{Path, PathBuf};
use std::time::SystemTime;

use crate::{Error, Result};

/// Read a file that may legitimately not exist yet.
///
/// A missing entry list and a missing description bundle both mean "nothing
/// registered", which is a normal state and not a failure. Separating that from
/// a real read error here keeps the callers from matching on error kinds.
pub(crate) fn read_to_string_if_exists(path: &Path) -> Result<Option<String>> {
    match std::fs::read_to_string(path) {
        Ok(text) => Ok(Some(text)),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(source) => Err(error(path, source)),
    }
}

pub(crate) fn write(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    std::fs::write(path, contents).map_err(|source| error(path, source))
}

/// Write a file, creating the directories leading to it.
pub(crate) fn write_new(path: &Path, contents: impl AsRef<[u8]>) -> Result<()> {
    if let Some(parent) = path.parent() {
        create_dir_all(parent)?;
    }
    write(path, contents)
}

pub(crate) fn copy(from: &Path, to: &Path) -> Result<()> {
    std::fs::copy(from, to).map(drop).map_err(|source| error(from, source))
}

pub(crate) fn rename(from: &Path, to: &Path) -> Result<()> {
    std::fs::rename(from, to).map_err(|source| error(to, source))
}

pub(crate) fn create_dir_all(path: &Path) -> Result<()> {
    std::fs::create_dir_all(path).map_err(|source| error(path, source))
}

pub(crate) fn remove_file(path: &Path) -> Result<()> {
    std::fs::remove_file(path).map_err(|source| error(path, source))
}

/// The paths directly inside a directory, or none if there is no such directory.
pub(crate) fn entries(path: &Path) -> Result<Vec<PathBuf>> {
    let dir = match std::fs::read_dir(path) {
        Ok(dir) => dir,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(source) => return Err(error(path, source)),
    };
    dir.map(|entry| entry.map(|e| e.path()).map_err(|source| error(path, source)))
        .collect()
}

/// When a file was last written.
pub(crate) fn modified(path: &Path) -> Result<SystemTime> {
    std::fs::metadata(path)
        .and_then(|metadata| metadata.modified())
        .map_err(|source| error(path, source))
}

/// For operations with no wrapper here, so that they still name their path.
pub(crate) fn error(path: &Path, source: std::io::Error) -> Error {
    Error::Io { path: path.display().to_string(), source }
}
