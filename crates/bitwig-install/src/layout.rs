use std::path::{Path, PathBuf};

use crate::{ENV_APP, ENV_JAR, ENV_RESOURCES, Error, Result, env_path, home, probe};

/// Where the JAR directory sits inside an install root. `Contents/Java` is the
/// verified macOS layout; the rest are probed by existence, never assumed.
const JAVA_DIRS: &[&str] = &["Contents/Java", "bin", "lib/bitwig-studio", "."];

/// Where the resources directory sits inside an install root.
const RESOURCE_DIRS: &[&str] = &["Contents/Resources", "resources", "lib/bitwig-studio", "."];

/// Where the bundled JVM sits. macOS ships one bundle per architecture.
const JVM_DIRS: &[&str] = &[
    "Contents/PlugIns/JavaVM-arm64.bundle/Contents/Home",
    "Contents/PlugIns/JavaVM-x64.bundle/Contents/Home",
    "lib/jre",
    "jre",
];

/// A resolved Bitwig Studio installation.
///
/// Construction probes the layout once; every accessor is then a pure join.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Installation {
    root: PathBuf,
    java_dir: PathBuf,
    resources_dir: PathBuf,
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
        let resources_dir = match env_path(ENV_RESOURCES) {
            Some(dir) => dir,
            None => probe(root, RESOURCE_DIRS, |d| d.join("Library").is_dir())
                .ok_or_else(|| Error::NotAnInstallation(root.into(), "Library"))?,
        };
        Ok(Self { root: root.to_path_buf(), java_dir, resources_dir })
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
        self.resources_dir.join("Library")
    }

    /// Browser descriptions and search keywords live here, as properties files.
    pub fn localization_dir(&self) -> PathBuf {
        self.resources_dir.join("localization")
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
        let root = if cfg!(target_os = "macos") {
            home.join("Library/Application Support/Bitwig/Bitwig Studio")
        } else if cfg!(windows) {
            home.join("AppData/Roaming/Bitwig Studio")
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

    /// Skips rather than fails when no Bitwig is installed, so the suite stays
    /// green on machines that only build the library.
    #[test]
    fn discovers_a_real_installation() {
        let Ok(install) = Installation::discover() else {
            eprintln!("no Bitwig Studio installed, skipping");
            return;
        };
        assert!(install.jar().is_file(), "jar missing at {:?}", install.jar());
        assert!(install.library_dir().join("devices").is_dir());
        assert!(install.localization_dir().is_dir());
    }
}
