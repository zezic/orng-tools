// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Documents the user has dropped, before anything has been written.
//!
//! A dropped file becomes a row with one of three answers on it: it can be
//! registered, it cannot be registered as it stands, or it is not something this
//! application can register at all. The distinction is the point - "it did not
//! work" is not a thing a user can act on, and each of the three has a different
//! next move.
//!
//! Nothing here writes. Staging reads the document, derives the registration it
//! would produce, and asks what would collide; the writing is
//! [`orng_tools::Update`]'s, and does not happen until the primary action is
//! pressed.

use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, TryRecvError, channel};

use eframe::egui;
use orng_tools::{
    Destination, Document, DocumentError, Kind, Manifest, Registration, placement,
};

/// A document the user has dropped, as the list shows it.
#[derive(Debug)]
pub struct Staged {
    /// What the row leads with: the document's display name, or the file's own
    /// name when it could not be read as a document at all.
    pub label: String,
    pub state: State,
}

/// What can be done with a dropped file.
#[derive(Debug)]
pub enum State {
    /// Read, and nothing objects to registering it. The document is boxed
    /// because it carries the whole file, and a staged list is a list of these.
    Ready { registration: Registration, document: Box<Document> },
    /// Readable, and registering it would collide with something already here.
    /// The reason is the useful half and is carried on the row.
    Conflict { registration: Registration, why: String },
    /// Not something this application can register.
    Rejected { why: String },
}

impl Staged {
    /// The status word the row shows, in the design's vocabulary.
    pub fn status(&self) -> &'static str {
        match self.state {
            State::Ready { .. } => "Staged",
            State::Conflict { .. } => "Conflict",
            State::Rejected { .. } => "Rejected",
        }
    }

    /// Why this row is not simply staged, when it is not.
    pub fn reason(&self) -> Option<&str> {
        match &self.state {
            State::Ready { .. } => None,
            State::Conflict { why, .. } | State::Rejected { why } => Some(why),
        }
    }

    /// The registration this row describes, when the document could be read.
    ///
    /// A conflicting row has one too: it is what the collision is *about*, and
    /// what the row draws its kind and identity from.
    pub fn registration(&self) -> Option<&Registration> {
        match &self.state {
            State::Ready { registration, .. } | State::Conflict { registration, .. } => {
                Some(registration)
            }
            State::Rejected { .. } => None,
        }
    }

    /// Whether this row is part of the pending work. Only a staged row is.
    pub fn is_ready(&self) -> bool {
        matches!(self.state, State::Ready { .. })
    }
}

/// The extensions a drop accepts, as the empty state names them.
pub const ACCEPTED: [&str; 3] = ["bwdevice", "bwmodulator", "bwmodule"];

/// The files a drop of these paths would actually read.
///
/// A dropped folder is taken **one level deep**, which is what the design
/// promises and what somebody who drops their "My Devices" folder means. Deeper
/// is not offered: a drop is one gesture, and it should not be able to walk a
/// home directory.
pub fn documents_in(paths: &[PathBuf]) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for path in paths {
        if Kind::from_path(path).is_some() {
            found.push(path.clone());
            continue;
        }
        let Ok(dir) = std::fs::read_dir(path) else { continue };
        let mut inside: Vec<PathBuf> = dir
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| Kind::from_path(path).is_some())
            .collect();
        // Sorted, so a folder stages in the order it reads on screen rather than
        // in whatever order the filesystem hands it back.
        inside.sort();
        found.extend(inside);
    }
    found
}

/// A drop being read, on another thread.
///
/// Reading one document is fast. Reading a folder of them is fast several
/// hundred times over, and the thread that draws is the one thing here that must
/// not be busy - so this has the same shape as every other worker in this
/// application: start it, poll it, take what it says.
pub struct Reading {
    result: Receiver<Vec<Staged>>,
    /// How many documents this is reading, so the interface can say so rather
    /// than show nothing while a large folder resolves.
    pub count: usize,
}

impl Reading {
    pub fn start(
        paths: Vec<PathBuf>,
        entries: Manifest,
        to: Destination,
        already: Vec<Registration>,
        ctx: egui::Context,
    ) -> Reading {
        let count = paths.len();
        let (tx, result) = channel();
        std::thread::spawn(move || {
            let _ = tx.send(read(&paths, &entries, &to, &already));
            // Wake the window rather than leaving it to notice. Nothing else is
            // going to happen on this thread.
            ctx.request_repaint();
        });
        Reading { result, count }
    }

