// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What a row says about an entry, and what can be done with it in that state.
//!
//! The design gives an entry exactly ten states and gates every row control on
//! which one it is in - `EntryRow.dc.html:121-130`, and the same table again in
//! the inspector. Getting that table wrong is not cosmetic: during design the
//! panel offered `Reveal file` on a missing file, which is the one action that
//! cannot work, while omitting `Locate file`, which is the one that fixes it.
//!
//! So the table is written once, here, rather than at each of the two surfaces
//! that draw it. This module answers *which* controls a state offers;
//! [`crate::widget`] answers what each one looks like, because a colour is the
//! design's and not this module's.
//!
//! **Two tables, because there are two lists.** [`Status`] is an entry on this
//! machine and [`Published`] is an item in the catalog, and they are not the
//! same seven-of-ten: a catalog row can say `Available` and an entry cannot, and
//! an entry can say `Missing file` and a catalog row has no file to miss. The
//! one word they share - `Update available` - is the two lists describing the
//! same fact from either end, and the README is explicit that it has to be one
//! fact: an entry is catalog-sourced exactly when a catalog item of the same
//! identity reads as installed.

use orng_tools::{BitwigVersion, TheDocument};

/// One of the ten words the design has for the state of an entry.
///
/// Every one of them, including the four nothing computes yet - the table is
/// the contract, and a table missing a row reads as a state that offers
/// nothing rather than as a state nobody has got to.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Dropped, not yet applied.
    Staged,
    /// Live in the installation.
    Registered,
    /// Applied; Bitwig must relaunch to see it.
    PendingRestart,
    /// The document is not where the entry says it is.
    MissingFile,
    /// The file on disk differs from the record.
    Changed,
    /// The catalog has a newer version of this identity.
    UpdateAvailable,
    /// Something about this document collides with what is already here.
    Conflict,
    /// Not a Bitwig document at all. It was never read, so it has neither a
    /// kind nor an identity.
    Rejected,
    /// Queued for removal, and still registered until the next apply.
    PendingRemoval,
    /// Bitwig's own entry, which this application only reads.
    Factory,
}

/// One thing a row offers to do to its entry.
///
/// The design's overflow control is deliberately not here: the bundle draws the
/// button and nothing in it says what the menu holds, so there is a control to
/// draw and no menu to put behind it. See `docs/design-review.md` round 3
/// item 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// Mint a new identity for a document that cannot be registered under the
    /// one it carries.
    Assign,
    /// Point a registered entry back at its document.
    Locate,
    /// Take a row out of the removal queue.
    Undo,
    /// Show the document where it lives, in the system's own file manager.
    Reveal,
    /// Queue the entry for removal, or drop a staged row that has not been
    /// written yet.
    Remove,
}

impl Status {
    /// The word the row draws.
    pub fn word(self) -> &'static str {
        match self {
            Status::Staged => "Staged",
            Status::Registered => "Registered",
            Status::PendingRestart => "Pending restart",
            Status::MissingFile => "Missing file",
            Status::Changed => "Changed",
            Status::UpdateAvailable => "Update available",
            Status::Conflict => "Conflict",
            Status::Rejected => "Rejected",
            Status::PendingRemoval => "Pending removal",
            Status::Factory => "Factory",
        }
    }

    /// The controls this state offers, in the order the design lays them out.
    ///
    /// An iterator rather than a collection: the caller is placing them right
    /// to left across a reserved column and never needs them twice, and a row
    /// is drawn every frame. Reversible for the same reason - the design's DOM
    /// order is left to right and the group is laid out from its right edge.
    pub fn actions(self) -> impl DoubleEndedIterator<Item = Action> {
        [Action::Assign, Action::Locate, Action::Undo, Action::Reveal, Action::Remove]
            .into_iter()
            .filter(move |action| self.offers(*action))
    }

    /// Whether this state offers that one control.
    fn offers(self, action: Action) -> bool {
        use Status::*;
        // Bitwig's own entry is read-only. Not one of these applies to it, and
        // the design says so by hiding the whole group rather than by drawing
        // controls that would refuse.
        if self == Factory {
            return false;
        }
        match action {
            // A staged document is the only one whose identity can still be
            // changed, because nothing has been written under it yet.
            Action::Assign => matches!(self, Staged | Conflict),
            Action::Locate => self == MissingFile,
            Action::Undo => self == PendingRemoval,
            // Not on a missing file, which is the point of the table: that is
            // the one state where revealing cannot work. Not on a staged or
            // conflicting one either - the document is still wherever the user
            // dropped it from, and nothing has been placed to reveal.
            Action::Reveal => !matches!(self, Rejected | MissingFile | Staged | Conflict),
            // A rejected row was never read and there is nothing to remove; a
            // row already queued for removal offers the undo instead.
            Action::Remove => !matches!(self, Rejected | PendingRemoval),
        }
    }

    /// Whether the name is struck through, which is how a queued removal reads
    /// as a row that is about to stop existing.
    pub fn struck_through(self) -> bool {
        self == Status::PendingRemoval
    }
}

