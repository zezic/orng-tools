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
    Destination, Document, DocumentError, Kind, Manifest, Registration, Uuid, placement,
};

use crate::status::{Collided, Status};

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
    ///
    /// Carries the document for the same reason [`State::Ready`] does, though
    /// it will not be written as it stands: settling the collision means
    /// rewriting the bytes, with a new identity or a new name. No registration,
    /// because a name that cannot be a file has none, and what the row draws -
    /// the kind, the identity, the name - is the document's own.
    Conflict { document: Box<Document>, collision: Collision },
    /// Not something this application can register.
    Rejected { why: String },
}

/// What a readable document collides with, and so what settles it.
///
/// The design tells the two remedies apart on the row - a pencil for a name,
/// a fingerprint for an identity - so this is an enum of causes rather than a
/// sentence: the row and the rename dialog say each one differently, and which
/// remedy a cause takes is a property of the cause.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Collision {
    /// Another dropped document claims the same identity.
    SameIdentity,
    /// The identity is registered here as another kind, whose file the
    /// registration keeps and whose extension would name this document wrongly.
    RegisteredAs(Kind),
    /// An entry of another identity already has this name. Bitwig's browser is
    /// flat and matches on name, so the two would be indistinguishable there
    /// (identity rule 7.3.4).
    NameRegistered,
    /// Another dropped document has this name.
    NameDropped,
    /// Another dropped document would be placed in the same file.
    SameFile,
    /// The file this would be placed in holds a different document. The path
    /// as drawn.
    FileOccupied(String),
    /// The file this would be placed in could not be read, and whatever
    /// stopped that will stop it being written.
    FileUnreadable(String),
    /// The name cannot be a file name, or cannot be written in the list.
    Unplaceable,
}

impl Collision {
    pub fn collided(&self) -> Collided {
        match self {
            Collision::SameIdentity | Collision::RegisteredAs(_) => Collided::Identity,
            // The file is named after the document, so a new name is a new
            // file - and a file that cannot be read may be the one place the
            // name leads, rather than the folder.
            Collision::NameRegistered
            | Collision::NameDropped
            | Collision::SameFile
            | Collision::FileOccupied(_)
            | Collision::FileUnreadable(_)
            | Collision::Unplaceable => Collided::Name,
        }
    }

    /// What the row says beside the document's name.
    ///
    /// `NameRegistered` is the design's own words (`ORNG Registry.dc.html`,
    /// state `renameconflict`); the rest are ours, in its shape.
    fn on_the_row(&self, name: &str, kind: Kind) -> String {
        let kind = kind.label().to_lowercase();
        match self {
            Collision::SameIdentity => "Another dropped document has the same identity".to_owned(),
            Collision::RegisteredAs(registered) => format!(
                "This identity is registered as a {}",
                registered.label().to_lowercase()
            ),
            Collision::NameRegistered => format!("Name already used by a registered {kind}"),
            Collision::NameDropped => format!("Another dropped document is also called {name}"),
            Collision::SameFile => {
                "Another dropped document would be placed in the same file".to_owned()
            }
            Collision::FileOccupied(path) => format!("{path} already holds a different document"),
            Collision::FileUnreadable(why) => why.clone(),
            Collision::Unplaceable => format!("{name} cannot be a file name"),
        }
    }

    /// What the rename dialog says under its field about the name typed into
    /// it. The design draws one of these, `NameRegistered`'s, and the rest are
    /// ours in the same sentence.
    pub fn in_the_dialog(&self, name: &str, kind: Kind) -> String {
        match self {
            Collision::NameRegistered => {
                format!("A registered {} is already called {name}.", kind.label().to_lowercase())
            }
            Collision::NameDropped => format!("Another dropped document is already called {name}."),
            other => format!("{}.", other.on_the_row(name, kind)),
        }
    }
}

impl Staged {
    /// Which of the design's states this row is in.
    pub fn status(&self) -> Status {
        match &self.state {
            State::Ready { .. } => Status::Staged,
            State::Conflict { collision, .. } => Status::Conflict(collision.collided()),
            State::Rejected { .. } => Status::Rejected,
        }
    }

