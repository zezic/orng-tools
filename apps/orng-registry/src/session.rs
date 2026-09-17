// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What the application knows about this machine.
//!
//! Read once on opening and again whenever something has been done that could
//! change it. Nothing here is remembered across runs: an installation can be
//! replaced by a Bitwig update, prepared by an older build of this app, or moved
//! entirely, so every answer is taken from disk rather than from a setting.
//!
//! The shape is deliberate. Everything that only exists when an installation was
//! found lives inside [`Session::Found`], so no screen can ask for an entry list
//! or a build number while there is no installation to have one - the case the
//! interface has an empty state for.

use orng_tools::{Condition, Installation, Manifest, OrngHome, RunState, prepare, running_state};

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
    pub install: Installation,
    /// Which Bitwig this is, and what has been done to its archive.
    pub condition: Condition,
    /// Whether Bitwig is running, which preparation needs it not to be.
    pub running: RunState,
    /// Everything this app has registered.
    pub entries: Manifest,
}

impl Session {
    /// Read the machine.
    pub fn read() -> Session {
        let install = match Installation::discover() {
            Ok(install) => install,
            Err(e) => return Session::NoInstallation { searched: e.to_string() },
        };
        Session::at(install)
    }

    /// Read a specific installation, for when the user has pointed at one.
    pub fn at(install: Installation) -> Session {
        let root = install.root().display().to_string();

        // The condition is read before anything else is offered, because every
        // action the interface can present depends on which one this is.
        let condition = match prepare::inspect(&install) {
            Ok(condition) => condition,
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };

        let home = match OrngHome::discover() {
            Ok(home) => home,
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };
        let entries = match Manifest::load(&home.entries()) {
            Ok(entries) => entries,
            // A list that does not parse is this app's own state being wrong,
            // not the installation's. Say so against the installation rather
            // than pretending there is nothing registered, which would invite
            // overwriting it.
            Err(e) => return Session::Unreadable { root, why: e.to_string() },
        };

        Session::Found(Box::new(Found {
            running: running_state(&install),
            condition,
            install,
            entries,
        }))
    }
}

impl Found {
    /// What the status line says about the installation, in the design's words.
    pub fn guard_summary(&self) -> &'static str {
        use orng_tools::{GuardState, Helper};
        match (self.condition.helper, self.condition.guard) {
            (_, GuardState::Unknown) => "guard site not recognised, preparation refuses",
            (Helper::Present, GuardState::Disarmed) => "prepared",
            (Helper::Absent, GuardState::Armed) => "guard armed, installation not prepared",
            // Neither of the remaining two is a state this app produces and
            // stops at, so neither gets a comfortable word for it.
            (Helper::Present, GuardState::Armed) => "partly prepared, the guard is still armed",
            (Helper::Absent, GuardState::Disarmed) => "the guard was disarmed by something else",
        }
    }
}
