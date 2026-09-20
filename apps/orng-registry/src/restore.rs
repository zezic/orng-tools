// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What the Restore screen states, and which copy it is pointed at.
//!
//! **Read when the screen opens, not per frame.** Listing the backups walks a
//! directory and asks the disk for a modification time and a size per copy, and
//! this screen is redrawn on every mouse move across it - the same trade
//! [`Diagnostics`](crate::diagnostics::Diagnostics) makes, written down there.
//! Nothing in this application changes what is here except a restore, so
//! re-reading has exactly one trigger.
//!
//! **Every date is turned into words here, where the disk was read.** A
//! timestamp formatted at the point of drawing is formatted in the drawing
//! machine's zone, and two machines then date one copy a day apart. That is also
//! why no snapshot of this screen carries a row: a picture of a date is a picture
//! of whichever runner took it. See `interface-notes.md`.

use std::path::Path;

use orng_tools::{Backup, Installation, Result};

use crate::diagnostics::{directory_size, megabytes, when};
use crate::session::Session;
use crate::widget::SEPARATOR;

/// The backups this machine holds, and which of them is pointed at.
///
/// The choice lives here rather than beside the screen, because it is only a
/// choice while the screen is open and it can only mean one of the rows in this
/// list. Held as an index into that list, which is what makes "chosen" and
/// "there is nothing to choose" the same question.
#[derive(Debug)]
pub struct Backups {
    taken: Vec<Taken>,
    chosen: usize,
}

/// One copy, as the screen states it.
#[derive(Debug)]
pub struct Taken {
    /// What restoring this row acts on. Private: the screen picks a row, and
    /// what that row does is this module's to say.
    backup: Backup,
    /// The day and time it was taken, in this machine's zone.
    pub when: String,
    /// Which build it is of, and what the copy holds.
    pub what: String,
    pub size: String,
}

impl Backups {
    /// Read them, most recent first, with the newest one pointed at.
    ///
    /// The newest rather than none, because this screen exists to put back what
    /// was last replaced and a list with nothing selected would make the first
    /// press of the primary do nothing.
    pub fn of(session: &Session) -> Backups {
        let taken = match session {
            // No installation, no home to look in. Every backup lives under the
            // `~/.orng` a destination resolves, and a machine with no
            // installation has not got one.
            Session::Found(found) => Backup::list(&found.to.home)
                .unwrap_or_default()
                .into_iter()
                .map(Taken::of)
                .collect(),
            _ => Vec::new(),
        };
        Backups { taken, chosen: 0 }
    }

    pub fn taken(&self) -> &[Taken] {
        &self.taken
    }

    /// Whether this row is the one pointed at.
    pub fn is_chosen(&self, at: usize) -> bool {
        at == self.chosen
    }

    pub fn choose(&mut self, at: usize) {
        self.chosen = at;
    }

    /// The copy the primary action would put back, if there is one.
    pub fn chosen(&self) -> Option<&Taken> {
        self.taken.get(self.chosen)
    }

    /// The day of that copy, for the line at the foot saying what pressing will
    /// do. Only the day: the foot names which backup, and the row above it
    /// already carries the time.
    pub fn chosen_day(&self) -> Option<&str> {
        self.chosen().map(Taken::day)
    }

    /// Put the installation back the way the chosen copy found it.
    ///
    /// Answers `None` where there is nothing chosen, which is the same state as
    /// nothing to choose.
    pub fn restore(&self, install: &Installation) -> Option<Result<()>> {
        self.chosen().map(|taken| taken.backup.restore(install))
    }

    /// Where the chosen copy lives.
    ///
    /// The one thing a child process needs in order to put the same copy back:
    /// a backup has no name but its directory, and the child holds that path
    /// against the copies it can see rather than trusting it.
    pub fn chosen_directory(&self) -> Option<&Path> {
        self.chosen().map(|taken| taken.backup.directory())
    }
}

impl Taken {
    fn of(backup: Backup) -> Taken {
        let at = backup.taken_at().ok();
        let size = directory_size(backup.directory())
            .map_or_else(|| SIZE_UNREAD.to_owned(), megabytes);
        Taken {
            when: at.map_or_else(
                || DATE_UNREAD.to_owned(),
                |at| when(at, &jiff::tz::TimeZone::system(), MOMENT),
            ),
            what: format!("Bitwig Studio {} {SEPARATOR} {HOLDS}", backup.taken_from()),
            size,
            backup,
        }
    }

    /// The day out of the full moment, which is what the foot names.
    fn day(&self) -> &str {
        self.when.split_once(',').map_or(self.when.as_str(), |(day, _)| day)
    }
}

/// The design's own `14 September 2026, 18:42`: the month written out, because
/// this is a list somebody reads once to pick from rather than a report column.
const MOMENT: &str = "%d %B %Y, %H:%M";

/// What a copy holds, which is what this project writes and not the
/// installation - see [`orng_tools::backup`].
const HOLDS: &str = "jar + description bundles";

/// A directory that stopped answering between being listed and being read.
const DATE_UNREAD: &str = "date not read";
const SIZE_UNREAD: &str = "size not read";

#[cfg(test)]
mod tests {
    use super::*;

    /// The moment a copy was taken is the design's, and it is the zone that
    /// decides which day it falls on.
    ///
    /// The same trap `diagnostics::day` is asserted against, one format further
    /// out and worth asserting again: this one carries a time as well, so a zone
    /// that was wrong by an hour would be wrong here and invisible there.
    /// Asserted on both sides of a midnight, and never left to a snapshot - no
    /// picture of this screen carries a row at all.
    #[test]
    fn a_copy_is_dated_and_timed_in_the_zone_it_is_read_in() {
        use jiff::tz::TimeZone;
        use std::time::{Duration, UNIX_EPOCH};

        // 2026-09-15 02:30 UTC, which is still 2026-09-14 in Denver.
        let at = UNIX_EPOCH + Duration::from_secs(1_789_439_400);
        assert_eq!(when(at, &TimeZone::UTC, MOMENT), "15 September 2026, 02:30");
        let denver = TimeZone::get("America/Denver").expect("a zone the database has");
        assert_eq!(when(at, &denver, MOMENT), "14 September 2026, 20:30");
    }

    /// The foot names the day and not the moment, so it has to come back out of
    /// the one string that was formatted - and come back out whole, including
    /// the year, which is what a naive split on the first space would lose.
    #[test]
    fn the_foot_names_the_day_out_of_the_moment() {
        let taken = |when: &str| Taken {
            backup: Backup::location(
                &orng_tools::OrngHome::at(std::path::Path::new("target/render-fixtures/day")),
                &orng_tools::BuildId {
                    version: orng_tools::BitwigVersion::parse("6.1").expect("a version"),
                    revision: "ab".repeat(20),
                },
            ),
            when: when.to_owned(),
            what: String::new(),
            size: String::new(),
        };
        assert_eq!(taken("14 September 2026, 18:42").day(), "14 September 2026");
        // And a moment that could not be read has no day to take out of it, so
        // it says what it said rather than an empty string.
        assert_eq!(taken(DATE_UNREAD).day(), DATE_UNREAD);
    }
}