    /// Why this row is not simply staged, when it is not.
    pub fn reason(&self) -> Option<String> {
        match &self.state {
            State::Ready { .. } => None,
            State::Conflict { collision, document } => {
                Some(collision.on_the_row(&self.label, document.kind()))
            }
            State::Rejected { why } => Some(why.clone()),
        }
    }

    /// The document this row read, when it could be read.
    ///
    /// What the row draws its kind and identity from, a conflicting row's too:
    /// it is what the collision is *about*.
    pub fn document(&self) -> Option<&Document> {
        match &self.state {
            State::Ready { document, .. } | State::Conflict { document, .. } => Some(document),
            State::Rejected { .. } => None,
        }
    }

    /// The registration a ready row will be written under. Only a ready row
    /// has one to be written under.
    pub fn registration(&self) -> Option<&Registration> {
        match &self.state {
            State::Ready { registration, .. } => Some(registration),
            State::Conflict { .. } | State::Rejected { .. } => None,
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
        already: Vec<Claim>,
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
    already: &[Claim],
) -> Vec<Staged> {
    let mut seen: Vec<Claim> = already.to_vec();
    let mut staged = Vec::new();
    for path in documents_in(paths) {
        let one = read_one(&path, entries, to, &seen);
        seen.extend(one.claim());
        staged.push(one);
    }
    staged
}

fn read_one(path: &Path, entries: &Manifest, to: &Destination, seen: &[Claim]) -> Staged {
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
    settle(document, entries, to, seen)
}

/// What one document in hand comes to, against everything else there is.
///
/// Split out of [`read_one`] because a row is settled twice: once when the file
/// is read, and again whenever something it was measured against changes -
/// which is [`resettle`]'s job and is the whole reason a repair on one row is
/// visible on another.
///
/// An identity's collisions are asked about before a name's. Both would be the
/// row's to settle, and a new name cannot settle the first.
fn settle(document: Document, entries: &Manifest, to: &Destination, seen: &[Claim]) -> Staged {
    let label = document.identity().name.clone();
    let settled = identity_collision(&document, entries, seen)
        .map_or_else(|| placed(&document, entries, to, seen), Err);
    let document = Box::new(document);
    let state = match settled {
        Ok(registration) => State::Ready { registration, document },
        Err(collision) => State::Conflict { document, collision },
    };
    Staged { label, state }
}

/// Whether the document's identity is already spoken for.
fn identity_collision(
    document: &Document,
    entries: &Manifest,
    seen: &[Claim],
) -> Option<Collision> {
    let uuid = document.identity().uuid;
    if let Some(entry) = entries.get(uuid)
        && entry.kind() != document.kind()
    {
        return Some(Collision::RegisteredAs(entry.kind()));
    }
    seen.iter().any(|other| other.uuid == uuid).then_some(Collision::SameIdentity)
}

/// The registration the document would be placed under, or what about its name
/// stops it.
///
/// Note what is *not* here: an identity already in the entry list. Re-dropping a
/// document that is registered is how an edited one is re-applied, and the list
/// updates that entry rather than growing a second under the same UUID.
///
/// The one statement of what a name has to be, which is why the rename dialog
/// asks it too: a name the dialog let through would otherwise be a conflict on
/// the row it was typed to settle.
fn placed(
    document: &Document,
    entries: &Manifest,
    to: &Destination,
    seen: &[Claim],
) -> Result<Registration, Collision> {
    // A name with a tab in it cannot survive the entry list, and one with a
    // slash cannot be a file. Said here rather than at the write, where it
    // would be a failure after a press.
    let mut registration =
        Registration::from_document(document).map_err(|_| Collision::Unplaceable)?;
    // An identity already registered keeps the file it has. The document may
    // have been renamed since, and following the name would leave the old file
    // behind under the same identity - two files Bitwig takes for one device.
    if let Some(entry) = entries.get(registration.uuid) {
        registration.library_path = entry.library_path.clone();
    }

    let taken = entries
        .entries()
        .iter()
        .any(|entry| entry.name == registration.name && entry.uuid != registration.uuid);
    if taken {
        return Err(Collision::NameRegistered);
    }
    let file = Claim::file_of(&registration.library_path);
    for other in seen {
        if other.name == registration.name {
            return Err(Collision::NameDropped);
        }
        if other.file.as_ref() == Some(&file) {
            return Err(Collision::SameFile);
        }
    }
    match placement::would_replace(to, &registration) {
        Ok(None) => Ok(registration),
        Ok(Some(path)) => Err(Collision::FileOccupied(crate::widget::drawn_path(&path))),
        // Whatever stopped the target being read will stop it being written.
        Err(e) => Err(Collision::FileUnreadable(e.to_string())),
    }
}

/// What a row already holds, which a document settled after it collides with.
///
/// The identity and the name of every row that was read, and the file only of
/// one that will be written: a conflicting row places nothing, so its file is
/// not taken until it is settled - at which point [`resettle`] reads the rows
/// after it again.
#[derive(Debug, Clone)]
pub struct Claim {
    uuid: Uuid,
    name: String,
    /// Without regard to case, which is how two of the three platforms compare
    /// file names.
    file: Option<String>,
}

impl Claim {
    fn file_of(path: &orng_tools::LibraryPath) -> String {
        path.as_str().to_lowercase()
    }
}

impl Staged {
    /// What this row holds against the rows read after it.
    pub fn claim(&self) -> Option<Claim> {
        let document = self.document()?;
        let file = match &self.state {
            State::Ready { registration, .. } => Some(Claim::file_of(&registration.library_path)),
            _ => None,
        };
        Some(Claim {
            uuid: document.identity().uuid,
            name: document.identity().name.clone(),
            file,
        })
    }
}

/// Read every staged row again, from the documents already in hand, putting
/// each through `each` on the way.
///
/// A row's state is a statement about the whole set and not about that row:
/// two documents claiming one identity are both a conflict, so settling one of
/// them settles the other. A repair applied to the row it was pressed on would
/// leave the other still saying it collides with something that has gone.
///
/// So every edit to the set goes through here, and none of them edits a row in
/// place. Between rewriting a document and reading it again there is a moment
/// where the row's registration describes the bytes as they were, and that
/// moment is inside this loop rather than expressible anywhere else.
///
/// Rejected rows pass through untouched. Nothing was read, so there is no
/// document to read again and nothing about the rest of the set can change what
/// such a row says.
fn resettle(
    rows: Vec<Staged>,
    entries: &Manifest,
    to: &Destination,
    mut each: impl FnMut(usize, Document) -> Document,
) -> Vec<Staged> {
    let mut seen: Vec<Claim> = Vec::new();
    let mut settled = Vec::with_capacity(rows.len());
    for (at, row) in rows.into_iter().enumerate() {
        let row = match row.state {
            State::Rejected { .. } => row,
            State::Ready { document, .. } | State::Conflict { document, .. } => {
                settle(each(at, *document), entries, to, &seen)
            }
        };
        seen.extend(row.claim());
        settled.push(row);
    }
    settled
}

/// Read the set again, unchanged.
///
/// What every edit that only *removes* rows has to end with. A row conflicts
/// because of what else is there, so cancelling one is what settles the row it
/// was colliding with - and a row still saying it collides with a document that
/// has gone is the kind of stale claim this design keeps finding.
pub fn restaged(rows: Vec<Staged>, entries: &Manifest, to: &Destination) -> Vec<Staged> {
    resettle(rows, entries, to, |_, document| document)
}

/// Give the staged row at `at` a fresh identity, and read the set again.
///
/// By position and not by identity, which is the whole point of the control:
/// the collision it settles is two dropped documents claiming one UUID, so a
/// UUID here names both of them and minting one for each would settle nothing.
/// The order is stable through [`resettle`], so a position taken off the drawn
/// list still names the same row.
///
/// The one repair the design offers from the list itself, and it is offered
/// only on rows nothing has been written for. Rewriting a UUID rewrites the
/// document, and doing that to something already registered would orphan every
/// project that refers to it.
///
/// A new identity settles the collisions of identities and none of the rest: a
/// name already taken stays taken and the row goes on saying so. Which is why
/// revision 8 offers it on a staged row and on a conflict of identities, and
/// the pencil on a conflict of names in its place.
pub fn reassign(
    rows: Vec<Staged>,
    at: usize,
    entries: &Manifest,
    to: &Destination,
) -> Vec<Staged> {
    resettle(rows, entries, to, |which, document| {
        if which != at {
            return document;
        }
        // A UUID is fixed width in both of Bitwig's forms, so this is a splice
        // into bytes that were parsed a moment ago, at offsets parsing
        // recorded. Failing means the reader and the writer disagree about this
        // format, which is a bug here rather than a condition on the machine.
        document
            .with_uuid(Uuid::new_v4())
            .expect("a document that parsed can be rewritten at the offsets parsing found")
    })
}

/// Give the staged row at `at` a new name, and read the set again.
///
/// By position, as [`reassign`] is and for its reason. The name is written into
/// the document, because that is the name Bitwig shows: the one in the entry
/// list only finds search keywords. The file it will be placed in follows,
/// being named after the document.
///
/// Only a name [`refusal`] has let through: the dialog asks it before its press
/// is live, so a name this cannot write is a press the dialog should not have
/// allowed.
pub fn rename(
    rows: Vec<Staged>,
    at: usize,
    name: &str,
    entries: &Manifest,
    to: &Destination,
) -> Vec<Staged> {
    resettle(rows, entries, to, |which, document| {
        if which != at {
            return document;
        }
        document.with_name(name).expect("a name the rename dialog let through")
    })
}

/// What stops the staged row at `at` being called `name`, if anything does.
///
/// Measured against every other row and not only those before it, as reading
/// the set does: a name taken from a row further down would settle this one by
/// putting that one in conflict, which is a rename that makes a conflict.
pub fn refusal(
    rows: &[Staged],
    at: usize,
    name: &str,
    entries: &Manifest,
    to: &Destination,
) -> Option<Collision> {
    let document = rows[at].document().expect("a rename is offered only on a row that was read");
    let Ok(renamed) = document.with_name(name) else {
        return Some(Collision::Unplaceable);
    };
    let others: Vec<Claim> = rows
        .iter()
        .enumerate()
        .filter(|(which, _)| *which != at)
        .filter_map(|(_, row)| row.claim())
        .collect();
    placed(&renamed, entries, to, &others).err()
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
        assert_eq!(staged[0].status(), Status::Staged);
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
        assert_eq!(staged[0].status(), Status::Rejected);
        assert_eq!(staged[0].reason().as_deref(), Some("Not a Bitwig document"));
        assert_eq!(staged[0].label, "BROKEN.bwdevice", "a rejected row is named by its file");
        assert!(staged[0].registration().is_none());
        assert_eq!(staged[1].reason().as_deref(), Some("Not a complete Bitwig document"));
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
        assert_eq!(staged[0].status(), Status::Conflict(Collided::Name));
        assert_eq!(staged[0].reason().as_deref(), Some("Name already used by a registered device"));
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
        assert_eq!(staged[0].status(), Status::Staged, "{:?}", staged[0].reason());
    }

    /// Two files in one drop, both claiming one identity. Without this the
    /// second would quietly win when the list was written.
    #[test]
    fn two_dropped_documents_that_claim_one_identity_collide() {
        let machine = machine();
        let first = machine.document("drop/one/SHAPER.bwdevice", A, "SHAPER");
        let second = machine.document("drop/two/SHAPER.bwdevice", A, "SHAPER COPY");

        let staged = machine.stage(&[first, second], &Manifest::default());
        assert_eq!(staged[0].status(), Status::Staged);
        assert_eq!(staged[1].status(), Status::Conflict(Collided::Identity));
        assert_eq!(
            staged[1].reason().as_deref(),
            Some("Another dropped document has the same identity")
        );
    }

    /// Different identities, different names, one file - on the two platforms
    /// whose file names do not tell case apart, which is where people drop.
    #[test]
    fn two_dropped_documents_that_would_share_a_file_collide() {
        let machine = machine();
        let first = machine.document("drop/one/SHARED.bwdevice", A, "SHARED");
        let second = machine.document("drop/two/Shared.bwdevice", B, "Shared");

        let staged = machine.stage(&[first, second], &Manifest::default());
        assert_eq!(staged[1].status(), Status::Conflict(Collided::Name));
        assert_eq!(
            staged[1].reason().as_deref(),
            Some("Another dropped document would be placed in the same file")
        );
    }

    /// The user library is where Bitwig's own "Save device..." writes, so a file
    /// already at the target is as likely to be their work as an older copy of
    /// this. Said on the row, before anything is pressed.
    #[test]
    fn a_target_holding_somebody_elses_document_is_a_conflict() {
        let machine = machine();
        let occupied = machine.to.library.folder(Kind::Device.user_folder()).join("MINE.bwdevice");
        std::fs::create_dir_all(occupied.parent().unwrap()).unwrap();
        std::fs::copy(machine.document("other/X.bwdevice", B, "THEIRS"), &occupied).unwrap();

        let path = machine.document("drop/X.bwdevice", A, "MINE");
        let staged = machine.stage(&[path], &Manifest::default());
        assert_eq!(staged[0].status(), Status::Conflict(Collided::Name));
        assert!(
            staged[0].reason().expect("a conflict says why").ends_with("a different document"),
            "{:?}",
            staged[0].reason()
        );
    }

    /// The browser lists a device by its file name and the device's header by
    /// the name inside it, so a document is placed under the second whatever
    /// it arrived as.
    #[test]
    fn a_document_is_placed_under_its_own_name_and_not_its_files() {
        let machine = machine();
        let path = machine.document("drop/export (3).bwdevice", A, "DISPERSER");

        let staged = machine.stage(&[path], &Manifest::default());
        let registration = staged[0].registration().expect("a staged row has one");
        assert_eq!(registration.library_path.as_str(), "devices/My Devices/DISPERSER.bwdevice");
    }

    /// A registered identity keeps its file, whatever the document is called
    /// now: following the name would leave the old file behind, a second copy
    /// Bitwig takes for the same device.
    #[test]
    fn a_registered_identity_keeps_its_file_and_its_kind() {
        let machine = machine();
        let registered = format!(
            "#orng-registry 2\n\
             {A}\tDEVICE\tDISPERSER\tdevices/My Devices/disperser v1.bwdevice\t\t\t\tlocal\n"
        );
        let entries = Manifest::parse(&registered).unwrap();
        let renamed = machine.document("drop/DISPERSER.bwdevice", A, "DISPERSER MK2");
        let other_kind = machine.document("drop/DISPERSER.bwmodulator", A, "DISPERSER");

        let staged = machine.stage(&[renamed], &entries);
        let registration = staged[0].registration().expect("a staged row has one");
        assert_eq!(staged[0].status(), Status::Staged, "{:?}", staged[0].reason());
        assert_eq!(registration.name, "DISPERSER MK2");
        assert_eq!(registration.library_path.as_str(), "devices/My Devices/disperser v1.bwdevice");

        let staged = machine.stage(&[other_kind], &entries);
        assert_eq!(staged[0].status(), Status::Conflict(Collided::Identity));
        assert_eq!(staged[0].reason().as_deref(), Some("This identity is registered as a device"));
    }

    /// A name no platform could hold as a file is a conflict a rename settles,
    /// not a rejection: the document is fine, and only what it is called is not.
    #[test]
    fn a_name_that_cannot_be_a_file_is_a_conflict_of_names() {
        let machine = machine();
        let path = machine.document("drop/AB.bwdevice", A, "A/B");

        let staged = machine.stage(&[path], &Manifest::default());
        assert_eq!(staged[0].status(), Status::Conflict(Collided::Name));
        assert_eq!(staged[0].reason().as_deref(), Some("A/B cannot be a file name"));
    }

    /// What the rename dialog is told about a name: measured against every
    /// other row, the ones after this one included, and against nothing about
    /// this row itself.
    #[test]
    fn a_rename_is_refused_a_name_any_other_row_has() {
        let machine = machine();
        let first = machine.document("drop/FIRST.bwdevice", A, "FIRST");
        let second = machine.document("drop/SECOND.bwdevice", B, "SECOND");
        let (to, entries) = (&machine.to, &Manifest::default());
        let rows = machine.stage(&[first, second], entries);

        assert_eq!(refusal(&rows, 0, "SECOND", entries, to), Some(Collision::NameDropped));
        assert_eq!(refusal(&rows, 0, "A/B", entries, to), Some(Collision::Unplaceable));
        assert_eq!(refusal(&rows, 0, "FIRST", entries, to), None, "its own name is not taken");
        assert_eq!(refusal(&rows, 0, "THIRD", entries, to), None);

        let renamed = rename(rows, 0, "THIRD", entries, to);
        assert_eq!(renamed[0].label, "THIRD");
        assert_eq!(renamed[1].label, "SECOND", "the other row was renamed");
        assert!(renamed.iter().all(Staged::is_ready));
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
