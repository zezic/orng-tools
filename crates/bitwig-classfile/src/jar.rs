use std::collections::BTreeMap;
use std::io::Read;
use std::ops::ControlFlow;
use std::path::{Path, PathBuf};

use zip::{ZipArchive, ZipWriter};

use crate::{Error, Result};

/// A JAR opened for reading. Each operation reopens the file, so a `Jar` is
/// cheap to hold and safe to keep across a rewrite of the archive it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Jar {
    path: PathBuf,
}

impl Jar {
    pub fn open(path: &Path) -> Result<Self> {
        if !path.is_file() {
            return Err(Error::Io {
                path: path.display().to_string(),
                source: std::io::Error::from(std::io::ErrorKind::NotFound),
            });
        }
        Ok(Jar { path: path.to_path_buf() })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    fn archive(&self) -> Result<ZipArchive<std::fs::File>> {
        let file = std::fs::File::open(&self.path).map_err(|source| Error::Io {
            path: self.path.display().to_string(),
            source,
        })?;
        ZipArchive::new(file).map_err(|e| Error::Archive(e.to_string()))
    }

    /// Read one entry by name.
    pub fn entry(&self, name: &str) -> Result<Vec<u8>> {
        let mut archive = self.archive()?;
        let mut entry = archive
            .by_name(name)
            .map_err(|_| Error::EntryNotFound(name.to_owned()))?;
        let mut buf = Vec::with_capacity(entry.size() as usize);
        entry.read_to_end(&mut buf).map_err(|source| Error::Io {
            path: name.to_owned(),
            source,
        })?;
        Ok(buf)
    }

    /// Visit every `.class` entry until `visit` yields a value.
    ///
    /// Anchor resolution is a single sweep that stops as soon as it has what it
    /// needs, which on a 30k-entry archive is most of the cost saved.
    pub fn find_class<T>(
        &self,
        mut visit: impl FnMut(&str, &[u8]) -> Option<T>,
    ) -> Result<Option<T>> {
        let mut found = None;
        self.visit_classes(|name, bytes| match visit(name, bytes) {
            Some(value) => {
                found = Some(value);
                ControlFlow::Break(())
            }
            None => ControlFlow::Continue(()),
        })?;
        Ok(found)
    }

    /// Visit every `.class` entry, in archive order.
    pub fn visit_classes(
        &self,
        mut visit: impl FnMut(&str, &[u8]) -> ControlFlow<()>,
    ) -> Result<()> {
        let mut archive = self.archive()?;
        let mut buf = Vec::new();
        for i in 0..archive.len() {
            let mut entry = archive
                .by_index(i)
                .map_err(|e| Error::Archive(e.to_string()))?;
            let name = entry.name().to_owned();
            if !name.ends_with(".class") {
                continue;
            }
            buf.clear();
            buf.reserve(entry.size() as usize);
            entry.read_to_end(&mut buf).map_err(|source| Error::Io {
                path: name.clone(),
                source,
            })?;
            if visit(&name, &buf).is_break() {
                return Ok(());
            }
        }
        Ok(())
    }

    /// Write a copy of this JAR with `edits` applied.
    ///
    /// Untouched entries are copied without recompressing, which keeps a full
    /// rewrite to seconds rather than minutes.
    pub fn rewrite(&self, output: &Path, edits: &JarEdits) -> Result<()> {
        let mut archive = self.archive()?;
        let file = std::fs::File::create(output).map_err(|source| Error::Io {
            path: output.display().to_string(),
            source,
        })?;
        let mut writer = ZipWriter::new(std::io::BufWriter::new(file));
        let options: zip::write::FileOptions<'_, ()> =
            zip::write::FileOptions::default().compression_method(zip::CompressionMethod::Deflated);

        for i in 0..archive.len() {
            let entry = archive
                .by_index_raw(i)
                .map_err(|e| Error::Archive(e.to_string()))?;
            let name = entry.name().to_owned();
            match edits.replacements.get(&name) {
                Some(bytes) => {
                    drop(entry);
                    writer
                        .start_file(&name, options)
                        .map_err(|e| Error::Archive(e.to_string()))?;
                    std::io::Write::write_all(&mut writer, bytes).map_err(|source| Error::Io {
                        path: name,
                        source,
                    })?;
                }
                None => writer
                    .raw_copy_file(entry)
                    .map_err(|e| Error::Archive(e.to_string()))?,
            }
        }

        for (name, bytes) in &edits.additions {
            writer
                .start_file(name, options)
                .map_err(|e| Error::Archive(e.to_string()))?;
            std::io::Write::write_all(&mut writer, bytes).map_err(|source| Error::Io {
                path: name.clone(),
                source,
            })?;
        }

        writer.finish().map_err(|e| Error::Archive(e.to_string()))?;
        Ok(())
    }
}

/// Entries to replace and entries to add when rewriting a JAR.
#[derive(Debug, Default, Clone)]
pub struct JarEdits {
    replacements: BTreeMap<String, Vec<u8>>,
    additions: BTreeMap<String, Vec<u8>>,
}

impl JarEdits {
    pub fn new() -> Self {
        Self::default()
    }

    /// Replace an existing entry. Replacing the same entry twice is a bug: the
    /// second write would silently discard the first edit.
    pub fn replace(&mut self, name: impl Into<String>, bytes: Vec<u8>) -> &mut Self {
        let name = name.into();
        assert!(
            self.replacements.insert(name.clone(), bytes).is_none(),
            "{name} replaced twice in one rewrite"
        );
        self
    }

    /// Add an entry that is not in the source archive.
    pub fn add(&mut self, name: impl Into<String>, bytes: Vec<u8>) -> &mut Self {
        let name = name.into();
        assert!(
            self.additions.insert(name.clone(), bytes).is_none(),
            "{name} added twice in one rewrite"
        );
        self
    }

    pub fn is_empty(&self) -> bool {
        self.replacements.is_empty() && self.additions.is_empty()
    }

    /// Entry names this edit set touches, for reporting.
    pub fn touched(&self) -> impl Iterator<Item = &str> {
        self.replacements.keys().chain(self.additions.keys()).map(String::as_str)
    }
}
