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

use orng_tools::TheDocument;

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
            Action::Assign => "Assign new UUID".to_owned(),
            Action::Locate => "Locate file...".to_owned(),
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
}