    /// What was read, once it has been.
    ///
    /// Answers `Some` exactly once: the rows move into the list, and a reader
    /// that has given them up has nothing left to be asked for.
    pub fn take(&mut self) -> Option<Vec<Staged>> {
        match self.result.try_recv() {
            Ok(staged) => Some(staged),
            Err(TryRecvError::Empty) => None,
            // The worker died without answering. Nothing was staged, which is
            // the truth, and the drop can simply be repeated.
            Err(TryRecvError::Disconnected) => Some(Vec::new()),
        }
    }
}

/// Read every dropped file into a row.
///
/// `already` is what is staged from earlier drops. A second drop has to collide
/// with the first, or two documents claiming one identity would both be staged
/// and the second would silently win when they were written.
///
/// Public so that the pictures in `render` are made by the code that makes the
/// rows, rather than by a second hand-built copy of what a row looks like.
pub fn read(
    paths: &[PathBuf],
    entries: &Manifest,
    to: &Destination,
    already: &[Registration],
) -> Vec<Staged> {
    let mut seen: Vec<Registration> = already.to_vec();
    let mut staged = Vec::new();
    for path in documents_in(paths) {
        let one = read_one(&path, entries, to, &seen);
        if let Some(registration) = one.registration() {
            seen.push(registration.clone());
        }
        staged.push(one);
    }
    staged
}

fn read_one(
    path: &Path,
    entries: &Manifest,
    to: &Destination,
    seen: &[Registration],
) -> Staged {
    let file_name = path
        .file_name()
        .unwrap_or(path.as_os_str())
        .to_string_lossy()
        .into_owned();

    let document = match Document::read(path) {
        Ok(document) => document,
        Err(why) => {
            return Staged { label: file_name, state: State::Rejected { why: rejection(&why) } };
        }
    };
    // A name with a tab in it cannot survive the entry list, and is refused
    // here rather than at the write, where it would be a failure after a press.
    let registration = match Registration::from_document(&document, &file_name) {
        Ok(registration) => registration,
        Err(why) => {
            return Staged { label: file_name, state: State::Rejected { why: why.to_string() } };
        }
    };

    let label = registration.name.clone();
    match objection(&registration, entries, to, seen) {
        Some(why) => Staged { label, state: State::Conflict { registration, why } },
        None => {
            Staged { label, state: State::Ready { registration, document: Box::new(document) } }
        }
    }
}

/// Why this document cannot be registered as it stands, if it cannot.
///
/// Note what is *not* here: an identity already in the entry list. Re-dropping a
/// document that is registered is how an edited one is re-applied, and the list
/// updates that entry rather than growing a second under the same UUID.
fn objection(
    registration: &Registration,
    entries: &Manifest,
    to: &Destination,
    seen: &[Registration],
) -> Option<String> {
    // Bitwig's browser is flat and matches on name, so two entries under one
    // display name are indistinguishable there (identity rule 7.3.4).
    let taken = entries
        .entries()
        .iter()
        .any(|entry| entry.name == registration.name && entry.uuid != registration.uuid);
    if taken {
        return Some(format!("{} is already registered under another identity", registration.name));
    }

    for other in seen {
        if other.uuid == registration.uuid {
            return Some("another dropped document has the same identity".to_owned());
        }
        if other.name == registration.name {
            return Some(format!("another dropped document is also called {}", other.name));
        }
        if other.library_path == registration.library_path {
            return Some("another dropped document would be placed in the same file".to_owned());
        }
    }

    match placement::would_replace(to, registration) {
        Ok(None) => None,
        Ok(Some(path)) => {
            let path = crate::widget::drawn_path(&path);
            Some(format!("{path} already holds a different document"))
        }
        // Whatever stopped the target being read will stop it being written.
        Err(e) => Some(e.to_string()),
    }
}

