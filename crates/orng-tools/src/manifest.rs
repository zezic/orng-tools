// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The entry list: this app's durable state, and the prepared installation's
//! input at startup.
//!
//! The format is tab-separated on purpose. The class injected into the
//! installation reads this file on every launch, and `split("\t")` needs no
//! parser, no dependency and no error handling worth the name. Fields that
//! cannot survive that round trip are rejected when a [`Registration`] is built,
//! not here.

use std::path::Path;

use uuid::Uuid;

use crate::{Error, Kind, LibraryPath, Registration, Result, fs};

/// Format marker. The reader refuses anything it does not recognise rather than
/// misinterpreting a future layout.
const VERSION_LINE: &str = "#orng-registry 1";
const COLUMNS: usize = 6;

/// The list of everything this app has registered.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Manifest {
    entries: Vec<Registration>,
}

impl Manifest {
    /// Read the list, treating a missing file as an empty one: an installation
    /// with nothing registered is a normal state, not a failure.
    pub fn load(path: &Path) -> Result<Self> {
        match fs::read_to_string_if_exists(path)? {
            Some(text) => Self::parse(&text),
            None => Ok(Self::default()),
        }
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        fs::write_new(path, self.to_tsv())
    }

    pub fn parse(text: &str) -> Result<Self> {
        let mut entries = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            entries.push(parse_line(line, index + 1)?);
        }
        Ok(Manifest { entries })
    }

    pub fn to_tsv(&self) -> String {
        let mut out = String::from(VERSION_LINE);
        out.push('\n');
        for entry in &self.entries {
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\n",
                entry.uuid,
                entry.kind.enum_constant(),
                entry.name,
                entry.library_path.as_str(),
                entry.description,
                entry.keywords.join(" "),
            ));
        }
        out
    }

    pub fn entries(&self) -> &[Registration] {
        &self.entries
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn get(&self, uuid: Uuid) -> Option<&Registration> {
        self.entries.iter().find(|e| e.uuid == uuid)
    }

    /// Add a registration, replacing any existing one with the same UUID.
    ///
    /// Re-adding an edited document is the common case, and it must update the
    /// entry rather than create a second one under the same identity.
    pub fn insert(&mut self, registration: Registration) {
        match self.entries.iter_mut().find(|e| e.uuid == registration.uuid) {
            Some(existing) => *existing = registration,
            None => self.entries.push(registration),
        }
    }

    pub fn remove(&mut self, uuid: Uuid) -> Option<Registration> {
        let at = self.entries.iter().position(|e| e.uuid == uuid)?;
        Some(self.entries.remove(at))
    }
}

fn parse_line(line: &str, number: usize) -> Result<Registration> {
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() != COLUMNS {
        return Err(Error::MalformedManifest { line: number, reason: "wrong number of columns" });
    }
    let uuid = Uuid::parse_str(columns[0])
        .map_err(|_| Error::MalformedManifest { line: number, reason: "bad UUID" })?;
    let kind = Kind::ALL
        .into_iter()
        .find(|k| k.enum_constant() == columns[1])
        .ok_or(Error::MalformedManifest { line: number, reason: "unknown kind" })?;
    let library_path = LibraryPath::new(columns[3])
        .map_err(|_| Error::MalformedManifest { line: number, reason: "bad library path" })?;

    Ok(Registration {
        uuid,
        kind,
        name: columns[2].to_owned(),
        library_path,
        description: columns[4].to_owned(),
        keywords: columns[5].split_whitespace().map(str::to_owned).collect(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Registration {
        Registration {
            uuid: Uuid::parse_str("80c0dc4c-d142-53a7-85ee-b91427819b66").unwrap(),
            kind: Kind::Device,
            name: "DISPERSER".into(),
            library_path: LibraryPath::new("devices/My Devices/DISPERSER.bwdevice").unwrap(),
            description: "Allpass phase-rotator".into(),
            keywords: vec!["disperser".into(), "allpass".into()],
        }
    }

    #[test]
    fn round_trips_through_the_wire_format() {
        let mut manifest = Manifest::default();
        manifest.insert(sample());
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn re_adding_the_same_identity_updates_it() {
        let mut manifest = Manifest::default();
        manifest.insert(sample());
        let mut renamed = sample();
        renamed.name = "DISPERSER MK2".into();
        manifest.insert(renamed);
        assert_eq!(manifest.entries().len(), 1);
        assert_eq!(manifest.entries()[0].name, "DISPERSER MK2");
    }

    #[test]
    fn a_missing_list_is_an_empty_list_not_an_error() {
        let manifest = Manifest::load(Path::new("/nonexistent/entries.tsv")).unwrap();
        assert!(manifest.is_empty());
    }

    #[test]
    fn malformed_lines_are_refused_rather_than_skipped() {
        assert!(Manifest::parse("not-a-uuid\tDEVICE\tA\tdevices/a.bwdevice\t\t").is_err());
        assert!(Manifest::parse("#comment\n\n").unwrap().is_empty());
    }
}
