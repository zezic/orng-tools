// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What the application knows about this machine.
//!
//! Read once on opening and again whenever something has been done that could
//! change it. **Nothing here is remembered across runs**: an installation can be
//! replaced by a Bitwig update, prepared by an older build of this app, or moved
//! entirely, so every answer is taken from disk rather than carried over.
//!
//! What the user *chose* is the other thing, and it does persist - see
//! [`Settings`]. The two meet in one place, here, and only in one direction:
//! the preferences say where to look and which strategy is in force, and
//! everything that follows is read off the machine - bar one set, what this run
//! has written since, which lives and dies with the list it is about: see
//! [`Found::relist`]. A preference is never evidence about what is there. A
//! stored installation root that has stopped resolving falls back to discovery
//! rather than to a stale answer, which is [`Settings::installation`]'s job and
//! not this module's.
//!
//! The shape is deliberate. Everything that only exists when an installation was
//! found lives inside [`Session::Found`], so no screen can ask for an entry list
//! or a build number while there is no installation to have one - the case the
//! interface has an empty state for.

use std::collections::{BTreeMap, BTreeSet};

use orng_tools::{
    Condition, Destination, GuardState, Helper, InstallError, Installation, Manifest, Rights,
    RunState, Standing, UserLibrary, Uuid, prepare, running_state,
};

use crate::settings::Settings;

/// Everything the interface draws from.
#[derive(Debug)]
pub enum Session {
    /// No Bitwig Studio. The interface offers to locate one by hand.
    NoInstallation { searched: String },
    /// An installation was found but could not be read far enough to say
    /// anything true about it: an archive this build does not recognise, or one
    /// it has no permission to open.
    Unreadable { root: String, why: String },
    Found(Box<Found>),
}

/// An installation, and what is true of it right now.
#[derive(Debug)]
pub struct Found {
    /// The installation, the user library, this project's own directory and the
    /// placement strategy - resolved once, here, so that nothing further in can
    /// resolve them again and get a different answer.
    pub to: Destination,
    /// Which Bitwig this is, and what has been done to its archive.
    pub condition: Condition,
    /// Whether Bitwig is running, which preparation needs it not to be.
    pub running: RunState,
    /// Whether this process may write inside the installation.
    ///
    /// Beside [`Found::running`] because it is the same kind of fact and is
    /// wanted at the same moment: both are conditions on a press, both are
    /// about the machine rather than about the list, and both are answered once
    /// when the installation is read rather than per frame. Three probes on a
    /// spun-down volume is the same cost the standing already refuses to pay
    /// per row.
    ///
    /// On Windows this is the ordinary answer for an installation under
    /// `Program Files`, and what it decides is whether a press is carried out
    /// here or handed to a child process holding rights this one does not.
    pub rights: Rights,
    /// Everything this app has registered.
    entries: Manifest,
    /// What the disk says about each registered document, keyed by identity.
    ///
    /// Answering this per row per frame is three blocking syscalls for every
    /// row on screen, taken on the thread that draws, for an answer that had
    /// not changed - the loop `1026139` took out of the inspector, at list
    /// scale. So it is resolved with the list and replaced with the list, and
    /// [`Found::relist`] is the only way to do either.
    standing: BTreeMap<Uuid, Standing>,
    /// Entries this run wrote into the installation since it was read - the
    /// design's `Pending restart`.
    ///
    /// Bitwig reads the entry list when it launches, so a row written while it
    /// is open is a row it is not showing. Nothing here watches for Bitwig
    /// being restarted, and nothing should: the window reads the machine rather
    /// than polling it.
    ///
    /// **The one thing here that is not read off the disk**, and here all the
    /// same, because its lifetime is exactly this list's: a statement about
    /// rows of the list as this run has written it. Reading the installation
    /// again - a new installation chosen, a preparation, `Check again` - starts
    /// a new `Found` with nothing waiting, which is what it has to be: a
    /// preparation needed Bitwig closed, and a list read afresh is one Bitwig
    /// may already be showing. Held in the window beside the session, it had to
    /// be cleared by hand at each of those.
    ///
    /// Identities rather than rows, and never filtered: the rows that ask are
    /// registered ones, so a removal leaving one behind here is a word nobody
    /// draws.
    awaiting_restart: BTreeSet<Uuid>,
}

impl Found {
    /// An installation that has been read, together with what was found where
    /// each of its entries says its document is.
    ///
    /// A constructor rather than a literal because the standing is derived from
    /// the other four and from the disk: it is not something a caller can be
    /// asked for, and a caller who could supply one could supply the wrong one.
    pub fn new(
        to: Destination,
        condition: Condition,
        running: RunState,
        entries: Manifest,
    ) -> Found {
        Found {
            standing: standing_of(&to.install, &entries),
            // Asked here rather than taken, for the reason the standing is:
            // it is derived from the installation and the disk, so a caller
            // who could supply one could supply the wrong one.
            rights: orng_tools::rights(&to.install),
            to,
            condition,
            running,
            entries,
            awaiting_restart: BTreeSet::new(),
        }
    }

    /// Everything this app has registered.
    pub fn entries(&self) -> &Manifest {
        &self.entries
    }

