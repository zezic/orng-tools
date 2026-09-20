// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! What the user has chosen, as against what the machine says.
//!
//! [`Session`](crate::session::Session) is the machine: read on opening and
//! re-read whenever anything could have changed it, and remembered across runs
//! not at all. This is the other half, and the division is what each is *about*
//! rather than how expensive it is to answer. Where a document goes, whether
//! removing an entry takes the file with it and which palette to draw in are
//! nobody's facts but the user's, and an application that forgot them every
//! launch would be asking the same question every launch.
//!
//! The two paths are the awkward pair and are here on purpose. An installation
//! can be moved, replaced by a Bitwig update or thrown away, so a remembered
//! root is a claim that may have stopped being true - which is why
//! [`Settings::install`] is checked when it is used rather than when it is read.
//! What is remembered is that the user said *where to look*, and that outlives
//! any particular directory being there.
//!
//! The check is deliberately only that something is still there. Whether what is
//! there is an installation is [`Session::read`](crate::session::Session::read)'s
//! question, and it answers a directory that is not one by naming it rather than
//! by discovering a different one.
//!
//! Written only when something changes, which is the only time there is
//! anything to write.

use std::path::{Path, PathBuf};

use orng_tools::{OrngHome, Strategy};
use serde::{Deserialize, Serialize};

/// Every preference, as `~/.orng/settings.toml` spells it.
///
/// `#[serde(default)]` throughout: a file holding one key is a file the user
/// wrote by hand, and it should mean "this one thing, the rest as they were"
/// rather than fail to parse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The installation the user pointed at. `None` means look for one, which is
    /// what a fresh machine means and what `Reset to auto-detected` returns to.
    pub install: Option<PathBuf>,
    /// The same for their own content. Bitwig's own default is per platform and
    /// is what discovery answers; a library on another volume is a thing people
    /// genuinely do.
    pub library: Option<PathBuf>,
    /// Where a registered document goes. Was a constant until this file existed,
    /// and the design has always drawn it as a choice.
    #[serde(with = "placement")]
    pub placement: Strategy,
    /// Whether removing an entry also deletes the document it names. The design
    /// defaults it off, and so does this: the document is the user's own work
    /// and deleting it cannot be undone from here.
    pub delete_file: bool,
    pub appearance: Appearance,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            install: None,
            library: None,
            // The strategy that survives a Bitwig update: documents stay in the
            // user library and the installation's folders are linked to it, so
            // an update costs one preparation rather than every document.
            placement: Strategy::Link,
            delete_file: false,
            appearance: Appearance::System,
        }
    }
}

impl Settings {
    /// Read them.
    ///
    /// A machine that has never been used has no file, and that is the defaults
    /// rather than an error - the first launch must not report the absence of a
    /// preferences file as a problem with the installation.
    ///
    /// **A file that does not parse is reported and is not replaced.** The run
    /// continues on the defaults, and nothing is written back until the user
    /// changes something, so a file broken by hand can be fixed by hand. That is
    /// a weaker promise than the entry list beside it gets - a list that does not
    /// parse stops the window, because it is a record of work that cannot be
    /// reconstructed and overwriting it would destroy it - and the difference is
    /// the point: this file is five choices.
    fn load(home: &OrngHome) -> Settings {
        let path = home.settings();
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(why) if why.kind() == std::io::ErrorKind::NotFound => return Settings::default(),
            Err(why) => {
                eprintln!("could not read {}: {why}", path.display());
                return Settings::default();
            }
        };
        match toml::from_str(&text) {
            Ok(settings) => settings,
            Err(why) => {
                eprintln!("{} is not a settings file: {why}", path.display());
                Settings::default()
            }
        }
    }

    /// Write them.
    ///
    /// A failure is reported and not raised. Nothing the user can do about a
    /// read-only home directory belongs on the screen they just changed a radio
    /// button on, and the choice is in force for this run either way.
    fn save(&self, home: &OrngHome) {
        let path = home.settings();
        let text = match toml::to_string_pretty(self) {
            Ok(text) => text,
            // Five scalars and two paths. If this cannot be written as TOML the
            // struct above changed into something that cannot be, which is a bug
            // here rather than a condition on the machine.
            Err(why) => unreachable!("the preferences do not serialise: {why}"),
        };
        if let Some(parent) = path.parent()
            && let Err(why) = std::fs::create_dir_all(parent)
        {
            eprintln!("could not create {}: {why}", parent.display());
            return;
        }
        if let Err(why) = std::fs::write(&path, text) {
            eprintln!("could not write {}: {why}", path.display());
        }
    }

    /// The installation to open, given what the user chose and what is there.
    ///
    /// The filter is whether the directory is still there at all, and nothing
    /// more. A root the user pointed at and then deleted or unmounted is `None`,
    /// which is discovery: there is no folder left to describe, so looking again
    /// is the only answer available.
    ///
    /// A directory that *is* there is handed on whether or not it holds an
    /// installation, because the two answers belong apart. Reading the folder is
    /// [`Session::read`](crate::session::Session::read)'s job, and when it finds
    /// nothing it says so against that folder rather than quietly opening
    /// another one - a window that answered with a different installation than
    /// the one on record would be describing the wrong machine.
    pub fn installation(&self) -> Option<&Path> {
        self.install.as_deref().filter(|root| root.is_dir())
    }
}

