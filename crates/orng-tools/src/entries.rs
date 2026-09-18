// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Updating the entry list: every change that is not preparing an installation.
//!
//! Adding, removing or renaming registered content rewrites a text file, copies
//! documents into the user library, and rewrites three properties bundles. No
//! archive work, no backup, no requirement that Bitwig be closed - which is what
//! decision 6.2 buys by keeping the archive out of it, and what would be lost
//! the moment one of these writes reached into it.
//!
//! The pieces this composes already existed separately: [`crate::manifest`] owns
//! the list, [`crate::placement`] owns where a document goes, and
//! [`crate::descriptions`] owns what the browser says about it. What did not
//! exist is the thing that keeps them in step, and keeping them in step is the
//! whole of this module.

use uuid::Uuid;

use crate::{
    Destination, Document, Kind, Manifest, Registration, Result, descriptions, placement,
};

/// A change to the entry list, together with the documents it names.
///
/// One type rather than a manifest plus a list of files to place, because those
/// two have to agree. An entry registered without its document is the `Missing
/// file` state, and a document placed without its entry is a file nobody has
/// been told about; [`Update::add`] does both, so neither half can be forgotten
/// at a call site.
///
/// Consumed by [`Update::apply`], which returns the list as it now stands. A
/// caller therefore cannot go on holding the manifest it started from and write
/// that instead.
#[derive(Debug)]
pub struct Update {
    entries: Manifest,
    /// Documents whose bytes are not yet where their registration resolves.
    place: Vec<(Registration, Document)>,
}

impl Update {
    /// Start from the list as it stands.
    pub fn to(entries: Manifest) -> Update {
        Update { entries, place: Vec::new() }
    }

    /// Register `document` under `registration`, and place the document where
    /// that registration resolves.
    ///
    /// An identity already in the list is updated rather than added a second
    /// time, so re-dropping an edited document does the obvious thing instead of
    /// producing two rows under one UUID.
    pub fn add(&mut self, registration: Registration, document: Document) {
        // A registration describes the document it is placed with. Deriving one
        // from the other is the caller's job, because the description and the
        // keywords may have been edited between the two; that they still match
        // is not negotiable, and a mismatch would place one document under
        // another's identity.
        assert_eq!(
            registration.uuid,
            document.identity().uuid,
            "registration {} does not describe the document being placed with it",
            registration.name
        );
        assert_eq!(
            registration.kind,
            document.kind(),
            "registration {} is a {:?} and its document is a {:?}",
            registration.name,
            registration.kind,
            document.kind()
        );
        self.entries.insert(registration.clone());
        self.place.push((registration, document));
    }

    /// Change what the list says about an identity that is already in it,
    /// without touching the document.
    ///
    /// What [`Update::add`] is for content, this is for words: the description
    /// and the search keywords live in the bundles rather than in the document,
    /// so editing them places nothing and rewrites the three bundles from the
    /// list, exactly as every other change to it does.
    ///
    /// **Refuses an identity the list does not carry.** Inserting one here
    /// would register an entry with no document behind it, which is the
    /// `Missing file` state arrived at by accident; adding content is
    /// [`Update::add`]'s job and it takes the document that proves the entry.
    pub fn revise(&mut self, registration: Registration) {
        assert!(
            self.entries.entries().iter().any(|entry| entry.uuid == registration.uuid),
            "{} is not registered, so there is nothing to revise",
            registration.name
        );
        self.entries.insert(registration);
    }

    /// Take an identity out of the list.
    ///
    /// The document file is left where it is. Whether it goes too is a choice
    /// the user makes on the removal itself, and the default is to keep it.
    pub fn remove(&mut self, uuid: Uuid) {
        self.entries.remove(uuid);
    }

    /// The list as it will be once this is applied.
    pub fn entries(&self) -> &Manifest {
        &self.entries
    }

