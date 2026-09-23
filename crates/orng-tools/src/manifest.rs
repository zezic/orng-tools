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

use crate::{
    Digest, Error, ItemVersion, Kind, LibraryPath, Provenance, Registration, Result, Revision, fs,
};

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
    /// Nine columns: the digest of the document as it was placed, which is what
    /// tells a file somebody else rewrote apart from one that is simply gone.
    ///
    /// Written before the two provenance columns rather than after them, so the
    /// column that goes empty stays interior - the same hazard [`columns_for`]
    /// names, and the digest is empty for every row registered before this.
    V3,
    /// Ten columns: the catalog commit an item was reviewed in, which is what
    /// lets an installed entry name the change that published it rather than
    /// only the version it was at.
    ///
    /// Written between the version and the source for the reason the digest is
    /// written before both: it is empty for every row that is not a catalog
    /// item, and the source column stays last so that the column an editor
    /// might strip is never the empty one.
    V4,
}

impl Format {
    const CURRENT: Format = Format::V4;

    fn parse(number: &str) -> Option<Format> {
        match number {
            "1" => Some(Format::V1),
            "2" => Some(Format::V2),
            "3" => Some(Format::V3),
            "4" => Some(Format::V4),
            _ => None,
        }
    }

    fn columns(self) -> usize {
        match self {
            Format::V1 => 6,
            Format::V2 => 8,
            Format::V3 => 9,
            Format::V4 => 10,
        }
    }