/// The preferences, and the one place they are written back to.
///
/// The two are one value because they are never usefully apart: a choice that is
/// not written down lasts until the window closes, and the only way to be certain
/// every choice is written is for changing one and writing them to be the same
/// call. They used to be two fields on the window with a `remember` beside them,
/// which meant every arm of the Settings dispatch had to remember to call it -
/// four did, and the fifth arm anybody adds would have compiled, run, and lost
/// the preference on the next launch with nothing to say so.
///
/// So [`Preferences::change`] is the only way to alter them, and it writes.
pub struct Preferences {
    chosen: Settings,
    /// Where to write. **`None` under the tests**, so a render fixture neither
    /// reads nor writes the preferences of whoever ran it.
    home: Option<OrngHome>,
}

impl Preferences {
    /// What was read off the disk, and the place to put it back.
    pub fn read(home: OrngHome) -> Preferences {
        Preferences { chosen: Settings::load(&home), home: Some(home) }
    }

    /// Preferences that are in force for this run and written nowhere.
    ///
    /// What the render fixtures get, and what a window with no reachable home
    /// directory falls back to: the choices still work, they just do not outlive
    /// the run. Nothing the user can do about that belongs on the screen.
    pub fn unwritten(chosen: Settings) -> Preferences {
        Preferences { chosen, home: None }
    }

    /// Read them.
    pub fn chosen(&self) -> &Settings {
        &self.chosen
    }

    /// Change one, and write them down.
    ///
    /// One call because it is one act. On the change rather than on quitting:
    /// each is one line of TOML, and the alternative is a window that loses
    /// whatever was chosen when it is killed.
    pub fn change(&mut self, to: impl FnOnce(&mut Settings)) {
        to(&mut self.chosen);
        if let Some(home) = &self.home {
            self.chosen.save(home);
        }
    }
}

/// Which palette to draw in.
///
/// Three values rather than a boolean, because `System` is not one of the two
/// palettes: it is a statement about where the answer comes from, and it has to
/// survive the desktop changing its mind while the window is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    /// Follow the desktop. The design draws this one selected.
    #[default]
    System,
    Light,
    Dark,
}

impl Appearance {
    /// Every one of them, in the order the design's switch draws them.
    pub const ALL: [Appearance; 3] = [Appearance::System, Appearance::Light, Appearance::Dark];

    pub fn label(self) -> &'static str {
        match self {
            Appearance::System => "System",
            Appearance::Light => "Light",
            Appearance::Dark => "Dark",
        }
    }
}

/// [`Strategy`] as the file spells it.
///
/// A module here rather than a derive on the library's own type. The file format
/// is this application's, `orng-tools` has no other reason to depend on serde,
/// and a preference nothing outside this binary reads should not be able to pull
/// one into a crate that only locates installations and edits documents.
///
/// [`crate::elevate::Job`] spells it the same way, because it is the same
/// choice crossing to a child process that will act on it. One spelling rather
/// than two: a job that read `copy` as `link` would place documents somewhere
/// the window did not say.
pub(crate) mod placement {
    use orng_tools::Strategy;
    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    const LINK: &str = "link";
    const COPY: &str = "copy";