    /// Carry it out, and answer with the list that is now on disk.
    ///
    /// The order is chosen by what each failure would leave behind:
    ///
    /// 1. **The documents**, because a document nothing points at is inert.
    /// 2. **The description bundles**, because they are inside the installation
    ///    and are therefore the write that can be refused for want of rights.
    /// 3. **The entry list**, because it is what a prepared installation reads
    ///    at startup, so nothing should be registered until everything it needs
    ///    exists. A description key naming an entry that was never registered is
    ///    read by nobody; an entry whose document was never written is the
    ///    `Missing file` state staring back at the user.
    ///
    /// This is not a transaction and does not pretend to be one. A failure part
    /// way leaves files written and nothing registered, and applying again from
    /// the same state finishes the job, because every write here is idempotent.
    pub fn apply(self, to: &Destination) -> Result<Manifest> {
        for (registration, document) in &self.place {
            placement::place(to, registration, document)?;
        }

        // All three bundles, whether or not this update touched that kind. The
        // bundle is rewritten from the whole list rather than appended to, so
        // writing only the kinds that changed is what would leave the
        // description of a removed entry behind for Bitwig to go on reading.
        for kind in Kind::ALL {
            let of_kind: Vec<&Registration> =
                self.entries.entries().iter().filter(|entry| entry.kind == kind).collect();
            descriptions::write_bundle(&to.install, kind, &of_kind)?;
        }

        self.entries.save(&to.home.entries())?;
        Ok(self.entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{OrngHome, Strategy, UserLibrary, testing};

    /// A machine to write to, all of it thrown away afterwards.
    struct Machine {
        _temp: tempfile::TempDir,
        to: Destination,
    }

    fn machine() -> Machine {
        let temp = tempfile::tempdir().unwrap();
        let to = Destination {
            install: testing::install(&temp.path().join("install")),
            library: UserLibrary::at(&temp.path().join("library")),
            home: OrngHome::at(temp.path()),
            placement: Strategy::Link,
        };
        Machine { _temp: temp, to }
    }

    impl Machine {
        fn bundle(&self, kind: Kind) -> String {
            let path = self.to.install.localization_dir().join(kind.descriptions_bundle());
            std::fs::read_to_string(path).unwrap_or_default()
        }

        fn list(&self) -> String {
            std::fs::read_to_string(self.to.home.entries()).unwrap_or_default()
        }

        fn placed(&self, kind: Kind, file_name: &str) -> std::path::PathBuf {
            self.to.library.folder(kind.user_folder()).join(file_name)
        }
    }

    fn staged(kind: Kind, name: &str) -> (Registration, Document) {
        let document = testing::document(kind, Uuid::new_v4(), name);
        let file_name = format!("{name}.{}", kind.extension());
        let registration = Registration::from_document(&document, &file_name).unwrap();
        (registration, document)
    }

    /// The three writes that make one registration real. Any one of them missing
    /// is a state the interface has a name for, and none of them is the
    /// caller's to remember.
    #[test]
    fn registering_places_the_document_lists_it_and_describes_it() {
        let machine = machine();
        let (registration, document) = staged(Kind::Device, "DISPERSER");

        let mut update = Update::to(Manifest::default());
        update.add(registration.clone(), document.clone());
        let entries = update.apply(&machine.to).unwrap();

        assert_eq!(entries.entries(), std::slice::from_ref(&registration));
        assert_eq!(
            std::fs::read(machine.placed(Kind::Device, "DISPERSER.bwdevice")).unwrap(),
            document.bytes(),
            "the document was not placed"
        );
        assert!(machine.list().contains(&registration.uuid.to_string()), "{}", machine.list());
        assert!(
            machine.bundle(Kind::Device).contains("device.disperser.keywords=disperser test"),
            "{}",
            machine.bundle(Kind::Device)
        );
    }

    /// Every bundle is rewritten from the whole list, not appended to. Writing
    /// only the kinds an update touched is what would leave Bitwig reading the
    /// description of an entry that no longer exists.
    #[test]
    fn removing_an_entry_takes_its_description_with_it() {
        let machine = machine();
        let (kept, kept_document) = staged(Kind::Device, "KEPT");
        let (dropped, dropped_document) = staged(Kind::Device, "DROPPED");

        let mut update = Update::to(Manifest::default());
        update.add(kept.clone(), kept_document);
        update.add(dropped.clone(), dropped_document);
        let entries = update.apply(&machine.to).unwrap();
        assert!(machine.bundle(Kind::Device).contains("device.dropped.desc"));

        let mut update = Update::to(entries);
        update.remove(dropped.uuid);
        update.apply(&machine.to).unwrap();

        let bundle = machine.bundle(Kind::Device);
        assert!(!bundle.contains("device.dropped"), "{bundle}");
        assert!(bundle.contains("device.kept.desc"), "{bundle}");
        assert!(!machine.list().contains(&dropped.uuid.to_string()));
        // The entry goes; the file it named stays. Deleting it is a choice made
        // on the removal, and the default is to keep it.
        assert!(machine.placed(Kind::Device, "DROPPED.bwdevice").is_file());
    }

    /// Editing the words is a change to the bundles and to the list, and to
    /// nothing else: the document is not rewritten and not placed again.
    ///
    /// Worth its own test because the bundle is keyed by the entry's *name*,
    /// so a revision that quietly re-derived the registration from the document
    /// would put the new words under the old key and Bitwig would go on reading
    /// the ones that were replaced.
    #[test]
    fn revising_an_entry_rewrites_its_words_and_leaves_its_document_alone() {
        let machine = machine();
        let (registration, document) = staged(Kind::Device, "DISPERSER");

        let mut update = Update::to(Manifest::default());
        update.add(registration.clone(), document.clone());
        let entries = update.apply(&machine.to).unwrap();
        let placed = machine.placed(Kind::Device, "DISPERSER.bwdevice");
        let written = std::fs::metadata(&placed).unwrap().len();

        let edited = Registration {
            description: "Allpass diffusion network".to_owned(),
            keywords: vec!["smear".to_owned(), "allpass".to_owned()],
            ..registration.clone()
        };
        let mut update = Update::to(entries);
        update.revise(edited);
        let entries = update.apply(&machine.to).unwrap();

        let bundle = machine.bundle(Kind::Device);
        assert!(bundle.contains("device.disperser.desc=Allpass diffusion network"), "{bundle}");
        assert!(bundle.contains("device.disperser.keywords=smear allpass"), "{bundle}");
        assert_eq!(entries.entries().len(), 1, "a revision added a second row");
        assert_eq!(std::fs::metadata(&placed).unwrap().len(), written, "the document was rewritten");
        assert_eq!(std::fs::read(&placed).unwrap(), document.bytes());
    }

    /// Revising is for an identity the list already carries. Letting it insert
    /// one would register an entry with no document behind it, which is the
    /// `Missing file` state reached by accident rather than by anything going
    /// wrong.
    #[test]
    #[should_panic(expected = "is not registered")]
    fn revising_something_that_is_not_registered_is_refused() {
        let (registration, _) = staged(Kind::Device, "NEVER ADDED");
        Update::to(Manifest::default()).revise(registration);
    }

    /// The order exists so that a failure leaves a state the user can act on.
    /// The bundles live inside the installation, which is the write that can be
    /// refused for want of rights, and nothing may be registered before it.
    #[test]
    fn a_bundle_that_cannot_be_written_registers_nothing() {
        let machine = machine();
        let (registration, document) = staged(Kind::Device, "DISPERSER");

        // A directory where the properties file goes. Writing to it fails on
        // every platform, which a permission bit does not.
        let blocked = machine
            .to
            .install
            .localization_dir()
            .join(Kind::Device.descriptions_bundle());
        std::fs::create_dir_all(&blocked).unwrap();

        let mut update = Update::to(Manifest::default());
        update.add(registration, document);
        assert!(update.apply(&machine.to).is_err(), "the blocked bundle was not reported");

        assert!(
            machine.list().is_empty(),
            "an entry was registered that Bitwig could not describe: {}",
            machine.list()
        );
        // The document is written by then, and is meant to be: a document
        // nothing points at is inert, and applying again finishes the job.
        assert!(machine.placed(Kind::Device, "DISPERSER.bwdevice").is_file());
    }

    /// Applying the same change twice has to be the same as applying it once,
    /// because that is what makes recovering from the failure above a re-press
    /// rather than a repair.
    #[test]
    fn applying_the_same_change_twice_changes_nothing_the_second_time() {
        let machine = machine();
        let (registration, document) = staged(Kind::Modulator, "SHAPER");

        let mut first = Update::to(Manifest::default());
        first.add(registration.clone(), document.clone());
        first.apply(&machine.to).unwrap();
        let (list, bundle) = (machine.list(), machine.bundle(Kind::Modulator));

        let mut again = Update::to(Manifest::parse(&list).unwrap());
        again.add(registration, document);
        again.apply(&machine.to).unwrap();

        assert_eq!(machine.list(), list);
        assert_eq!(machine.bundle(Kind::Modulator), bundle);
    }

    /// A registration and the document placed under it describe one thing. The
    /// caller assembles them separately, because the description and keywords
    /// may have been edited in between, so this is the only place the pair can
    /// be checked at all.
    #[test]
    #[should_panic(expected = "does not describe the document")]
    fn a_registration_for_another_document_is_refused() {
        let (registration, _) = staged(Kind::Device, "DISPERSER");
        let (_, other) = staged(Kind::Device, "SOMETHING ELSE");
        Update::to(Manifest::default()).add(registration, other);
    }
}