    /// Take the list a run wrote, and answer the disk about it again.
    ///
    /// The one way to replace the list, because a list and what the disk says
    /// about it are one answer: a registration places documents and a removal
    /// can delete them, so a caller that set the entries alone would leave
    /// every row describing the file that used to be there.
    ///
    /// `wrote` is the rows the run wrote, which an open Bitwig is not showing
    /// until it is started again - see [`Found::awaits_restart`].
    pub fn relist(&mut self, entries: Manifest, wrote: impl IntoIterator<Item = Uuid>) {
        self.standing = standing_of(&self.to.install, &entries);
        self.entries = entries;
        self.awaiting_restart.extend(wrote);
    }

    /// Whether this entry was written by this run since the installation was
    /// read, and so is not yet in an open Bitwig's browser.
    pub fn awaits_restart(&self, uuid: Uuid) -> bool {
        self.awaiting_restart.contains(&uuid)
    }

    /// What was found where this entry says its document is.
    ///
    /// Every registered identity is in the map, because it was built from this
    /// list - so an identity that is not registered is the caller asking about
    /// a row that is not there.
    pub fn standing(&self, uuid: Uuid) -> &Standing {
        self.standing
            .get(&uuid)
            .unwrap_or_else(|| panic!("{uuid} is not registered, so nothing was looked at for it"))
    }
}

fn standing_of(install: &Installation, entries: &Manifest) -> BTreeMap<Uuid, Standing> {
    entries
        .entries()
        .iter()
        .map(|entry| (entry.uuid, Standing::of(install, entry)))
        .collect()
}

impl Session {
    /// Read the machine, at whichever installation the preferences point to.
    ///
    /// The stored root first, when there still is one, and discovery otherwise.
    /// A root the user chose is not re-derived every launch: they said where to
    /// look precisely because looking gives the wrong answer on their machine.
    pub fn read(settings: &Settings) -> Session {
        let install = match settings.installation() {
            Some(root) => match Installation::at(root) {
                Ok(install) => install,
                // A folder the user insisted on and that no longer holds an
                // installation. Said against that folder rather than quietly
                // discovering another one, because a window that answered with
                // a different installation than the one on record would be
                // describing the wrong machine. `Reset to auto-detected` in
                // Settings is the way back.
                Err(e) => {
                    return Session::Unreadable {
                        root: crate::widget::drawn_path(root),
                        why: e.to_string(),
                    };
                }
            },
            None => match Installation::discover() {
                Ok(install) => install,
                Err(e) => return Session::NoInstallation { searched: searched_in(&e) },
            },
        };
        Session::at(install, settings)
    }

    /// Read a specific installation, for when the user has just pointed at one.
    pub fn at(install: Installation, settings: &Settings) -> Session {
        let root = crate::widget::drawn_path(install.root());

        // The condition is read before anything else is offered, because every
        // action the interface can present depends on which one this is.
        let condition = match prepare::inspect(&install) {
            Ok(condition) => condition,
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };

        // The library the user chose, or the platform's own. Resolved here and
        // handed over rather than discovered further in, so that one answer is
        // what every write uses.
        let library = match &settings.library {
            Some(root) => UserLibrary::at(root),
            None => match UserLibrary::discover() {
                Ok(library) => library,
                Err(e) => return Session::Unreadable { root, why: e.to_string() },
            },
        };
        let to = match Destination::at(install, library, settings.placement) {
            Ok(to) => to,
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };
        let entries = match Manifest::load(&to.home.entries()) {
            Ok(entries) => entries,
            // A list that does not parse is this app's own state being wrong,
            // not the installation's. Say so against the installation rather
            // than pretending there is nothing registered, which would invite
            // overwriting it.
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };

        let running = running_state(&to.install);
        Session::Found(Box::new(Found::new(to, condition, running, entries)))
    }
}

/// The places that were looked at, without the sentence wrapped around them.
///
/// The interface puts this inside a sentence of its own, so it wants the list
/// and not the library's phrasing - otherwise the two read as one sentence
/// containing two.
fn searched_in(error: &InstallError) -> String {
    match error {
        InstallError::NoInstallation { searched } => searched.clone(),
        other => other.to_string(),
    }
}

/// What the install bar's badge says about the registry.
///
/// Exactly one of these is true at a time, which is why it is an enum and not a
/// string built at the point of drawing: two screens describing one installation
/// differently is the failure this prevents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    /// Nothing registered, and the archive is as Bitwig shipped it.
    Stock,
    /// Registered and verified, with the number of entries.
    Registered(usize),
    /// Entries on record, and a stock archive again. Normal after a Bitwig
    /// update, and not the user's fault.
    NeedsReapply,
    /// The registry structure could not be located inside this archive.
    UnknownBuild,
    /// The archive was changed by something that is not this application.
    ModifiedElsewhere,
}

impl Badge {
    pub fn label(self) -> String {
        match self {
            Badge::Stock => "Stock".to_owned(),
            Badge::Registered(count) => format!("Registered ({count})"),
            Badge::NeedsReapply => "Needs re-apply".to_owned(),
            Badge::UnknownBuild => "Unknown build".to_owned(),
            Badge::ModifiedElsewhere => "Modified elsewhere".to_owned(),
        }
    }
}

