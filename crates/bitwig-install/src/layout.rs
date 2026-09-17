// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: MIT OR Apache-2.0

use std::path::{Path, PathBuf};

use crate::{ENV_APP, ENV_JAR, ENV_RESOURCES, Error, Result, env_path, home, probe};

/// Where the JAR directory sits inside an install root. `Contents/Java` is the
/// verified macOS layout; the rest are probed by existence, never assumed.
const JAVA_DIRS: &[&str] = &["Contents/Java", "bin", "lib/bitwig-studio", "."];

/// Where content directories sit inside an install root.
///
/// Probed once per directory that is wanted, not once for all of them. `Library`
/// and `localization` are siblings on macOS and Linux and are *not* on Windows,
/// where the library sits at the install root and localization under
/// `resources`. Assuming one parent for both resolved localization to a path
/// that does not exist there.
const CONTENT_DIRS: &[&str] = &["Contents/Resources", "resources", "lib/bitwig-studio", "."];

const MAC_JVM_ARM: &str = "Contents/PlugIns/JavaVM-arm64.bundle/Contents/Home";
const MAC_JVM_X64: &str = "Contents/PlugIns/JavaVM-x64.bundle/Contents/Home";

/// Where the bundled JVM sits, this build's architecture first.
///
/// macOS ships both bundles, so probing by existence alone would find the
/// foreign one on an Intel machine and fail to execute it -- which, during
/// verification, would look like a bad patch.
#[cfg(target_arch = "aarch64")]
const JVM_DIRS: &[&str] = &[MAC_JVM_ARM, MAC_JVM_X64, "lib/jre", "jre"];
#[cfg(not(target_arch = "aarch64"))]
const JVM_DIRS: &[&str] = &[MAC_JVM_X64, MAC_JVM_ARM, "lib/jre", "jre"];

/// A resolved Bitwig Studio installation.
///
/// Construction probes the layout once; every accessor is then a pure join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    root: PathBuf,
    java_dir: PathBuf,
    /// Factory content. Not necessarily beside `localization`.
    library_dir: PathBuf,
    /// Description and keyword bundles. Not necessarily beside `Library`.
    localization_dir: PathBuf,
}

impl Installation {
    /// Resolve the installation the environment points at, or the first
    /// platform default that exists.
    pub fn discover() -> Result<Self> {
        if let Some(root) = env_path(ENV_APP) {
            return Self::at(&root);
        }
        // A bare BITWIG_JAR override still needs resources; treat the jar's
        // grandparent as the root and let the probes sort the rest out.
        if let Some(jar) = env_path(ENV_JAR) {
            let root = jar.parent().and_then(Path::parent).unwrap_or(Path::new("."));
            return Self::at(root);
        }
        default_roots()
            .into_iter()
            .find(|p| p.is_dir())
            .ok_or_else(|| Error::NoInstallation { searched: searched_hint() })
            .and_then(|root| Self::at(&root))
    }

    /// Resolve a specific install root, failing if it does not hold a Bitwig.
    pub fn at(root: &Path) -> Result<Self> {
        let java_dir = match env_path(ENV_JAR) {
            Some(jar) => jar.parent().map(Path::to_path_buf).unwrap_or_default(),
            None => probe(root, JAVA_DIRS, |d| d.join("bitwig.jar").is_file())
                .ok_or_else(|| Error::NotAnInstallation(root.into(), "bitwig.jar"))?,
        };
        // Each content directory is found by looking for itself. The override
        // still names one parent, because the only thing that sets it is a test
        // building a mirror, and a mirror is built with them side by side.
        let content = |what: &'static str| match env_path(ENV_RESOURCES) {
            Some(dir) => Ok(dir.join(what)),
            None => probe(root, CONTENT_DIRS, |d| d.join(what).is_dir())
                .map(|dir| dir.join(what))
                .ok_or_else(|| Error::NotAnInstallation(root.into(), what)),
        };
        let library_dir = content("Library")?;
        let localization_dir = content("localization")?;
        Ok(Self { root: root.to_path_buf(), java_dir, library_dir, localization_dir })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn jar(&self) -> PathBuf {
        env_path(ENV_JAR).unwrap_or_else(|| self.java_dir.join("bitwig.jar"))
    }

    /// The bundled third-party classpath. Needed on the classpath when running
    /// anything against `bitwig.jar`.
    pub fn libs_jar(&self) -> PathBuf {
        self.java_dir.join("libs.jar")
    }

    /// Factory content: `devices/`, `modulators/`, `modules/`, `presets/`.
    pub fn library_dir(&self) -> PathBuf {
        self.library_dir.clone()
    }

    /// Browser descriptions and search keywords live here, as properties files.
    pub fn localization_dir(&self) -> PathBuf {
        self.localization_dir.clone()
    }

