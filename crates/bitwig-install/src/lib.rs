//! Locating a Bitwig Studio installation and the user data that belongs to it.
//!
//! Three roots are distinct and must not be confused:
//!
//! - [`Installation`] is the application: `bitwig.jar`, the factory `Library`,
//!   the localization bundles, the bundled JVM.
//! - [`UserLibrary`] is the user's own content, under `Documents` or `$HOME`.
//! - [`AppData`] is Bitwig's settings directory.
//!
//! Every path into any of them is resolved here and nowhere else.

mod layout;
mod running;

use std::path::{Path, PathBuf};

pub use layout::{Installation, UserLibrary, AppData};
pub use running::{RunState, running_state};

/// Install root override. The `Bitwig Studio.app` bundle on macOS, the install
/// directory elsewhere.
pub const ENV_APP: &str = "BITWIG_APP";
/// Direct override for `bitwig.jar`. Wins over [`ENV_APP`].
pub const ENV_JAR: &str = "BITWIG_JAR";
/// Direct override for the directory holding `Library/`, `localization/`, `icons/`.
pub const ENV_RESOURCES: &str = "BITWIG_RESOURCES";

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("no Bitwig Studio installation found (looked at: {searched})")]
    NoInstallation { searched: String },
    #[error("{0} does not look like a Bitwig Studio installation: {1} is missing")]
    NotAnInstallation(PathBuf, &'static str),
    #[error("cannot determine the home directory")]
    NoHome,
}

pub type Result<T> = std::result::Result<T, Error>;

/// Read an environment variable as a path, treating empty as unset.
pub(crate) fn env_path(var: &str) -> Option<PathBuf> {
    std::env::var_os(var)
        .map(PathBuf::from)
        .filter(|p| !p.as_os_str().is_empty())
}

pub(crate) fn home() -> Result<PathBuf> {
    let var = if cfg!(windows) { "USERPROFILE" } else { "HOME" };
    env_path(var).ok_or(Error::NoHome)
}

/// First candidate under `root` for which `accept` holds.
pub(crate) fn probe(root: &Path, candidates: &[&str], accept: impl Fn(&Path) -> bool) -> Option<PathBuf> {
    candidates
        .iter()
        .map(|rel| if *rel == "." { root.to_path_buf() } else { root.join(rel) })
        .find(|dir| accept(dir))
}