impl Found {
    /// Which of the five states this installation is in.
    ///
    /// The guard is asked first. A guard site this build cannot read is the one
    /// condition that makes every other answer a guess, because preparation
    /// refuses before it looks at anything else.
    pub fn badge(&self) -> Badge {
        let registered = self.entries.entries().len();
        match (self.condition.guard, self.condition.helper) {
            (GuardState::Unknown, _) => Badge::UnknownBuild,
            (GuardState::Disarmed, Helper::Present) => Badge::Registered(registered),
            (GuardState::Armed, Helper::Absent) if registered == 0 => Badge::Stock,
            // The entries are fine; the installation is what needs work. Said
            // once, here, rather than once per row.
            (GuardState::Armed, Helper::Absent) => Badge::NeedsReapply,
            // Neither remaining pair is a state this application produces and
            // stops at: a disarmed guard with no helper is somebody else's edit,
            // and a helper behind an armed guard is a preparation that stopped
            // between patching and activating.
            _ => Badge::ModifiedElsewhere,
        }
    }

    /// Which Bitwig this is, for the install bar's title.
    pub fn title(&self) -> String {
        match &self.condition.build {
            Some(build) => format!("Bitwig Studio {}", build.version),
            // A build that does not state its version still has an
            // installation's name. Inventing a number would be worse than
            // leaving the title short.
            None => "Bitwig Studio".to_owned(),
        }
    }

    /// The short build revision, as the design shows it. Empty when this archive
    /// does not say, which the design already draws as an empty slot.
    pub fn revision(&self) -> String {
        match &self.condition.build {
            Some(build) => build.revision.chars().take(SHORT_REVISION).collect(),
            None => String::new(),
        }
    }

    /// The whole revision, for the hover that carries what the bar truncates.
    pub fn revision_in_full(&self) -> String {
        match &self.condition.build {
            Some(build) => format!("Build {}", build.revision),
            None => "This build does not state a revision".to_owned(),
        }
    }
}

/// How much of a forty-character revision is enough to tell two builds apart,
/// and the length the design draws.
const SHORT_REVISION: usize = 8;

#[cfg(test)]
mod tests {
    use super::*;

    fn condition(guard: GuardState, helper: Helper) -> Condition {
        Condition { build: None, helper, guard }
    }

    fn found(guard: GuardState, helper: Helper, entries: &str) -> Found {
        let temp = std::path::Path::new("target/render-fixtures/badge");
        Found::new(
            Destination {
                install: orng_tools::testing::install(&temp.join("Bitwig Studio.app")),
                library: orng_tools::UserLibrary::at(&temp.join("Library")),
                home: orng_tools::OrngHome::at(temp),
                placement: Settings::default().placement,
            },
            condition(guard, helper),
            RunState::Clear,
            Manifest::parse(entries).expect("the sample list parses"),
        )
    }

    const NONE: &str = "#orng-registry 2\n";
    const ONE: &str = "#orng-registry 2\n\
        80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tA\tdevices/My Devices/A.bwdevice\t\t\t\tlocal\n";

    /// The state a user reaches the morning after a Bitwig release, and the one
    /// most likely to be seen by somebody not expecting it.
    #[test]
    fn an_update_that_reset_the_installation_reads_as_needing_re_apply() {
        let found = found(GuardState::Armed, Helper::Absent, ONE);
        assert_eq!(found.badge(), Badge::NeedsReapply);
    }

    /// The same archive with nothing on record is not a problem at all, and must
    /// not be dressed as one.
    #[test]
    fn a_stock_archive_with_nothing_registered_is_stock() {
        assert_eq!(found(GuardState::Armed, Helper::Absent, NONE).badge(), Badge::Stock);
    }

    #[test]
    fn a_prepared_installation_counts_what_it_carries() {
        let found = found(GuardState::Disarmed, Helper::Present, ONE);
        assert_eq!(found.badge(), Badge::Registered(1));
        assert_eq!(found.badge().label(), "Registered (1)");
    }

    /// An unreadable guard outranks everything, because preparation refuses on
    /// it before it looks at anything else - so any other badge would send the
    /// user to press a button that will not work.
    #[test]
    fn a_guard_this_build_cannot_read_outranks_every_other_answer() {
        for helper in [Helper::Present, Helper::Absent] {
            let found = found(GuardState::Unknown, helper, ONE);
            assert_eq!(found.badge(), Badge::UnknownBuild, "{helper:?}");
        }
    }

    /// Half-prepared and disarmed-by-something-else are different accidents with
    /// the same remedy, and neither is a state this application stops at.
    #[test]
    fn an_archive_this_application_did_not_leave_that_way_says_so() {
        assert_eq!(
            found(GuardState::Disarmed, Helper::Absent, NONE).badge(),
            Badge::ModifiedElsewhere
        );
        assert_eq!(
            found(GuardState::Armed, Helper::Present, NONE).badge(),
            Badge::ModifiedElsewhere
        );
    }
}