impl Action {
    /// What the control says it does, on hover.
    ///
    /// The removal names the setting in force rather than the action alone.
    /// Removing is the one thing here that can destroy the user's own work, and
    /// the design made this tooltip the confirmation instead of adding a
    /// dialog, so a fixed wording would promise the opposite of the setting
    /// half the time.
    pub fn label(self, status: Status, document: TheDocument) -> String {
        let separator = crate::widget::SEPARATOR;
        match self {
            // The bundle's own `title` attributes, and none of them carries the
            // ellipsis the *inspector* writes on the same two actions: a
            // trailing "..." says the press opens something, and it is the
            // panel's labelled line that does, not this.
            Action::Assign => "Assign new UUID".to_owned(),
            Action::Locate => "Locate file".to_owned(),
            Action::Undo => "Undo removal".to_owned(),
            Action::Reveal => "Reveal file".to_owned(),
            // Nothing has been written for a staged row, so there is no entry
            // to remove and no file of ours to delete. The design calls that
            // press Cancel, and it says nothing about a document.
            Action::Remove if matches!(status, Status::Staged | Status::Conflict) => {
                "Cancel".to_owned()
            }
            Action::Remove => match document {
                TheDocument::Kept => {
                    format!("Remove entry {separator} the document file is kept")
                }
                TheDocument::Deleted => {
                    format!("Remove entry {separator} the document file is deleted too")
                }
            },
        }
    }

    /// What the inspector writes beside the icon.
    ///
    /// The panel has room for words where the row has only a tooltip, so these
    /// are the labels and not the explanations - `Inspector.dc.html:148-156`.
    ///
    /// **Two of them carry a trailing ellipsis and the row's do not**, and the
    /// difference is a claim rather than a flourish: "..." says the press opens
    /// something, and on this surface those two do. Locating opens a file
    /// picker and assigning opens the question of which identity to mint.
    pub fn in_the_panel(self) -> &'static str {
        match self {
            Action::Assign => "Assign new UUID...",
            Action::Locate => "Locate file...",
            Action::Undo => "Undo removal",
            Action::Reveal => "Reveal file",
            // Not "Remove", because the panel is already about one entry and
            // the word alone would read as removing what is being looked at
            // rather than the registration. What becomes of the document is on
            // the tooltip, which is [`removal_consequence`].
            Action::Remove => "Remove entry",
        }
    }
}

/// One of the seven words the design has for the state of a catalog item.
///
/// `CatalogRow.dc.html:68-79`, the same shape as [`Status`]'s ten: the word, the
/// colour and the one control each state offers, written once for the row and
/// the detail panel that both draw it.
///
/// Not [`Copy`], and deliberately: the one state that names a version carries
/// it, because a row that said `Needs Bitwig` and got the number from somewhere
/// else is a row that can say the wrong number.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Published {
    /// Not installed here, and this installation can load it.
    Available,
    /// Installed, at the version the catalog publishes.
    Installed,
    /// Installed, and the catalog publishes a newer version of this identity.
    UpdateAvailable,
    /// Installed, and some other published item lists this one as replaced. A
    /// new identity rather than a new version, so both stay published and both
    /// can be installed at once.
    Superseded,
    /// This installation is older than the item states it needs. Carries that
    /// version, because it is the whole of what the row says.
    Incompatible(BitwigVersion),
    /// The bytes did not arrive. Ordinary, and offered again.
    DownloadFailed,
    /// The bytes arrived and are not the ones the catalog described. A trust
    /// event, and the one failure here that is never offered again.
    VerificationFailed,
}