    pub fn serialize<S: Serializer>(value: &Strategy, to: S) -> Result<S::Ok, S::Error> {
        match value {
            Strategy::Link => LINK,
            Strategy::Copy => COPY,
        }
        .serialize(to)
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(from: D) -> Result<Strategy, D::Error> {
        let word = String::deserialize(from)?;
        match word.as_str() {
            LINK => Ok(Strategy::Link),
            COPY => Ok(Strategy::Copy),
            other => Err(serde::de::Error::unknown_variant(other, &[LINK, COPY])),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A round trip through the file, because the file is the interface: a
    /// preference that writes as one word and reads back as another is a
    /// preference that silently resets on the next launch.
    #[test]
    fn every_preference_survives_the_file() {
        let settings = Settings {
            install: Some(PathBuf::from("/Volumes/Audio/Bitwig Studio.app")),
            library: Some(PathBuf::from("/Volumes/Audio/Library")),
            placement: Strategy::Copy,
            delete_file: true,
            appearance: Appearance::Light,
        };
        let text = toml::to_string_pretty(&settings).expect("the preferences serialise");
        let read: Settings = toml::from_str(&text).expect("what was written parses");
        assert_eq!(read, settings);
    }

    /// The default is what a machine that has never been used gets, and it has to
    /// be the design's own default rather than whatever `Default` derives: a
    /// derived `Strategy` would be whichever variant is written first, and
    /// `Copy` is the one a Bitwig update discards.
    #[test]
    fn the_default_is_the_placement_that_survives_an_update() {
        assert_eq!(Settings::default().placement, Strategy::Link);
        assert_eq!(Settings::default().appearance, Appearance::System);
        assert!(!Settings::default().delete_file);
    }

    /// A file holding one key means that key, not "and reset the rest".
    #[test]
    fn a_hand_written_file_need_not_state_every_preference() {
        let read: Settings = toml::from_str("appearance = \"dark\"\n").expect("one key parses");
        assert_eq!(read.appearance, Appearance::Dark);
        assert_eq!(read.placement, Settings::default().placement);
    }

    /// A word the file should not contain is refused rather than silently read
    /// as the default, because a placement read wrongly writes documents
    /// somewhere the user did not ask for.
    #[test]
    fn a_placement_this_application_does_not_have_is_refused() {
        let read: Result<Settings, _> = toml::from_str("placement = \"symlink\"\n");
        assert!(read.is_err(), "an unknown placement parsed");
    }

    /// A remembered root that no longer holds anything is not an installation.
    #[test]
    fn a_stored_path_that_has_gone_falls_back_to_discovery() {
        let settings =
            Settings { install: Some(PathBuf::from("/nonexistent/Bitwig Studio.app")), ..Settings::default() };
        assert!(settings.installation().is_none());
    }

    /// The contract the type exists for: changing a preference writes the file,
    /// with nothing else to remember to call. Mutate `change` to skip the save
    /// and this is what fails.
    #[test]
    fn changing_a_preference_writes_it_down() {
        let temp = tempfile::tempdir().expect("a temporary home");
        let home = OrngHome::at(temp.path());
        assert!(!home.settings().exists(), "nothing is written before a change");

        let mut preferences = Preferences::read(home);
        preferences.change(|chosen| chosen.placement = Strategy::Copy);

        let written = Settings::load(&OrngHome::at(temp.path()));
        assert_eq!(written.placement, Strategy::Copy, "the change did not reach the file");
        assert_eq!(preferences.chosen().placement, Strategy::Copy);
    }

    /// And preferences with nowhere to write still take the change, because the
    /// choice is in force for this run either way. This is what the render
    /// fixtures hold, so a failure here would mean tests writing to whoever ran
    /// them.
    #[test]
    fn preferences_with_no_home_change_without_writing_anywhere() {
        let mut preferences = Preferences::unwritten(Settings::default());
        preferences.change(|chosen| chosen.appearance = Appearance::Light);
        assert_eq!(preferences.chosen().appearance, Appearance::Light);
    }

    /// And a directory that is there is handed on even though it holds no
    /// installation, which is the half the wording used to get wrong. Discovery
    /// must not run here: the session reads this folder, finds nothing and says
    /// so against it, and quietly opening some other installation instead would
    /// describe a machine the user never pointed at.
    #[test]
    fn a_stored_path_that_is_there_is_handed_on_unexamined() {
        let root = std::env::temp_dir();
        assert!(!root.join("bitwig.jar").exists(), "the temp dir is not an installation");
        let settings = Settings { install: Some(root.clone()), ..Settings::default() };
        assert_eq!(settings.installation(), Some(root.as_path()));
    }
}
