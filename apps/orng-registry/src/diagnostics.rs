// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What this machine is, in words somebody else can read.
//!
//! The block Settings draws under Diagnostics, and the reason it exists: a build
//! this application does not recognise is reported by the person in front of it
//! to somebody who is not, and every question that person will be asked is
//! answered here in one paste.
//!
//! **Answered once, when the screen opens.** Nine of these lines ask the disk -
//! two file sizes, a directory listing, three bundles and a link - and the screen
//! they are drawn on is redrawn on every mouse move. Resolving them per frame is
//! the mistake `eaf5e47` took out of the inspector, and it is larger here.
//!
//! **The content is this application's and only the shape is the design's.**
//! `docs/design-review.md` round 2 records three paths the bundle gets wrong and
//! this gets right - the entry list is `~/.orng/entries.tsv` and not JSON inside
//! the installation's `Library`, the backups are under `~/.orng` and not in
//! `Application Support`, and the archive is `Contents/Java/bitwig.jar` and not
//! `Contents/Resources`. A wrong path here is repeated by the user to whoever is
//! helping them, which is what makes this the one block worth being pedantic
//! about.

use std::path::Path;

use orng_tools::{Backup, GuardState, Kind};

use crate::session::{Found, Session};
use crate::widget::{SEPARATOR, drawn_path};

/// What Settings says about this machine, resolved when it opened.
///
/// The three paths are here beside the report rather than read off the session
/// where they are drawn, because two of them are only a path once the user's home
/// directory has been taken off the front of them - and because the two states
/// with no installation in them have no paths to read at all. One value that
/// knows what it could and could not answer beats four call sites each deciding
/// again.
#[derive(Debug)]
pub struct Diagnostics {
    /// The report, laid out as one column of labels and one of values.
    pub report: String,
    /// The installation, as the Paths group states it. `None` where there is no
    /// path to state, which is what the two states with no installation behind
    /// them mean; the row still draws, because the group would otherwise be a
    /// different height depending on what went wrong.
    pub install: Option<String>,
    /// Where the user keeps their own content.
    pub library: Option<String>,
    /// Where the backup of this build belongs, whether or not it is there. The
    /// design draws the path either way, because where the copy *would* be is
    /// the thing somebody asking has to know.
    pub backups: Option<String>,
    /// Whether it is there, which decides between offering to restore it and
    /// saying there is none yet.
    pub backup: bool,
}

impl Diagnostics {
    /// Read everything the screen states.
    pub fn of(session: &Session) -> Diagnostics {
        match session {
            Session::Found(found) => of_installation(found),
            // Both of the states worth reporting. The shape is the design's and
            // the facts are what there are, because this is exactly when
            // somebody reaches for this block.
            Session::Unreadable { root, why } => Diagnostics {
                report: report(&[
                    ("install", root.clone()),
                    ("version", "-  (not read)".to_owned()),
                    ("anchors", format!("NOT RESOLVED  {SEPARATOR}  {why}")),
                ]),
                install: Some(root.clone()),
                library: None,
                backups: None,
                backup: false,
            },
            Session::NoInstallation { searched } => Diagnostics {
                report: report(&[
                    ("install", "none selected".to_owned()),
                    ("searched", searched.clone()),
                ]),
                install: None,
                library: None,
                backups: None,
                backup: false,
            },
        }
    }
}

/// The nine-ish lines about an installation that could be read.
///
/// One line the bundle draws is missing, deliberately: `factory`, the count of
/// Bitwig's own devices, modulators and modules. Reading those means parsing a
/// class out of the archive and takes about a second, so it belongs on the worker
/// the factory toggle needs and not on the thread that draws. A line reading
/// `not read` beside eight that were would say something false about why.
fn of_installation(found: &Found) -> Diagnostics {
    let home = found.to.home.root().parent().map(Path::to_path_buf);
    let short = |path: &Path| shortened(path, home.as_deref());

    let version = match &found.condition.build {
        Some(build) => format!("{}  ({})", build.version, build.short_revision()),
        None => "-  (unresolved)".to_owned(),
    };

    let jar = found.to.install.jar();
    // Relative to the installation, as the design draws it: the root is on the
    // line above and repeating it would push the size off a narrow report.
    let inside = jar.strip_prefix(found.to.install.root()).unwrap_or(&jar);
    let archive = match size_of(&jar) {
        Some(bytes) => format!("{}  {}", drawn_path(inside), megabytes(bytes)),
        None => format!("{}  not read", drawn_path(inside)),
    };

    // A backup is named for the build it came from, so a build that does not
    // state one has no backup directory to name - which is also why preparation
    // refuses such an installation rather than guessing at a name it could not
    // find again.
    let backup = found
        .condition
        .build
        .as_ref()
        .map(|build| Backup::location(&found.to.home, build));
    let backups = backup.as_ref().map_or_else(
        // No build, no directory name: a backup that cannot be named for its
        // build is one that cannot be found again, which is why preparation
        // refuses rather than guessing.
        || format!("{}  {SEPARATOR}  this build is not named", short(&found.to.home.backups())),
        |backup| short(backup.directory()),
    );

    let install = short(found.to.install.root());
    let entries = found.entries().entries().len();
    let report = report(&[
        ("install", install.clone()),
        ("version", version),
        ("archive", archive),
        ("anchors", anchors(found)),
        ("guard", guard(found.condition.guard)),
        ("backup", backup.as_ref().map_or_else(no_backup, taken)),
        (
            "entries",
            format!(
                "{}  {SEPARATOR}  {}",
                short(&found.to.home.entries()),
                match entries {
                    0 => "none recorded".to_owned(),
                    1 => "1 entry".to_owned(),
                    many => format!("{many} entries"),
                }
            ),
        ),
        ("placement", placement(found.to.placement)),
    ]);
    Diagnostics {
        report,
        install: Some(install),
        library: Some(short(found.to.library.root())),
        backups: Some(backups),
        backup: backup.is_some_and(|backup| backup.exists()),
    }
}

