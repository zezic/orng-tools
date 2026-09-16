//! The directory this project owns, beside the installation it prepares.
//!
//! One root holds everything durable: the entry list a prepared installation
//! reads at startup, and the backups preparation takes before it writes. The
//! layout is identical on every platform, because the class injected into the
//! installation has to derive the entry list's path from `user.home` in one
//! line and cannot afford a platform switch.
//!
//! Nothing here is under the installation. A Bitwig update replaces that
//! directory wholesale, and it would take the backup of the thing it replaced
//! with it.

use std::path::{Path, PathBuf};

use crate::{Error, Result};

/// Root name, relative to the user's home directory. Shared with the injected
/// class, so it is a wire format and not a preference.
const ROOT: &str = ".orange-registry";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrangeHome {
    root: PathBuf,
}

impl OrangeHome {
    /// The platform default, whether or not it exists yet.
    pub fn discover() -> Result<Self> {
        let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
        let home = std::env::var_os(var).ok_or(Error::Install(bitwig_install::Error::NoHome))?;
        Ok(Self { root: Path::new(&home).join(ROOT) })
    }

    /// An explicit root. Tests use it to keep off the real one.
    pub fn at(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The entry list. Read by this app and by the prepared installation.
    pub fn entries(&self) -> PathBuf {
        self.root.join("entries.tsv")
    }

    /// Where preparation keeps a pristine copy of what it replaces, one
    /// directory per Bitwig build.
    pub fn backups(&self) -> PathBuf {
        self.root.join("backups")
    }
}