/// One thing a catalog row offers to do to its item.
///
/// [`Action`]'s opposite number, and separate from it because none of the five
/// there applies to something that is not registered. The design gives a catalog
/// row at most one of these, which is why the whole column is 92 wide against
/// the entry row's 84 for two.
///
/// **`Update` is not here, and the omission is deliberate.** The design gives
/// `Update available` a press and gives that press a modal to confirm through -
/// Bitwig resolves a device by identity, so replacing the file changes every
/// project that already uses it, and the modal is where the user is told so and
/// shown both versions. Nothing in the bundle draws that modal. A press that
/// quietly overwrote a device under every open project rather than asking is not
/// a smaller version of the design; it is the one thing the design put a dialog
/// in front of. So the row states the fact in the accent and offers nothing,
/// which is the shape the progress dialog's missing `Cancel` already takes. See
/// `docs/design-review.md` round 3 item 3.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Offer {
    /// Fetch it, check it against the digest the index states, register it.
    ///
    /// Only ever a new identity: an item already registered here is never
    /// `Available`, so this cannot reach a document some project is loading.
    Install,
    /// Open the item that has taken this one's place.
    SeeReplacement,
    /// Ask for the bytes again.
    Retry,
    /// The words of what did not verify, for a report. Deliberately not a
    /// retry: see [`Published::VerificationFailed`].
    CopyDetails,
}

impl Published {
    /// The word the row draws.
    ///
    /// A `String` rather than a `&'static str` because one of the seven states
    /// its own version into the sentence, which is the design's own wording -
    /// `CatalogRow.dc.html:73` writes `"Needs Bitwig "` and appends the number.
    pub fn word(&self) -> String {
        match self {
            Published::Available => "Available".to_owned(),
            Published::Installed => "Installed".to_owned(),
            Published::UpdateAvailable => "Update available".to_owned(),
            // Not "Superseded", which is the bundle's key for the state and not
            // the word it draws. The row reads it from the user's side: there is
            // a replacement, and nothing has happened to what they have.
            Published::Superseded => "Replacement available".to_owned(),
            Published::Incompatible(version) => format!("Needs Bitwig {version}"),
            Published::DownloadFailed => "Download failed".to_owned(),
            Published::VerificationFailed => "Verification failed".to_owned(),
        }
    }

    /// The one control this state offers, where it offers one.
    pub fn offer(&self) -> Option<Offer> {
        match self {
            Published::Available => Some(Offer::Install),
            Published::Superseded => Some(Offer::SeeReplacement),
            Published::DownloadFailed => Some(Offer::Retry),
            Published::VerificationFailed => Some(Offer::CopyDetails),
            // Nothing to do to something that is already here, and nothing to
            // offer for a Bitwig that is too old - the row states the version it
            // needs instead, which is the only useful thing to say. `Update
            // available` is the third of these and is the one that has a press
            // in the design and none here: [`Offer`] says why.
            Published::Installed
            | Published::Incompatible(_)
            | Published::UpdateAvailable => None,
        }
    }

    /// Whether the detail panel offers to remove it, which is exactly the three
    /// states that mean it is registered on this machine -
    /// `CatalogDetail.dc.html:134`.
    ///
    /// The same queued removal a Local row offers, and not a second kind: an
    /// installed catalog item *is* a registered entry, so removing it from here
    /// has to be the press the other list already has.
    pub fn installed(&self) -> bool {
        matches!(self, Published::Installed | Published::UpdateAvailable | Published::Superseded)
    }

    /// What the detail panel's primary control says, where it has one.
    ///
    /// Not the row's control: the panel offers nothing for a superseded item,
    /// because the notice above it already carries `See <replacement>` and two
    /// controls for one press is a panel disagreeing with itself.
    /// `CatalogDetail.dc.html:119`.
    pub fn primary(&self) -> Option<Offer> {
        match self {
            Published::Superseded => None,
            other => other.offer(),
        }
    }
}

impl Offer {
    /// What the control says.
    ///
    /// One map and not two, unlike [`Action`]'s pair. The bundle words these the
    /// same on both surfaces except for the update, which wears a trailing
    /// ellipsis in the panel because the press opens the modal that names both
    /// versions - and that press is not offered here at all. If it is ever
    /// built, the second map comes back with it.
    pub fn label(self) -> &'static str {
        match self {
            Offer::Install => "Install",
            Offer::SeeReplacement => "See replacement",
            Offer::Retry => "Retry",
            Offer::CopyDetails => "Copy details",
        }
    }
}

