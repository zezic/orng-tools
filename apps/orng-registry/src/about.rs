// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What the About screen states: which build of this application is running,
//! and what it made of the installation it found.
//!
//! The screen exists for one exchange. Somebody reports that a Bitwig release is
//! not recognised, and whoever is helping them asks which version of what, on
//! what, against which installation. All four answers are on this screen and the
//! fifth - the whole diagnostics report - is one press away.
//!
//! **Resolved when the screen opens**, for the reason
//! [`Diagnostics`](crate::diagnostics::Diagnostics) is: the report behind `Copy
//! diagnostics` asks the disk nine times, and this screen is redrawn on every
//! mouse move across it.

use crate::diagnostics::Diagnostics;
use crate::session::Session;
use crate::widget::{SEPARATOR, UNREAD};

/// What About says, read when it opened.
#[derive(Debug)]
pub struct About {
    /// The line under the application's name: which build of it this is, what
    /// the binary is called, and what it is running on. One string, because it
    /// is one statement and the design draws it as one.
    pub identity: String,
    /// Which Bitwig was found, and which build of it. [`UNREAD`] where the
    /// installation could be opened and did not state one - which is a real
    /// state and not an error, and the reason the two are read separately from
    /// whether the anchors resolved.
    pub version: String,
    pub build: String,
    /// Whether this build's anchors were located, which is the whole of what
    /// decides whether anything can be done to this installation at all.
    pub resolved: bool,
    /// The report `Copy diagnostics` copies. The same block Settings draws, and
    /// deliberately the same: two places that described this machine differently
    /// would be two places to correct.
    pub report: String,
}

impl About {
    pub fn of(session: &Session) -> About {
        let build = match session {
            Session::Found(found) => found.condition.build.as_ref(),
            _ => None,
        };
        About {
            identity: identity(),
            version: build.map_or_else(|| UNREAD.to_owned(), |b| b.version.to_string()),
            build: build.map_or_else(|| UNREAD.to_owned(), |b| b.short_revision().to_owned()),
            // Reaching `Found` *is* the resolution: `prepare::inspect` locates
            // the registry anchor and a session that failed to is `Unreadable`.
            // So this is not a second opinion about the same question.
            resolved: matches!(session, Session::Found(_)),
            report: Diagnostics::of(session).report,
        }
    }
}

/// The version, the binary's name and the machine, separated by the design's own
/// mark: `0.9.2`, `orng-registry`, `macOS arm64`.
fn identity() -> String {
    format!(
        "{} {SEPARATOR} {} {SEPARATOR} {} {}",
        env!("CARGO_PKG_VERSION"),
        env!("CARGO_PKG_NAME"),
        platform(),
        architecture(),
    )
}

/// What the user calls the system they are on.
///
/// Rust names these for its own target triples and the design names them the way
/// the platforms name themselves, which is what somebody pasting this line into
/// a bug report will be asked for. Only the three this ships for are translated;
/// anything else says what Rust said, because a wrong friendly name is worse than
/// an unfamiliar exact one.
fn platform() -> &'static str {
    match std::env::consts::OS {
        "macos" => "macOS",
        "windows" => "Windows",
        "linux" => "Linux",
        other => other,
    }
}

/// The same, for the machine underneath. `aarch64` is what the compiler calls
/// what every Apple and Windows tool calls `arm64`.
fn architecture() -> &'static str {
    match std::env::consts::ARCH {
        "aarch64" => "arm64",
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The identity line is three fields separated by the design's mark, and it
    /// names this build rather than a build.
    ///
    /// Asserted on its shape rather than on its text: two of the three fields are
    /// a property of the machine the test runs on, which is exactly why no
    /// snapshot of this screen is taken against the running platform.
    #[test]
    fn the_identity_line_names_this_build_on_this_machine() {
        let line = identity();
        let fields: Vec<&str> = line.split(SEPARATOR).map(str::trim).collect();
        assert_eq!(fields.len(), 3, "{line}");
        assert_eq!(fields[0], env!("CARGO_PKG_VERSION"));
        assert_eq!(fields[1], "orng-registry");
        // The platform and the machine, in that order and both said.
        let (platform, machine) = fields[2].split_once(' ').expect("the platform and the machine");
        assert!(!platform.is_empty() && !machine.is_empty(), "{line}");
    }

    /// Both names are the ones the platform prints about itself, and not the
    /// ones the compiler uses for its targets.
    ///
    /// `aarch64` is what rustc calls what `uname -m` and every Apple and
    /// Microsoft tool call `arm64`, and this line is read by somebody who has
    /// never seen a target triple. The assertion is the set this ships for, so
    /// dropping either translation fails here on the machine it was dropped on.
    #[test]
    fn the_machine_is_named_the_way_it_names_itself() {
        assert!(["macOS", "Windows", "Linux"].contains(&platform()), "{}", platform());
        assert!(["arm64", "x86_64"].contains(&architecture()), "{}", architecture());
    }
}
