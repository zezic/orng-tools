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
    Backup, BuildId, Condition, Destination, GuardState, Helper, InstallError, Installation,
    Manifest, OrngHome, Rights, RunState, Standing, TakenFrom, UserLibrary, Uuid, prepare,
    running_state,
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
    /// What was prepared from this home before, if anything was. What tells
    /// an installation a Bitwig update reset apart from one nobody prepared,
    /// which look the same from the archive.
    history: Option<Prepared>,
}

/// What this home has prepared before, as its backups say: a preparation takes
/// one before it writes anything, and nothing else does.
///
/// Of any installation, not only this one, because a backup is named for the
/// build it came from and a Bitwig update is a new build. So a second
/// installation never prepared, beside one that was, reads as reset. The badge
/// asks for the same press either way.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Prepared {
    /// This build. A stock archive of it was put back without an update: a
    /// backup restored over it, or Bitwig installed over itself.
    ThisBuild,
    /// Other builds and not this one, the newest of them named. A stock
    /// archive of this build is what a Bitwig update leaves.
    Another(TakenFrom),
}

impl Prepared {
    /// Read off the backups in `home`, for an installation of `build`.
    fn of(home: &OrngHome, build: Option<&BuildId>) -> Option<Prepared> {
        // A home whose backups cannot be listed has none this can point to.
        let backups = Backup::list(home).unwrap_or_default();
        if build.is_some_and(|build| backups.iter().any(|b| b.taken_from().is_of(build))) {
            return Some(Prepared::ThisBuild);
        }
        // The list is newest first.
        backups.first().map(|newest| Prepared::Another(newest.taken_from().clone()))
    }
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
            history: Prepared::of(&to.home, condition.build.as_ref()),
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
    /// Entries on record, and an archive nothing from this home has prepared.
    /// Not `Needs re-apply`, which says it was applied once: the entries were
    /// written without a preparation, or for another installation.
    NotPrepared,
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
            Badge::NotPrepared => "Not prepared".to_owned(),
            Badge::UnknownBuild => "Unknown build".to_owned(),
            Badge::ModifiedElsewhere => "Modified elsewhere".to_owned(),
        }
    }
}

impl Found {
    /// Which of the six states this installation is in.
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
            (GuardState::Armed, Helper::Absent) => match self.history {
                Some(_) => Badge::NeedsReapply,
                None => Badge::NotPrepared,
            },
            // Neither remaining pair is a state this application produces and
            // stops at: a disarmed guard with no helper is somebody else's edit,
            // and a helper behind an armed guard is a preparation that stopped
            // between patching and activating.
            _ => Badge::ModifiedElsewhere,
        }
    }

    /// What this installation was prepared as before something put its archive
    /// back as Bitwig shipped it: only where the badge says `Needs re-apply`.
    pub fn reset(&self) -> Option<&Prepared> {
        self.history.as_ref().filter(|_| self.badge() == Badge::NeedsReapply)
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
        found_in("badge", guard, helper, entries)
    }

    /// In a home of its own, for a test that writes into it: the others share
    /// one and run beside each other.
    fn found_in(name: &str, guard: GuardState, helper: Helper, entries: &str) -> Found {
        let temp = std::path::Path::new("target/render-fixtures").join(name);
        Found::new(
            Destination {
                install: orng_tools::testing::install(&temp.join("Bitwig Studio.app")),
                library: orng_tools::UserLibrary::at(&temp.join("Library")),
                home: orng_tools::OrngHome::at(&temp),
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
    /// most likely to be seen by somebody not expecting it. The backup is what
    /// says it was prepared before: the archive alone looks the same as one
    /// nobody prepared.
    #[test]
    fn an_update_that_reset_the_installation_reads_as_needing_re_apply() {
        let backup = std::path::Path::new("target/render-fixtures/badge-reset/.orng/backups")
            .join("6.0-a1d34f07");
        std::fs::create_dir_all(&backup).expect("a place to keep a backup");
        std::fs::write(backup.join("bitwig.jar"), b"not an archive, and not read here")
            .expect("could not write the copy");
        let found = found_in("badge-reset", GuardState::Armed, Helper::Absent, ONE);
        assert_eq!(found.badge(), Badge::NeedsReapply);
        assert_eq!(found.badge().label(), "Needs re-apply");
    }

    /// What was prepared before is only said as a reset where the installation
    /// needs re-applying: a prepared one keeps its backups and lost nothing.
    #[test]
    fn a_backup_says_nothing_was_reset_where_the_installation_is_prepared() {
        let backup = std::path::Path::new("target/render-fixtures/reset-prepared/.orng/backups")
            .join("6.0-a1d34f07");
        std::fs::create_dir_all(&backup).expect("a place to keep a backup");
        std::fs::write(backup.join("bitwig.jar"), b"not an archive, and not read here")
            .expect("could not write the copy");
        let prepared = found_in("reset-prepared", GuardState::Disarmed, Helper::Present, ONE);
        assert_eq!(prepared.reset(), None);
        // A build that does not state itself cannot be the one backed up.
        let reset = found_in("reset-prepared", GuardState::Armed, Helper::Absent, ONE);
        let named = |prepared: &Prepared| match prepared {
            Prepared::Another(last) => last.to_string(),
            Prepared::ThisBuild => "this build".to_owned(),
        };
        assert_eq!(reset.reset().map(named).as_deref(), Some("6.0 (a1d34f07)"));
    }

    /// Entries with no preparation behind them - installed before anything was
    /// prepared, or kept for another installation - were never applied, so
    /// they are not re-applied either.
    #[test]
    fn entries_nothing_ever_prepared_read_as_not_prepared() {
        let found = found_in("badge-never", GuardState::Armed, Helper::Absent, ONE);
        assert_eq!(found.badge(), Badge::NotPrepared);
        assert_eq!(found.badge().label(), "Not prepared");
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