/// Whether the three places a preparation writes are where they should be.
///
/// The registry anchor is already answered: resolving it is what
/// `prepare::inspect` does, and an installation that reaches [`Found`] is one
/// where it resolved. The other two are directories, and a locale-stripped
/// installation genuinely may not have all three bundles - which is not an error
/// and is worth saying rather than leaving to be discovered by a preparation.
fn anchors(found: &Found) -> String {
    let localization = found.to.install.localization_dir();
    let descriptions = Kind::ALL
        .into_iter()
        .filter(|kind| localization.join(kind.descriptions_bundle()).is_file())
        .count();
    let bundles = match descriptions {
        0 => "descriptions NOT FOUND".to_owned(),
        found if found == Kind::ALL.len() => "descriptions ok".to_owned(),
        // Named by count rather than by which, because the remedy is the same
        // for any of them and the count is what says how much is missing.
        some => format!("descriptions {some} of {}", Kind::ALL.len()),
    };
    let library = if found.to.install.library_dir().is_dir() {
        "library ok"
    } else {
        "library NOT FOUND"
    };
    format!("registry ok  {SEPARATOR}  {bundles}  {SEPARATOR}  {library}")
}

/// What the tamper guard reads, and what follows from it.
///
/// The design's own three lines. A guard this build cannot read is the one state
/// that stops a preparation before it looks at anything else, so it says so here
/// as well as on the button.
fn guard(state: GuardState) -> String {
    match state {
        GuardState::Disarmed => "disarmed".to_owned(),
        GuardState::Armed => format!("armed  {SEPARATOR}  installation not prepared"),
        GuardState::Unknown => format!(
            "unknown  {SEPARATOR}  guard site not recognised  {SEPARATOR}  preparation refuses"
        ),
    }
}

/// When the pristine copy of this build was taken, how big it is, and what is in
/// it.
///
/// **Formatted here, where it is read off the disk**, and in this machine's own
/// zone. A timestamp turned into a date at the point of drawing is turned into a
/// date in the drawing machine's zone, and a render fixture then differs by a day
/// west of Denver.
fn taken(backup: &Backup) -> String {
    let Ok(at) = backup.taken_at() else { return no_backup() };
    let when = day(at, &jiff::tz::TimeZone::system());
    let size =
        directory_size(backup.directory()).map_or_else(|| "size not read".to_owned(), megabytes);
    format!("{when}  {SEPARATOR}  {size}  {SEPARATOR}  jar + description bundles")
}

/// The day an instant falls on, in a given zone.
///
/// The zone is a parameter so that this can be asserted. An instant does not have
/// a day until a zone is chosen, and choosing the wrong one - or choosing at the
/// point of drawing rather than the point of reading - is how the same backup
/// comes to be dated differently on two machines.
fn day(at: std::time::SystemTime, zone: &jiff::tz::TimeZone) -> String {
    when(at, zone, "%d %b %Y")
}

/// An instant written out in a given zone, however the reader of it wants it.
///
/// Here rather than beside each caller so that there is one answer to what a
/// clock nobody can read says. The Restore screen writes the same instant at
/// more length - see [`crate::restore`] - and the failure is the same failure.
pub(crate) fn when(
    at: std::time::SystemTime,
    zone: &jiff::tz::TimeZone,
    format: &str,
) -> String {
    jiff::Timestamp::try_from(at)
        .map(|stamp| stamp.to_zoned(zone.clone()).strftime(format).to_string())
        // A modification time outside the range of a civil calendar is a clock
        // that is wrong, not a backup that is missing.
        .unwrap_or_else(|_| "date not readable".to_owned())
}

/// What to say where there is no pristine copy yet, which is every machine before
/// its first preparation.
fn no_backup() -> String {
    format!("none yet  {SEPARATOR}  written before the installation is prepared")
}

fn placement(strategy: orng_tools::Strategy) -> String {
    match strategy {
        orng_tools::Strategy::Link => "link into user library".to_owned(),
        orng_tools::Strategy::Copy => "copy into the installation".to_owned(),
    }
}

