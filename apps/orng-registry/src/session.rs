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
//! everything that follows is read off the machine. A preference is never
//! evidence about what is there. A stored installation root that has stopped
//! resolving falls back to discovery rather than to a stale answer, which is
//! [`Settings::installation`]'s job and not this module's.
//!
//! The shape is deliberate. Everything that only exists when an installation was
//! found lives inside [`Session::Found`], so no screen can ask for an entry list
//! or a build number while there is no installation to have one - the case the
//! interface has an empty state for.

use orng_tools::{
    Condition, Destination, GuardState, Helper, InstallError, Installation, Manifest, RunState,
    UserLibrary, prepare, running_state,
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
    /// Everything this app has registered.
    pub entries: Manifest,
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
                        root: root.display().to_string(),
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
        let root = install.root().display().to_string();

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

        Session::Found(Box::new(Found {
            running: running_state(&to.install),
            condition,
            to,
            entries,
        }))
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
        Found {
            to: Destination {
                install: orng_tools::testing::install(&temp.join("Bitwig Studio.app")),
                library: orng_tools::UserLibrary::at(&temp.join("Library")),
                home: orng_tools::OrngHome::at(temp),
                placement: Settings::default().placement,
            },
            condition: condition(guard, helper),
            running: RunState::Clear,
            entries: Manifest::parse(entries).expect("the sample list parses"),
        }
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