/// A document error in the words the row shows.
///
/// The library's messages are accurate and written for whoever is reading a log.
/// A row has one line and a user who has just dropped the wrong file, so the
/// cases that happen to people get a sentence and the rest keep the message they
/// arrived with, which is better than a sentence that guesses.
fn rejection(why: &DocumentError) -> String {
    match why {
        DocumentError::UnknownKind(_) => {
            "Not a Bitwig device, modulator or Grid module".to_owned()
        }
        DocumentError::BadMagic => "Not a Bitwig document".to_owned(),
        // A file shorter than the header, or one that runs out part way through.
        // Either way there is no document here to register.
        DocumentError::Truncated { .. } => "Not a complete Bitwig document".to_owned(),
        DocumentError::UnsupportedFormat(_) => {
            "Saved by a newer Bitwig Studio than this build knows".to_owned()
        }
        other => other.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use orng_tools::{OrngHome, Strategy, UserLibrary};

    /// Somewhere to drop files, and an installation only as far as staging looks
    /// at one: it resolves where a document would be placed and reads what is
    /// already there.
    struct Machine {
        _temp: tempfile::TempDir,
        root: PathBuf,
        to: Destination,
    }

    fn machine() -> Machine {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().to_path_buf();
        let to = Destination {
            install: orng_tools::testing::install(&root.join("install")),
            library: UserLibrary::at(&root.join("library")),
            home: OrngHome::at(&root),
            placement: Strategy::Link,
        };
        Machine { _temp: temp, root, to }
    }

    impl Machine {
        /// Write a document to drop, built by the crate that knows the format.
        fn document(&self, at: &str, uuid: &str, name: &str) -> PathBuf {
            let path = self.root.join(at);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            let kind = Kind::from_path(&path).expect("a sample with a document extension");
            let document = orng_tools::testing::document(kind, uuid.parse().unwrap(), name);
            std::fs::write(&path, document.bytes()).unwrap();
            path
        }

        fn stage(&self, paths: &[PathBuf], entries: &Manifest) -> Vec<Staged> {
            read(paths, entries, &self.to, &[])
        }
    }

    const A: &str = "6d2a2f1e-0a4f-4d8e-9a6c-1d2e3f405162";
    const B: &str = "7e3b301f-1b50-4e9f-8b7d-2e3f40516273";

    #[test]
    fn a_document_that_reads_is_staged_with_the_identity_it_carries() {
        let machine = machine();
        let path = machine.document("drop/DISPERSER.bwdevice", A, "DISPERSER");

        let staged = machine.stage(&[path], &Manifest::default());
        assert_eq!(staged.len(), 1);
        assert_eq!(staged[0].status(), "Staged");
        assert_eq!(staged[0].label, "DISPERSER");
        let registration = staged[0].registration().expect("a staged row has one");
        assert_eq!(registration.uuid.to_string(), A);
        assert_eq!(registration.library_path.as_str(), "devices/My Devices/DISPERSER.bwdevice");
        // Keywords come from the document - its name and its browser category -
        // so a user who edits nothing still gets an entry found by typing.
        assert_eq!(registration.keywords, ["disperser", "test"]);
    }

    #[test]
    fn a_file_that_is_not_a_document_is_rejected_in_plain_language() {
        let machine = machine();
        let notes = machine.root.join("notes.txt");
        std::fs::write(&notes, b"not a device").unwrap();
        // Long enough to have a header, which is what something renamed to
        // `.bwdevice` looks like. A file too short to have one is the other
        // case, and gets its own sentence.
        let garbage = machine.root.join("BROKEN.bwdevice");
        std::fs::write(&garbage, vec![b'x'; 4096]).unwrap();
        let stub = machine.root.join("STUB.bwmodulator");
        std::fs::write(&stub, b"BtWg").unwrap();

        let staged = machine.stage(&[notes, garbage, stub], &Manifest::default());
        // The text file is not even offered to the reader: a drop takes the
        // acceptable files out of what it was given.
        assert_eq!(staged.len(), 2, "{staged:?}");
        assert_eq!(staged[0].status(), "Rejected");
        assert_eq!(staged[0].reason(), Some("Not a Bitwig document"));
        assert_eq!(staged[0].label, "BROKEN.bwdevice", "a rejected row is named by its file");
        assert!(staged[0].registration().is_none());
        assert_eq!(staged[1].reason(), Some("Not a complete Bitwig document"));
    }

    /// Bitwig's browser is flat and matches on name, so two entries under one
    /// name are indistinguishable in the place they exist to be found.
    #[test]
    fn a_name_another_identity_already_holds_is_a_conflict() {
        let machine = machine();
        let registered = "#orng-registry 2\n\
            7e3b301f-1b50-4e9f-8b7d-2e3f40516273\tDEVICE\tDISPERSER\t\
            devices/My Devices/DISPERSER.bwdevice\t\t\t\tlocal\n";
        let entries = Manifest::parse(registered).unwrap();
        let path = machine.document("drop/DISPERSER.bwdevice", A, "DISPERSER");

        let staged = machine.stage(&[path], &entries);
        assert_eq!(staged[0].status(), "Conflict");
        assert_eq!(
            staged[0].reason(),
            Some("DISPERSER is already registered under another identity")
        );
    }

    /// The same identity is not a collision. It is how an edited document is
    /// re-applied, and the list updates the entry rather than growing a second.
    #[test]
    fn re_dropping_something_already_registered_is_staged_not_refused() {
        let machine = machine();
        let registered = format!(
            "#orng-registry 2\n\
             {A}\tDEVICE\tDISPERSER\tdevices/My Devices/DISPERSER.bwdevice\t\t\t\tlocal\n"
        );
        let entries = Manifest::parse(&registered).unwrap();
        let path = machine.document("drop/DISPERSER.bwdevice", A, "DISPERSER");

        let staged = machine.stage(&[path], &entries);
        assert_eq!(staged[0].status(), "Staged", "{:?}", staged[0].reason());
    }

    /// Two files in one drop, both claiming one identity. Without this the
    /// second would quietly win when the list was written.
    #[test]
    fn two_dropped_documents_that_claim_one_identity_collide() {
        let machine = machine();
        let first = machine.document("drop/one/SHAPER.bwdevice", A, "SHAPER");
        let second = machine.document("drop/two/SHAPER.bwdevice", A, "SHAPER COPY");

        let staged = machine.stage(&[first, second], &Manifest::default());
        assert_eq!(staged[0].status(), "Staged");
        assert_eq!(staged[1].status(), "Conflict");
        assert_eq!(staged[1].reason(), Some("another dropped document has the same identity"));
    }

    /// Different identities, different names, one file name - so one file. The
    /// library path is derived from the file name, so this is not contrived.
    #[test]
    fn two_dropped_documents_that_would_share_a_file_collide() {
        let machine = machine();
        let first = machine.document("drop/one/SHARED.bwdevice", A, "FIRST");
        let second = machine.document("drop/two/SHARED.bwdevice", B, "SECOND");

        let staged = machine.stage(&[first, second], &Manifest::default());
        assert_eq!(staged[1].status(), "Conflict");
        assert_eq!(
            staged[1].reason(),
            Some("another dropped document would be placed in the same file")
        );
    }

    /// The user library is where Bitwig's own "Save device..." writes, so a file
    /// already at the target is as likely to be their work as an older copy of
    /// this. Said on the row, before anything is pressed.
    #[test]
    fn a_target_holding_somebody_elses_document_is_a_conflict() {
        let machine = machine();
        let occupied = machine.to.library.folder(Kind::Device.user_folder()).join("X.bwdevice");
        std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
        std::fs::copy(machine.document("other/X.bwdevice", B, "THEIRS"), &occupied).unwrap();

        let path = machine.document("drop/X.bwdevice", A, "MINE");
        let staged = machine.stage(&[path], &Manifest::default());
        assert_eq!(staged[0].status(), "Conflict");
        assert!(
            staged[0].reason().expect("a conflict says why").ends_with("a different document"),
            "{:?}",
            staged[0].reason()
        );
    }

    /// A dropped folder is what somebody with a library of their own drops.
    #[test]
    fn a_dropped_folder_is_read_one_level_deep() {
        let machine = machine();
        machine.document("drop/A.bwdevice", A, "A");
        machine.document("drop/B.bwmodulator", B, "B");
        machine.document("drop/deeper/C.bwdevice", A, "C");
        std::fs::write(machine.root.join("drop/notes.txt"), b"ignored").unwrap();

        let found = documents_in(&[machine.root.join("drop")]);
        let names: Vec<String> = found
            .iter()
            .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["A.bwdevice", "B.bwmodulator"], "a drop walked into a subfolder");
    }
}