/// The report, as one block of text.
///
/// The labels are padded to a column here rather than written out with their
/// spaces in them. A column padded by hand is a column that stops lining up the
/// first time one of the labels changes length, and this block is read as two
/// columns or it is not read at all.
fn report(lines: &[(&str, String)]) -> String {
    let widest = lines.iter().map(|(label, _)| label.len()).max().unwrap_or(0);
    lines
        .iter()
        .map(|(label, value)| format!("{label:<widest$}{BESIDE_A_LABEL}{value}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Between a label and its value in the report. Two spaces past the widest
/// label, which is the design's own gap: its `install` is four past a
/// seven-letter word and its `placement` is two past a nine-letter one.
const BESIDE_A_LABEL: &str = "  ";

/// A path with the user's home directory written as `~`.
///
/// What the design draws - `~/.orng/entries.tsv`, `~/Documents/Bitwig
/// Studio/Library` - and what this block wants anyway: the report exists to be
/// pasted somewhere other than this machine, and the account name is nobody
/// else's business.
fn shortened(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", drawn_path(rest)),
        None => drawn_path(path),
    }
}

fn size_of(path: &Path) -> Option<u64> {
    std::fs::metadata(path).ok().map(|meta| meta.len())
}

/// Everything in one directory, which is what a backup is: an archive and three
/// description bundles, no deeper.
pub(crate) fn directory_size(dir: &Path) -> Option<u64> {
    let mut total = 0;
    for entry in std::fs::read_dir(dir).ok()? {
        let Ok(entry) = entry else { continue };
        if let Ok(meta) = entry.metadata()
            && meta.is_file()
        {
            total += meta.len();
        }
    }
    Some(total)
}

/// A size as the design states one: megabytes, to a tenth.
///
/// Decimal megabytes rather than binary, because this is a number a user compares
/// against what their file manager tells them and every file manager on all three
/// platforms now shows decimal.
pub(crate) fn megabytes(bytes: u64) -> String {
    format!("{:.1} MB", bytes as f64 / 1_000_000.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every value begins at the same column, whatever its label is called. A
    /// report whose second column wanders is a report nobody reads across, and
    /// the column is computed rather than typed precisely so that adding a line
    /// cannot break the alignment of the others.
    #[test]
    fn every_value_starts_at_one_column() {
        let text = report(&[
            ("install", "/Applications/Bitwig Studio.app".to_owned()),
            ("guard", "disarmed".to_owned()),
            ("placement", "link into user library".to_owned()),
        ]);
        let at: Vec<usize> = text
            .lines()
            .map(|line| line.len() - line.trim_start_matches(char::is_alphabetic).trim_start().len())
            .collect();
        assert_eq!(at, ["placement".len() + BESIDE_A_LABEL.len(); 3]);
    }

    /// The gap is the design's two past the widest label, and not two past each.
    #[test]
    fn a_short_label_is_padded_to_the_widest() {
        let text = report(&[("guard", "disarmed".to_owned()), ("placement", "link".to_owned())]);
        let mut lines = text.lines();
        assert_eq!(lines.next(), Some("guard      disarmed"));
        assert_eq!(lines.next(), Some("placement  link"));
    }

    /// A path under the user's home is written as `~`, and one outside it is left
    /// alone. The report is pasted elsewhere, and the account name is not part of
    /// what is being reported.
    #[test]
    fn the_home_directory_is_not_named() {
        let home = Path::new("/Users/someone");
        assert_eq!(shortened(&home.join(".orng/entries.tsv"), Some(home)), "~/.orng/entries.tsv");
        assert_eq!(
            shortened(Path::new("/Applications/Bitwig Studio.app"), Some(home)),
            "/Applications/Bitwig Studio.app"
        );
        // And with nothing to compare against, a path is a path.
        assert_eq!(shortened(Path::new("/opt/bitwig"), None), "/opt/bitwig");
    }

    /// Decimal megabytes to a tenth, which is what the design writes and what a
    /// file manager on any of the three platforms agrees with.
    #[test]
    fn a_size_is_stated_the_way_the_design_states_one() {
        assert_eq!(megabytes(35_100_000), "35.1 MB");
        assert_eq!(megabytes(34_000_000), "34.0 MB");
    }

    /// The backup date is the design's `14 Sep 2026`, and it is the zone that
    /// decides which day that is.
    ///
    /// The trap `interface-notes.md` names: an instant has no day until a zone is
    /// chosen, so a date turned out at the point of drawing is turned out in the
    /// drawing machine's zone and two machines disagree by one. Asserted on both
    /// sides of a midnight to prove the zone is what this reads, and never left to
    /// a snapshot - no picture here carries a date at all.
    #[test]
    fn a_backup_is_dated_in_the_zone_it_is_read_in() {
        use jiff::tz::TimeZone;
        use std::time::{Duration, UNIX_EPOCH};

        // 2026-09-15 02:30 UTC, which is still 2026-09-14 in Denver.
        let at = UNIX_EPOCH + Duration::from_secs(1_789_439_400);
        assert_eq!(day(at, &TimeZone::UTC), "15 Sep 2026");
        let denver = TimeZone::get("America/Denver").expect("a zone the database has");
        assert_eq!(day(at, &denver), "14 Sep 2026");
    }
}
