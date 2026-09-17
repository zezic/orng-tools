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

use crate::{Error, ItemVersion, Kind, LibraryPath, Provenance, Registration, Result, fs};

/// Format marker, written first and checked on the way back in. The reader
/// refuses a number it does not know rather than misreading a future layout.
const MARKER: &str = "#orng-registry";

/// The layouts this reader understands.
///
/// Growing the line is deliberately cheap on the side that matters: the class
/// injected into Bitwig reads the first four columns and accepts any row with at
/// least that many, so an installation prepared before this change keeps working
/// against a longer list. Only this parser had to be taught the new shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Format {
    /// Six columns, ending at keywords. Predates the catalog, so everything it
    /// holds was a file the user chose.
    V1,
    /// Eight columns: version and source, which is what tells an available
    /// update apart from a local edit.
    V2,
}

impl Format {
    const CURRENT: Format = Format::V2;

    fn parse(number: &str) -> Option<Format> {
        match number {
            "1" => Some(Format::V1),
            "2" => Some(Format::V2),
            _ => None,
        }
    }

    fn columns(self) -> usize {
        match self {
            Format::V1 => 6,
            Format::V2 => 8,
        }
    }

    fn number(self) -> u32 {
        match self {
            Format::V1 => 1,
            Format::V2 => 2,
        }
    }
}

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
        let mut format = None;
        let mut entries = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let line = line.trim_end_matches('\r');
            let number = index + 1;
            if line.is_empty() {
                continue;
            }
            if let Some(rest) = line.strip_prefix(MARKER) {
                let reason = "entry list is a format this build does not know";
                format = Some(
                    Format::parse(rest.trim())
                        .ok_or(Error::MalformedManifest { line: number, reason })?,
                );
                continue;
            }
            if line.starts_with('#') {
                continue;
            }
            // An entry before the marker would have to be read under a guessed
            // layout, which is the guessing the marker exists to prevent.
            let reason = "entry list has no format marker";
            let format = format.ok_or(Error::MalformedManifest { line: number, reason })?;
            entries.push(parse_line(line, number, format)?);
        }
        Ok(Manifest { entries })
    }

    pub fn to_tsv(&self) -> String {
        let mut out = format!("{MARKER} {}\n", Format::CURRENT.number());
        for entry in &self.entries {
            let (version, source) = columns_for(&entry.provenance);
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{version}\t{source}\n",
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

/// Provenance as its two columns: the version its source can name, then the
/// source itself.
///
/// Source last, and so never empty, because a version is what goes missing: a
/// local file has none, and a trailing empty column is the one an editor that
/// strips trailing whitespace would silently eat.
fn columns_for(provenance: &Provenance) -> (String, &'static str) {
    match provenance {
        Provenance::Local => (String::new(), "local"),
        Provenance::Catalog { version } => (version.to_string(), "catalog"),
    }
}

fn parse_provenance(version: &str, source: &str) -> Option<Provenance> {
    match (source, version) {
        ("local", "") => Some(Provenance::Local),
        ("catalog", version) => {
            Some(Provenance::Catalog { version: version.parse::<ItemVersion>().ok()? })
        }
        // A local file with a version, or a catalog item without one, is a row
        // no writer here produces and no reader can act on.
        _ => None,
    }
}

fn parse_line(line: &str, number: usize, format: Format) -> Result<Registration> {
    let columns: Vec<&str> = line.split('\t').collect();
    if columns.len() != format.columns() {
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
    let provenance = match format {
        // Everything written under version 1 predates the catalog, so it can
        // only have come from a file the user chose.
        Format::V1 => Provenance::Local,
        Format::V2 => parse_provenance(columns[6], columns[7])
            .ok_or(Error::MalformedManifest { line: number, reason: "bad provenance" })?,
    };

    Ok(Registration {
        uuid,
        kind,
        name: columns[2].to_owned(),
        library_path,
        description: columns[4].to_owned(),
        keywords: columns[5].split_whitespace().map(str::to_owned).collect(),
        provenance,
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
            provenance: Provenance::Local,
        }
    }

    fn from_catalog() -> Registration {
        Registration {
            provenance: Provenance::Catalog { version: "2.0.3".parse().unwrap() },
            ..sample()
        }
    }

    /// One entry line under the current marker.
    fn row(line: &str) -> String {
        format!("{MARKER} {}\n{line}\n", Format::CURRENT.number())
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
        assert!(Manifest::parse(&row("not-a-uuid\tDEVICE\tA\tdevices/a.bwdevice\t\t\t\tlocal")).is_err());
        assert!(Manifest::parse("#orng-registry 2\n#comment\n\n").unwrap().is_empty());
    }

    #[test]
    fn a_catalog_item_round_trips_with_the_version_it_was_installed_at() {
        let mut manifest = Manifest::default();
        manifest.insert(from_catalog());
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed, manifest);
        let installed = Provenance::Catalog { version: "2.0.3".parse().unwrap() };
        assert_eq!(parsed.entries()[0].provenance, installed);
    }

    #[test]
    fn the_source_column_is_last_so_it_is_never_the_empty_one() {
        let mut manifest = Manifest::default();
        manifest.insert(sample());
        let line = manifest.to_tsv().lines().nth(1).unwrap().to_owned();
        assert!(line.ends_with("\tlocal"), "{line:?}");
    }

    #[test]
    fn a_list_written_before_the_catalog_reads_as_local_content() {
        // What is on a user's disk today: six columns under marker 1. It has to
        // keep loading, and everything in it predates the catalog.
        let v1 = "#orng-registry 1\n\
            80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
            devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\n";
        let parsed = Manifest::parse(v1).unwrap();
        assert_eq!(parsed.entries(), &[sample()]);

        // And is rewritten in the current format, not the one it arrived in.
        assert!(parsed.to_tsv().starts_with("#orng-registry 2\n"));
    }

    #[test]
    fn a_format_this_build_does_not_know_is_refused_rather_than_guessed_at() {
        let ahead = "#orng-registry 99\n\
            80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tA\tdevices/a.bwdevice\t\t\t\tlocal\n";
        assert!(Manifest::parse(ahead).is_err());
        // Nor may an entry be read under a layout nobody declared.
        let bare = "80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tA\ta\t\t\t\tlocal\n";
        assert!(Manifest::parse(bare).is_err());
    }

    #[test]
    fn provenance_that_contradicts_itself_is_refused() {
        let uuid = "80c0dc4c-d142-53a7-85ee-b91427819b66";
        let path = "devices/My Devices/A.bwdevice";
        // A local file cannot be at a published version, and a catalog item
        // cannot be at none: neither says anything an update check could use.
        for (version, source) in [("2.0.3", "local"), ("", "catalog"), ("nonsense", "catalog")] {
            let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t{version}\t{source}");
            assert!(Manifest::parse(&row(&line)).is_err(), "accepted {version:?} {source:?}");
        }
    }
}