/// What the inspector's `Remove entry` says it will do.
///
/// The same fact the row's tooltip carries, without the action in front of it:
/// the panel writes `Remove entry` beside the icon and the row has no words at
/// all, so only one of the two has to name what was pressed.
///
/// Here beside [`Action::label`] rather than at the panel that draws it, so
/// that both readings of one setting are written in one file. Getting this
/// backwards on one surface and not the other is precisely the defect the
/// design put the wording on the control to avoid.
pub fn removal_consequence(document: TheDocument) -> &'static str {
    match document {
        TheDocument::Kept => "The document file is kept",
        TheDocument::Deleted => "The document file is deleted too",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn offered(status: Status) -> Vec<Action> {
        status.actions().collect()
    }

    /// The ten the design has, so a test can walk them all. Here rather than on
    /// [`Status`] because nothing that draws needs the list - each surface is
    /// handed the one status its row is in.
    const EVERY: [Status; 10] = [
        Status::Staged,
        Status::Registered,
        Status::PendingRestart,
        Status::MissingFile,
        Status::Changed,
        Status::UpdateAvailable,
        Status::Conflict,
        Status::Rejected,
        Status::PendingRemoval,
        Status::Factory,
    ];

    /// The design's table, transcribed from `EntryRow.dc.html:121-130`. Written
    /// out state by state rather than as the rules above re-stated, so that a
    /// rule edited to fit one state has to be answered for in every other.
    #[test]
    fn every_state_offers_what_the_bundle_offers() {
        use Action::*;
        assert_eq!(offered(Status::Staged), [Assign, Remove]);
        assert_eq!(offered(Status::Conflict), [Assign, Remove]);
        assert_eq!(offered(Status::Registered), [Reveal, Remove]);
        assert_eq!(offered(Status::PendingRestart), [Reveal, Remove]);
        assert_eq!(offered(Status::Changed), [Reveal, Remove]);
        assert_eq!(offered(Status::UpdateAvailable), [Reveal, Remove]);
        assert_eq!(offered(Status::MissingFile), [Locate, Remove]);
        assert_eq!(offered(Status::PendingRemoval), [Undo, Reveal]);
        assert_eq!(offered(Status::Rejected), []);
        assert_eq!(offered(Status::Factory), []);
    }

    /// The ten words, against the keys of the bundle's own `STATUS` map -
    /// `EntryRow.dc.html:68-79`. Copy from an external document, and the kind
    /// of copy that is read as a state rather than as a sentence: `Missing
    /// file` is the design's wording and `File missing` is not.
    #[test]
    fn the_words_are_the_designs_ten() {
        let said: Vec<&str> = EVERY.iter().map(|status| status.word()).collect();
        assert_eq!(
            said,
            [
                "Staged",
                "Registered",
                "Pending restart",
                "Missing file",
                "Changed",
                "Update available",
                "Conflict",
                "Rejected",
                "Pending removal",
                "Factory",
            ]
        );
    }

    /// What each control says it does, against the `title` attributes on
    /// `EntryRow.dc.html:46-58`.
    ///
    /// Worth pinning because the inspector writes two of these with a trailing
    /// ellipsis and the row does not, and the difference is a claim: "..." says
    /// the press opens something, and none of these does.
    #[test]
    fn each_control_says_what_the_bundle_says_it_does() {
        let said = |action: Action| action.label(Status::Registered, TheDocument::Kept);
        assert_eq!(said(Action::Assign), "Assign new UUID");
        assert_eq!(said(Action::Locate), "Locate file");
        assert_eq!(said(Action::Undo), "Undo removal");
        assert_eq!(said(Action::Reveal), "Reveal file");
    }

    /// The panel's words, against `Inspector.dc.html:148-156`, and against the
    /// row's for the one thing the two must not agree about.
    ///
    /// A trailing "..." says the press opens something. The panel's locate and
    /// assign do - a file picker and the question of which identity to mint -
    /// and the row's carry no ellipsis because a tooltip is not a press. Three
    /// of the five are the same string on both surfaces, and that is fine;
    /// getting the other two the wrong way round is the claim that matters.
    #[test]
    fn the_panels_words_carry_the_ellipsis_the_rows_do_not() {
        use Action::*;
        assert_eq!(Assign.in_the_panel(), "Assign new UUID...");
        assert_eq!(Locate.in_the_panel(), "Locate file...");
        assert_eq!(Undo.in_the_panel(), "Undo removal");
        assert_eq!(Reveal.in_the_panel(), "Reveal file");
        assert_eq!(Remove.in_the_panel(), "Remove entry");
        for action in [Assign, Locate] {
            let on_a_row = action.label(Status::MissingFile, TheDocument::Kept);
            assert!(!on_a_row.ends_with("..."), "the row wrote {on_a_row:?}");
            assert_eq!(action.in_the_panel().trim_end_matches('.'), on_a_row);
        }
    }

    /// The design's table never puts more than two controls on a row, which is
    /// why the 84 it reserves - and the narrow row's 76 - is never close to
    /// full at 22 apiece.
    #[test]
    fn no_state_offers_more_than_two_controls() {
        for status in EVERY {
            let offered = offered(status);
            assert!(offered.len() <= 2, "{status:?} offers {offered:?}");
        }
    }

    /// The defect the bundle's README names as the one worth checking: the one
    /// state where revealing cannot work must not offer it, and must offer the
    /// action that fixes it instead.
    #[test]
    fn a_missing_file_is_located_and_never_revealed() {
        assert!(offered(Status::MissingFile).contains(&Action::Locate));
        assert!(!offered(Status::MissingFile).contains(&Action::Reveal));
    }

    /// The removal names the setting in force, because the design made this
    /// wording the confirmation rather than adding a dialog.
    #[test]
    fn the_removal_says_what_becomes_of_the_document() {
        let kept = Action::Remove.label(Status::Registered, TheDocument::Kept);
        let deleted = Action::Remove.label(Status::Registered, TheDocument::Deleted);
        assert!(kept.ends_with("the document file is kept"), "{kept}");
        assert!(deleted.ends_with("the document file is deleted too"), "{deleted}");
        // Nothing of ours has been written for a staged row, so the press is
        // not a removal at all and says so under either setting.
        for setting in [TheDocument::Kept, TheDocument::Deleted] {
            assert_eq!(Action::Remove.label(Status::Staged, setting), "Cancel");
            assert_eq!(Action::Remove.label(Status::Conflict, setting), "Cancel");
        }
    }

    /// Both readings of the setting, pinned against the bundle -
    /// `EntryRow.dc.html:123-127` and `Inspector.dc.html:154`. The two surfaces
    /// word it differently and must never disagree about which way round it is,
    /// which is the one thing four literals can get wrong.
    #[test]
    fn the_panel_and_the_row_agree_about_what_removing_does() {
        for setting in [TheDocument::Kept, TheDocument::Deleted] {
            let on_a_row = Action::Remove.label(Status::Registered, setting);
            let in_the_panel = removal_consequence(setting);
            assert!(
                on_a_row.to_lowercase().ends_with(&in_the_panel.to_lowercase()),
                "the row says {on_a_row:?} and the panel says {in_the_panel:?}"
            );
        }
        assert_eq!(removal_consequence(TheDocument::Kept), "The document file is kept");
        assert_eq!(
            removal_consequence(TheDocument::Deleted),
            "The document file is deleted too"
        );
    }

    fn needs(version: &str) -> Published {
        Published::Incompatible(BitwigVersion::parse(version).expect("a version"))
    }

    /// The seven the design has, in the bundle's own order.
    fn every_published() -> [Published; 7] {
        [
            Published::Available,
            Published::Installed,
            Published::UpdateAvailable,
            Published::Superseded,
            needs("6.2"),
            Published::DownloadFailed,
            Published::VerificationFailed,
        ]
    }

    /// The catalog's own table, transcribed from the `STATUS` map at
    /// `CatalogRow.dc.html:68-76`: the word and the control, state by state.
    ///
    /// Two of the seven do not draw the key they are stored under, and both
    /// matter. `Superseded` reads `Replacement available` - quieter, and from
    /// the user's side rather than the publisher's - and `Incompatible` states
    /// the version instead of naming the state at all.
    ///
    /// One row differs from the bundle on purpose: `Update available` draws the
    /// design's word and offers no press, because the design confirms that press
    /// through a modal nothing draws. [`Offer`] carries the reasoning.
    #[test]
    fn every_published_state_says_what_the_bundle_says() {
        let said: Vec<(String, Option<&str>)> = every_published()
            .iter()
            .map(|state| (state.word(), state.offer().map(Offer::label)))
            .collect();
        assert_eq!(
            said,
            [
                ("Available".to_owned(), Some("Install")),
                ("Installed".to_owned(), None),
                ("Update available".to_owned(), None),
                ("Replacement available".to_owned(), Some("See replacement")),
                ("Needs Bitwig 6.2".to_owned(), None),
                ("Download failed".to_owned(), Some("Retry")),
                ("Verification failed".to_owned(), Some("Copy details")),
            ]
        );
    }

    /// The press the design gives `Update available` is not offered, and that is
    /// the claim rather than an oversight.
    ///
    /// The design puts a modal in front of it because replacing a document
    /// changes every project that already loads that identity, and nothing in
    /// the bundle draws that modal. So the word is drawn and the press is not -
    /// and an install can never reach an identity that is already here, which is
    /// what keeps that guarantee true rather than merely intended.
    #[test]
    fn an_update_is_stated_and_never_run_without_the_modal_that_confirms_it() {
        assert_eq!(Published::UpdateAvailable.word(), "Update available");
        assert_eq!(Published::UpdateAvailable.offer(), None);
        assert_eq!(Published::UpdateAvailable.primary(), None);
        // The one press that writes a document, and the only state that offers
        // it is the one that means this machine does not have the item.
        let installs: Vec<String> = every_published()
            .iter()
            .filter(|state| state.offer() == Some(Offer::Install))
            .map(Published::word)
            .collect();
        assert_eq!(installs, ["Available"]);
        assert!(!Published::Available.installed());
    }

    /// The state that names a version names the one it was given.
    ///
    /// Worth pinning on its own because it is the only word here built rather
    /// than written, and the bundle builds it the same way: a fixed phrase and
    /// the item's `requires` after it.
    #[test]
    fn the_incompatible_row_states_the_version_the_item_asks_for() {
        assert_eq!(needs("6.2").word(), "Needs Bitwig 6.2");
        assert_eq!(needs("5.1.9").word(), "Needs Bitwig 5.1.9");
    }

    /// Verification failure is the one thing here that must never be offered
    /// again. The design says so twice - the README's trust-states section and
    /// round 2 of the review - because `Retry` on a hash mismatch teaches people
    /// to press through a refusal.
    #[test]
    fn a_verification_failure_is_never_offered_a_retry() {
        for surface in [Published::offer, Published::primary] {
            assert_eq!(surface(&Published::VerificationFailed), Some(Offer::CopyDetails));
            // The ordinary failure is offered one, and that is the whole
            // distinction between the two states.
            assert_eq!(surface(&Published::DownloadFailed), Some(Offer::Retry));
        }
    }

    /// The panel's footer, against `CatalogDetail.dc.html:119` and `:134`.
    ///
    /// `Remove` appears on exactly the three states that mean the item is
    /// registered here, which is also what the README keys Local provenance on -
    /// so a state added to one of those lists and not the other is two views
    /// disagreeing about what is installed.
    #[test]
    fn the_panel_offers_removal_on_exactly_the_installed_states() {
        let installed: Vec<String> = every_published()
            .iter()
            .filter(|state| state.installed())
            .map(Published::word)
            .collect();
        assert_eq!(installed, ["Installed", "Update available", "Replacement available"]);
    }

    /// The superseded panel offers no primary, because the notice above it
    /// already carries the press that walks to the replacement. The row does
    /// offer one, and that is not a contradiction: a row has no notice.
    #[test]
    fn the_superseded_panel_leaves_the_walk_to_its_notice() {
        assert_eq!(Published::Superseded.offer(), Some(Offer::SeeReplacement));
        assert_eq!(Published::Superseded.primary(), None);
        // Every other state's panel says what its row says.
        for state in every_published().iter().filter(|s| **s != Published::Superseded) {
            assert_eq!(state.primary(), state.offer(), "{state:?}");
        }
    }

    /// A trailing "..." says the press opens something, and not one of the four
    /// offered here does. Installing deliberately does not confirm, so it must
    /// not wear the mark that says it will - which is the only way this surface
    /// can get that wrong now that the press with a modal behind it is not
    /// offered at all.
    #[test]
    fn nothing_offered_here_says_it_will_ask_first() {
        for offer in [Offer::Install, Offer::SeeReplacement, Offer::Retry, Offer::CopyDetails] {
            assert!(!offer.label().ends_with("..."), "{offer:?}");
        }
        assert_eq!(Offer::Install.label(), "Install");
    }
}