    fn number(self) -> u32 {
        match self {
            Format::V1 => 1,
            Format::V2 => 2,
            Format::V3 => 3,
            Format::V4 => 4,
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
            let (version, reviewed_in, source) = columns_for(&entry.provenance);
            let digest = entry.digest.as_ref().map(Digest::as_str).unwrap_or_default();
            out.push_str(&format!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{digest}\t{version}\t{reviewed_in}\t{source}\n",
                entry.uuid,
                entry.kind().enum_constant(),
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

/// Provenance as its three columns: the version its source can name, the change
/// that published it, then the source itself.
///
/// Source last, and so never empty, because the other two are what go missing: a
/// local file has neither, and a trailing empty column is the one an editor that
/// strips trailing whitespace would silently eat.
fn columns_for(provenance: &Provenance) -> (String, &str, &'static str) {
    match provenance {
        Provenance::Local => (String::new(), "", "local"),
        Provenance::Catalog { version, reviewed_in } => (
            version.to_string(),
            reviewed_in.as_ref().map(Revision::as_str).unwrap_or_default(),
            "catalog",
        ),
    }
}

/// Read them back, refusing anything that says two things at once.
///
/// `reviewed_in` is `None` for every row written before format 4 and for any
/// item whose index could not name the commit that published it, so an empty
/// column is an answer rather than a fault. Anything else there has to be a
/// revision, for the reason a digest that cannot be one is refused: a link built
/// from it would go nowhere and say nothing about why.
fn parse_provenance(version: &str, reviewed_in: &str, source: &str) -> Option<Provenance> {
    match (source, version) {
        ("local", "") if reviewed_in.is_empty() => Some(Provenance::Local),
        ("catalog", version) => Some(Provenance::Catalog {
            version: version.parse::<ItemVersion>().ok()?,
            reviewed_in: match reviewed_in {
                "" => None,
                text => Some(Revision::new(text).ok()?),
            },
        }),
        // A local file with a version or a review, or a catalog item without a
        // version, is a row no writer here produces and no reader can act on.
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
    // The column is kept for whoever reads the list by eye, and held to what the
    // path says rather than trusted beside it.
    if library_path.kind() != kind {
        let reason = "kind is not the library path's";
        return Err(Error::MalformedManifest { line: number, reason });
    }
    let provenance = match format {
        // Everything written under version 1 predates the catalog, so it can
        // only have come from a file the user chose.
        Format::V1 => Provenance::Local,
        // Nothing written before version 4 recorded which change published an
        // item, so every row read under an older layout is told that no review
        // was named - which is the truth about it.
        Format::V2 => parse_provenance(columns[6], "", columns[7])
            .ok_or(Error::MalformedManifest { line: number, reason: "bad provenance" })?,
        Format::V3 => parse_provenance(columns[7], "", columns[8])
            .ok_or(Error::MalformedManifest { line: number, reason: "bad provenance" })?,
        Format::V4 => parse_provenance(columns[7], columns[8], columns[9])
            .ok_or(Error::MalformedManifest { line: number, reason: "bad provenance" })?,
    };
    // Nothing written before version 3 recorded what it placed, and an empty
    // column says the same thing: this row cannot be held against its document.
    // Anything else in that column has to be a digest, because a row that
    // carries one and cannot be compared is worse than one that carries none.
    let digest = match format {
        Format::V1 | Format::V2 => None,
        Format::V3 | Format::V4 if columns[6].is_empty() => None,
        Format::V3 | Format::V4 => Some(
            Digest::new(columns[6])
                .map_err(|_| Error::MalformedManifest { line: number, reason: "bad digest" })?,
        ),
    };

    Ok(Registration {
        uuid,
        name: columns[2].to_owned(),
        library_path,
        description: columns[4].to_owned(),
        keywords: columns[5].split_whitespace().map(str::to_owned).collect(),
        digest,
        provenance,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Registration {
        Registration {
            uuid: Uuid::parse_str("80c0dc4c-d142-53a7-85ee-b91427819b66").unwrap(),
            name: "DISPERSER".into(),
            library_path: LibraryPath::new("devices/My Devices/DISPERSER.bwdevice").unwrap(),
            description: "Allpass phase-rotator".into(),
            keywords: vec!["disperser".into(), "allpass".into()],
            digest: Some(Digest::of(b"DISPERSER")),
            provenance: Provenance::Local,
        }
    }

    /// The commit a sample item was published by. Forty hex digits, as git names
    /// an object and as [`Revision`] insists.
    const REVIEWED_IN: &str = "3f9a1c2e8b4d7a61c05f2d93ab7e14c8f6021b5d";

    fn from_catalog() -> Registration {
        Registration {
            provenance: Provenance::Catalog {
                version: "2.0.3".parse().unwrap(),
                reviewed_in: Some(Revision::new(REVIEWED_IN).unwrap()),
            },
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
        assert!(
            Manifest::parse(&row("not-a-uuid\tDEVICE\tA\tdevices/a.bwdevice\t\t\t\t\t\tlocal"))
                .is_err()
        );
        assert!(Manifest::parse("#orng-registry 4\n#comment\n\n").unwrap().is_empty());
    }

    /// The kind column is held to the library path's extension. The first line
    /// is the same row agreeing with itself, so what refuses the rest is the
    /// disagreement and nothing else about them.
    #[test]
    fn a_kind_its_library_path_does_not_name_is_refused() {
        let uuid = "80c0dc4c-d142-53a7-85ee-b91427819b66";
        assert!(Manifest::parse(&row(&format!(
            "{uuid}\tDEVICE\tA\tdevices/My Devices/A.bwdevice\t\t\t\t\t\tlocal"
        )))
        .is_ok());
        for (kind, path) in
            [("MODULATOR", "devices/My Devices/A.bwdevice"), ("DEVICE", "devices/My Devices/A.txt")]
        {
            let line = format!("{uuid}\t{kind}\tA\t{path}\t\t\t\t\t\tlocal");
            assert!(Manifest::parse(&row(&line)).is_err(), "accepted {kind} at {path}");
        }
    }

    #[test]
    fn a_catalog_item_round_trips_with_the_version_it_was_installed_at() {
        let mut manifest = Manifest::default();
        manifest.insert(from_catalog());
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed, manifest);
        let installed = Provenance::Catalog {
            version: "2.0.3".parse().unwrap(),
            reviewed_in: Some(Revision::new(REVIEWED_IN).unwrap()),
        };
        assert_eq!(parsed.entries()[0].provenance, installed);
    }

    /// The commit a catalog item was published by, back off the disk whole.
    ///
    /// Worth its own assertion because the value is only ever obtainable at the
    /// moment of installing: the catalog goes on publishing, so an item this
    /// build installed today and that is superseded next month has nowhere left
    /// to look the review up. A round trip that lost it would lose it for good.
    #[test]
    fn the_change_that_published_an_item_round_trips() {
        let mut manifest = Manifest::default();
        manifest.insert(from_catalog());
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        let Provenance::Catalog { reviewed_in, .. } = &parsed.entries()[0].provenance else {
            panic!("the catalog item came back as a local file");
        };
        assert_eq!(reviewed_in.as_ref().map(Revision::as_str), Some(REVIEWED_IN));
    }

    /// An index generated inside a pull request cannot name the commit that has
    /// not merged yet, so an item installed from one records no review. That is
    /// an answer and not a fault, and it has to survive the round trip as one
    /// rather than becoming a parse failure.
    #[test]
    fn a_catalog_item_with_no_review_named_is_read_back_as_having_none() {
        let mut manifest = Manifest::default();
        manifest.insert(Registration {
            provenance: Provenance::Catalog {
                version: "2.0.3".parse().unwrap(),
                reviewed_in: None,
            },
            ..sample()
        });
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn the_source_column_is_last_so_it_is_never_the_empty_one() {
        let mut manifest = Manifest::default();
        manifest.insert(sample());
        let line = manifest.to_tsv().lines().nth(1).unwrap().to_owned();
        assert!(line.ends_with("\tlocal"), "{line:?}");
    }

    /// The digest goes where an empty one is harmless. It is empty for every
    /// row registered before this format, and a trailing empty column is the
    /// one an editor that strips whitespace silently eats - which is the reason
    /// the source column was put last in the first place.
    #[test]
    fn the_digest_column_is_interior_so_an_empty_one_survives() {
        let mut manifest = Manifest::default();
        manifest.insert(Registration { digest: None, ..sample() });
        let line = manifest.to_tsv().lines().nth(1).unwrap().to_owned();
        // The digest, the version and the review, all empty, and the source
        // behind them.
        assert!(line.contains("\t\t\t\tlocal"), "{line:?}");
        assert!(line.ends_with("\tlocal"), "{line:?}");
        // And it comes back as nothing recorded rather than as a parse failure.
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed.entries()[0].digest, None);
    }

    /// What a registration records about the document it was placed with, back
    /// off the disk as the same value. Nothing else in the row can stand in for
    /// it, so a round trip that lost it would read as a document nobody can say
    /// anything about.
    #[test]
    fn the_recorded_digest_round_trips() {
        let mut manifest = Manifest::default();
        manifest.insert(sample());
        let parsed = Manifest::parse(&manifest.to_tsv()).unwrap();
        assert_eq!(parsed.entries()[0].digest, Some(Digest::of(b"DISPERSER")));
    }

    /// A row that carries a digest and cannot be compared is worse than one
    /// that carries none: it would read as a document that differs from its
    /// record forever, and the remedy the interface offers for that is to
    /// register the document again.
    #[test]
    fn a_digest_that_cannot_be_one_is_refused_rather_than_read_as_absent() {
        let uuid = "80c0dc4c-d142-53a7-85ee-b91427819b66";
        let path = "devices/My Devices/A.bwdevice";
        let real = Digest::of(b"DISPERSER").to_string();
        for digest in ["nonsense", &real[..63], &real.to_uppercase()] {
            let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t{digest}\t\t\tlocal");
            assert!(Manifest::parse(&row(&line)).is_err(), "accepted {digest:?}");
        }
        // And the real one is accepted, so the loop above is refusing the
        // digest rather than the row around it.
        let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t{real}\t\t\tlocal");
        assert!(Manifest::parse(&row(&line)).is_ok());
    }

    /// The same for the review: a revision that cannot be one is a link that
    /// goes nowhere, and the panel would draw it as somewhere to go.
    #[test]
    fn a_review_that_cannot_be_a_commit_is_refused() {
        let uuid = "80c0dc4c-d142-53a7-85ee-b91427819b66";
        let path = "devices/My Devices/A.bwdevice";
        for review in ["3f9a1c2", &REVIEWED_IN.to_uppercase(), &"z".repeat(40)] {
            let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t\t2.0.3\t{review}\tcatalog");
            assert!(Manifest::parse(&row(&line)).is_err(), "accepted {review:?}");
        }
        let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t\t2.0.3\t{REVIEWED_IN}\tcatalog");
        assert!(Manifest::parse(&row(&line)).is_ok());
    }

    /// The other list on a user's disk today: eight columns under marker 2.
    /// Everything in it was registered by a build that recorded nothing about
    /// the bytes, and nothing can invent that record afterwards - hashing what
    /// is there now would write the present down as the past.
    #[test]
    fn a_list_written_before_the_digest_says_nothing_about_its_documents() {
        let v2 = "#orng-registry 2\n\
            80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
            devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\t\
            \tlocal\n";
        let parsed = Manifest::parse(v2).unwrap();
        assert_eq!(parsed.entries(), &[Registration { digest: None, ..sample() }]);
        assert!(parsed.to_tsv().starts_with("#orng-registry 4\n"));
    }

    /// The list the build before this one wrote: nine columns under marker 3,
    /// with the digest but no review. It keeps loading, its digest survives, and
    /// its catalog rows come back naming no review - which is the truth about
    /// them, because nothing recorded one.
    #[test]
    fn a_list_written_before_the_review_says_nothing_about_which_change_published_it() {
        let real = Digest::of(b"DISPERSER");
        let v3 = format!(
            "#orng-registry 3\n\
            80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
            devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\t\
            {real}\t2.0.3\tcatalog\n"
        );
        let parsed = Manifest::parse(&v3).unwrap();
        assert_eq!(parsed.entries()[0].digest, Some(real));
        assert_eq!(
            parsed.entries()[0].provenance,
            Provenance::Catalog { version: "2.0.3".parse().unwrap(), reviewed_in: None }
        );
        assert!(parsed.to_tsv().starts_with("#orng-registry 4\n"));
    }

    #[test]
    fn a_list_written_before_the_catalog_reads_as_local_content() {
        // What is on a user's disk today: six columns under marker 1. It has to
        // keep loading, and everything in it predates the catalog.
        let v1 = "#orng-registry 1\n\
            80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
            devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\n";
        let parsed = Manifest::parse(v1).unwrap();
        assert_eq!(parsed.entries(), &[Registration { digest: None, ..sample() }]);

        // And is rewritten in the current format, not the one it arrived in.
        assert!(parsed.to_tsv().starts_with("#orng-registry 4\n"));
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
        // Nor can a local file have been reviewed anywhere.
        for (version, review, source) in [
            ("2.0.3", "", "local"),
            ("", "", "catalog"),
            ("nonsense", "", "catalog"),
            ("", REVIEWED_IN, "local"),
        ] {
            let line = format!("{uuid}\tDEVICE\tA\t{path}\t\t\t\t{version}\t{review}\t{source}");
            assert!(Manifest::parse(&row(&line)).is_err(), "accepted {version:?} {source:?}");
        }
    }
}