    /// Bitwig ships a JRE. It can run a class against the patched jar, which is
    /// how a patch is verified before it is activated. It has no compiler.
    pub fn bundled_java(&self) -> Option<PathBuf> {
        let exe = if cfg!(windows) { "bin/java.exe" } else { "bin/java" };
        probe(&self.root, JVM_DIRS, |d| d.join(exe).is_file()).map(|home| home.join(exe))
    }
}

/// The user's own Bitwig content. Survives application updates, which is why
/// registered documents belong here rather than inside the installation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UserLibrary {
    root: PathBuf,
}

impl UserLibrary {
    /// The platform default location, whether or not it exists yet.
    pub fn discover() -> Result<Self> {
        let home = home()?;
        let root = if cfg!(target_os = "linux") {
            home.join("Bitwig Studio/Library")
        } else {
            home.join("Documents/Bitwig Studio/Library")
        };
        Ok(Self { root })
    }

    pub fn at(root: &Path) -> Self {
        Self { root: root.to_path_buf() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// The folder Bitwig's "Save device..." writes into. Created on demand, so
    /// it may not exist on a fresh install.
    pub fn folder(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }
}

/// Bitwig's settings directory: `config.json`, caches, the run lock.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppData {
    root: PathBuf,
}

impl AppData {
    pub fn discover() -> Result<Self> {
        let home = home()?;
        // Windows is Local, not Roaming. There is no Roaming directory at all:
        // Bitwig keeps prefs, caches and its logs under Local, which is also
        // where it says it writes them on startup. Verified on 6.1; the Linux
        // path has not been.
        let root = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Bitwig/Bitwig Studio")
        } else if cfg!(windows) {
            home.join("AppData/Local/Bitwig Studio")
        } else {
            home.join(".BitwigStudio")
        };
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_json(&self) -> PathBuf {
        self.root.join("config.json")
    }

    pub fn lock_file(&self) -> PathBuf {
        self.root.join("lock")
    }
}

fn default_roots() -> Vec<PathBuf> {
    let home = home().ok();
    let mut roots = Vec::new();
    if cfg!(target_os = "macos") {
        roots.push(PathBuf::from("/Applications/Bitwig Studio.app"));
        if let Some(h) = &home {
            roots.push(h.join("Applications/Bitwig Studio.app"));
        }
    } else if cfg!(windows) {
        if let Some(pf) = env_path("ProgramFiles") {
            roots.push(pf.join("Bitwig Studio"));
        }
        roots.push(PathBuf::from(r"C:\Program Files\Bitwig Studio"));
    } else {
        roots.push(PathBuf::from("/opt/bitwig-studio"));
        roots.push(PathBuf::from("/usr/lib/bitwig-studio"));
        if let Some(h) = &home {
            roots.push(h.join(".local/share/bitwig-studio"));
        }
    }
    roots
}

fn searched_hint() -> String {
    default_roots()
        .iter()
        .map(|p| p.display().to_string())
        .collect::<Vec<_>>()
        .join(", ")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Skips only when told to. Absent that, a missing installation fails:
    /// this test returning quietly is how a layout that was wrong on Windows
    /// stayed wrong, on every machine that did not have Bitwig to check it
    /// against.
    #[test]
    fn discovers_a_real_installation() {
        let install = match Installation::discover() {
            Ok(install) => install,
            Err(_) if std::env::var_os("ORNG_SKIP_BITWIG_TESTS").is_some() => {
                eprintln!("no Bitwig Studio installed, skipping");
                return;
            }
            Err(e) => panic!("{e}; set ORNG_SKIP_BITWIG_TESTS=1 to skip these tests"),
        };
        assert!(install.jar().is_file(), "jar missing at {:?}", install.jar());
        assert!(install.libs_jar().is_file(), "libs missing at {:?}", install.libs_jar());

        // The two content directories are found separately because they are not
        // always siblings: on Windows the library is at the install root and
        // localization is under `resources`.
        let library = install.library_dir();
        assert!(library.join("devices").is_dir(), "no devices under {library:?}");
        assert!(library.join("modulators").is_dir(), "no modulators under {library:?}");
        assert!(library.join("modules").is_dir(), "no modules under {library:?}");

        let localization = install.localization_dir();
        assert!(localization.is_dir(), "no localization at {localization:?}");
        assert!(
            std::fs::read_dir(&localization)
                .into_iter()
                .flatten()
                .flatten()
                .any(|e| e.file_name().to_string_lossy().ends_with(".properties")),
            "no properties bundles under {localization:?}"
        );

        // The bundled JRE is what verifies a patch before it is activated, so
        // an installation without one cannot be prepared.
        assert!(install.bundled_java().is_some_and(|j| j.is_file()), "no bundled JVM");
    }
}
