// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The window.
//!
//! Three fixed regions, top to bottom, as the specification has them: the
//! install bar saying which Bitwig this is and what state it is in, the entry
//! list, and the action bar carrying the one primary action. The two top-level
//! views change the middle region and nothing else, which is what keeps the app
//! one app rather than two sharing a title bar.
//!
//! Draws from a [`Session`] and never from its own memory of what it drew last
//! time. Anything it wants to know about the machine it asks the session for,
//! and the session is re-read after anything that could change the answer.
//!
//! The one thing it does hold of its own is what has been dropped and not yet
//! written, and what the toolbar is filtering by. Both are pending work rather
//! than facts about the machine, so neither can be re-read from anywhere.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use eframe::egui::{self, Align, Layout, vec2};
use orng_catalog::Index;
use orng_tools::{
    Backup, Content, Document, Kind, Placement, Provenance, Registration, RunState, Step,
    Rights, Strategy, TheDocument, Uuid, placement,
};

use crate::about::About;
use crate::catalog::{self, Catalog, Freshness, Install};
use crate::diagnostics::{self, Diagnostics};
use crate::elevate::{self, Job};
use crate::restore::Backups;
use crate::session::{Badge, Found, Session};
use crate::settings::{Appearance, Preferences, Settings};
use crate::staging::{self, Reading, Staged};
use crate::status::{Action, Offer, Published, Status, removal_consequence};
use crate::theme::{self, Palette, font, metric};
use crate::widget::{self, Emphasis, Fact, Measure, Padding, Tone, icon};
use crate::work::{Applying, Errand, Stage, Work};

/// Which top-level view is showing. Two, as the design has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// What is registered on this machine.
    Local,
    /// What ORNG Catalog publishes.
    Catalog,
}

impl View {
    /// What to call it on the control that goes back to it.
    fn label(self) -> &'static str {
        match self {
            View::Local => "Local",
            View::Catalog => "Catalog",
        }
    }
}

/// Which surface the window is showing.
///
/// The surfaces behind the overflow are not panels over the list: each is
/// `width:100%; height:100%` on the page colour with a header of its own, so each
/// replaces the install bar and the action bar as well as the page.
/// [`App::browse`] is the seam they swap in at.
///
/// **Each screen carries what it states**, resolved when it opens rather than
/// per frame. All three ask the disk to answer themselves - Settings for two
/// file sizes, three bundles, a listing and a link; Restore for a directory walk
/// and a time and a size per copy; About for the same report Settings draws -
/// and a screen is redrawn on every mouse move across it. That is the fault
/// `eaf5e47` took out of the inspector, three times over. What was read is
/// re-read where this application has changed one of the answers and nowhere
/// else: see [`App::settle_screen`].
#[derive(Debug)]
enum Screen {
    Browsing,
    Settings(Diagnostics),
    /// Restore, holding the copies it found and which one is pointed at.
    Restore(Backups),
    About(About),
}

/// What Settings, or the overflow menu, was pressed for.
///
/// One value carried out of the drawing rather than each control acting where it
/// sits, because every one of these writes to something the surface is drawn
/// from - a path, a placement, the palette, the screen itself - and that surface
/// is still being drawn while the press is being noticed.
///
/// The overflow shares it rather than having one of its own, because what it
/// offers is what Settings offers: three of its four items are also a control on
/// that screen, and two enums would be two places to keep them agreeing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chose {
    Nothing,
    /// Out of the screen, back to the view it was opened from.
    Back,
    /// Point at one of the two paths by hand.
    Locate(Which),
    /// Give one of them back to discovery, which is `Reset to auto-detected`.
    Rediscover(Which),
    Placement(Strategy),
    /// Whether removing an entry takes the document with it. Named for the
    /// proposition it carries, so that `true` means what the word says: this was
    /// `Keep`, where `Keep(true)` meant delete.
    DeleteFile(bool),
    Appearance(Appearance),
    CopyReport,
    /// On to a screen: from the overflow, or from one of Settings' own two
    /// controls that lead to one - `Restore...` beside the backups path, and
    /// the row at the foot.
    Settings,
    Restore,
    About,
    /// Show the backups directory in the system's own file manager. The one
    /// overflow item that is not a screen, which is why it is here rather than
    /// being a fourth way of saying `Screen`.
    RevealBackups,
}

/// What the Restore screen was pressed for.
///
/// Collected and acted on after the drawing, for the reason [`Chose`] is: the
/// screen is drawn from the list it is holding, and two of these replace that
/// list or the screen itself.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Restoring {
    Nothing,
    Back,
    /// Show the backups directory in the system's own file manager.
    Reveal,
    /// Point at one of the copies.
    Choose(usize),
    /// Put the chosen one back, which is the one thing this screen is for.
    Restore,
}

/// And what the About screen was pressed for. Two things, and an enum anyway:
/// going back writes the field the screen is being drawn out of.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Asked {
    Nothing,
    Back,
    CopyReport,
}

/// Which of the two paths a press is about.
///
/// The two rows offer the same pair of controls and differ only in what they are
/// pointing at, so the alternative was four variants above that all did the same
/// two things.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Which {
    Install,
    Library,
}

/// How much of the catalog the install filter is letting through -
/// `CatalogToolbar.dc.html:34-36`.
///
/// Three states and not a checkbox, which is the design's own decision: the
/// question a returning user asks is "what have I got that has moved on", and a
/// two-state control cannot ask it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum Shown {
    #[default]
    All,
    Installed,
    Updatable,
}

impl Shown {
    /// Whether a published item in this state is one of the ones being shown.
    ///
    /// Read off [`Published`] rather than kept beside it, because the catalog's
    /// own statuses are what the design words these three with: `Installed` is
    /// exactly the three states the detail panel offers to remove, which is the
    /// README's one rule that the two views must agree about what is installed.
    fn lets_through(self, status: &Published) -> bool {
        match self {
            Shown::All => true,
            Shown::Installed => status.installed(),
            Shown::Updatable => *status == Published::UpdateAvailable,
        }
    }
}

/// What the list toolbar is showing of the list.
///
/// A filter is not a property of the entries, so it does not live in the
/// session: re-reading the machine must not silently clear what the user typed.
///
/// **One filter and not one per view**, which is the shell's own arrangement:
/// `ORNG Registry.dc.html:101` and `:111` hand the same `query` and the same
/// `kinds` to both toolbars, and only the install filter is the catalog's alone.
/// Switching views keeps what was asked for rather than quietly widening it.
#[derive(Debug)]
struct Filter {
    /// On the Local list, matched against the display name and against the
    /// identity - what is this thing I have, and which row is the one this UUID
    /// names. The catalog answers a different question and so searches
    /// different fields; see [`Filter::accepts_published`].
    query: String,
    /// Empty means none, not all: the toolbar shows every kind switched on and
    /// switching all three off is a thing a user can do and undo.
    kinds: BTreeSet<Kind>,
    /// Read by the Catalog view alone. Nothing on the Local list is published,
    /// so there is nothing there for it to say.
    shown: Shown,
}

impl Default for Filter {
    fn default() -> Self {
        Filter {
            query: String::new(),
            kinds: Kind::ALL.into_iter().collect(),
            shown: Shown::default(),
        }
    }
}

impl Filter {
    fn accepts(&self, entry: &Registration) -> bool {
        if !self.kinds.contains(&entry.kind()) {
            return false;
        }
        let query = self.query.trim().to_lowercase();
        query.is_empty()
            || entry.name.to_lowercase().contains(&query)
            || entry.uuid.to_string().contains(&query)
    }

    /// The same three questions asked of a published item.
    ///
    /// **The search covers four fields where Local's covers two**, and the
    /// design says so in the placeholder: browsing is looking for a thing that
    /// does X, so the description and the keywords are where the answer is. The
    /// UUID is deliberately not among them - it names a thing you already have,
    /// and a catalog row does not even draw one.
    fn accepts_published(&self, item: &orng_catalog::IndexEntry, status: &Published) -> bool {
        if !self.kinds.contains(&item.kind.into()) || !self.shown.lets_through(status) {
            return false;
        }
        let query = self.query.trim().to_lowercase();
        query.is_empty()
            || item.name.to_lowercase().contains(&query)
            || item.author.to_string().to_lowercase().contains(&query)
            || item.description.to_lowercase().contains(&query)
            || item.keywords.iter().any(|word| word.to_lowercase().contains(&query))
    }
}

/// What the Local list has to draw: the pending work, then what is registered,
/// both after the filter.
///
/// Worked out before anything is drawn rather than in among the drawing,
/// because which of the view's three surfaces to show is decided by what is
/// left - and because the two halves have to be worked out *together*. A
/// dropped document that is already registered is one piece of pending work and
/// gets one row, so the registered half is only the entries the pending half is
/// not already speaking for.
struct Listing<'a> {
    /// The staged rows the filter let through, each carrying where it sits in
    /// the unfiltered list.
    ///
    /// **Numbered before the filter**, so a row knows where it is in the
    /// pending list rather than where it is on screen. A control pressed on the
    /// third row of a filtered list acts on the third row of the list the
    /// filter was applied to, which is not the same row.
    pending: Vec<(usize, &'a Staged)>,
    /// The registered entries it let through, less the ones a staged row is
    /// already the row for.
    registered: Vec<&'a Registration>,
}

impl<'a> Listing<'a> {
    fn of(staged: &'a [Staged], entries: &'a [Registration], filter: &Filter) -> Self {
        // Membership and nothing else is asked of this, so a set rather than
        // the list it is drawn from: the alternative walks the pending rows
        // once per registered entry.
        let spoken_for: BTreeSet<Uuid> =
            staged.iter().filter_map(Staged::registration).map(|row| row.uuid).collect();
        Listing {
            pending: staged
                .iter()
                .enumerate()
                .filter(|(_, row)| row.registration().is_none_or(|row| filter.accepts(row)))
                .collect(),
            registered: entries
                .iter()
                .filter(|entry| !spoken_for.contains(&entry.uuid))
                .filter(|entry| filter.accepts(entry))
                .collect(),
        }
    }

    /// Whether the filter left nothing, which is the state that draws
    /// `No entries match` rather than a list.
    fn is_empty(&self) -> bool {
        self.pending.is_empty() && self.registered.is_empty()
    }
}

pub struct App {
    session: Session,
    /// What the user chose, which outlives the run where the session does not.
    /// The one thing here written to disk, and written only when it changes -
    /// which is [`Preferences`]' own guarantee rather than this module's.
    preferences: Preferences,
    view: View,
    /// Which surface is showing. The two views are what this changes the middle
    /// of; a screen replaces all of it.
    screen: Screen,
    palette: Palette,
    /// Which of the two palettes is in force, as against which was asked for:
    /// [`Appearance::System`] is a question rather than a palette, and the desktop
    /// can answer it differently while the window is open.
    dark: bool,
    filter: Filter,
    /// Documents dropped and not yet written. The pending work.
    staged: Vec<Staged>,
    /// Entries the user has asked to be rid of, which are still registered
    /// until the next apply - the design's `Pending removal`, and the other
    /// half of the pending work.
    ///
    /// Held beside [`App::staged`] rather than in it: a staged row is a
    /// document waiting to be written and this is an identity waiting to be
    /// forgotten, and the row it draws is the registered one struck through
    /// rather than a row of its own. A set because queueing the same entry
    /// twice is queueing it once, and ordered so that a press writes the
    /// removals in the same order every time.
    removing: BTreeSet<Uuid>,
    /// Entries written into a live installation since the list was last read
    /// off the machine - the design's `Pending restart`.
    ///
    /// Bitwig reads the entry list when it launches, so a row written while it
    /// is open is a row it is not showing. Nothing here watches for Bitwig
    /// being restarted, and nothing should: the window reads the machine rather
    /// than polling it, so this is emptied when the list is read again and by
    /// the preparation, which is the apply that starts from the archive.
    ///
    /// Identities rather than rows, and never filtered: [`App::status_of`] is
    /// the only reader and it asks about registered entries, so a removal
    /// leaving one behind here is a word nobody draws.
    awaiting_restart: BTreeSet<Uuid>,
    /// A drop being read, off the interface thread.
    reading: Option<Reading>,
    /// Set while work is in flight, and only while it is in flight: the moment
    /// it reports, what it did becomes an [`Outcome`] and the work is over.
    applying: Option<Applying>,
    /// The plan, while it is up: from the press of `Prepare installation` on
    /// the bar until the dialog's own press or its `Cancel`.
    ///
    /// The one press in the window that confirms before it runs, because it is
    /// the one that modifies Bitwig Studio itself - README, section 6. What the
    /// plan says is worked out as it is drawn, from the same rows the press
    /// will write: a drop can still land under the scrim, and a plan copied
    /// out at the press would then describe a press that no longer exists.
    /// What is held is only what the disk has to be asked about.
    confirming: Option<Confirming>,
    /// Words the user has edited for one entry that have not been written yet,
    /// because writing them would ask for rights this process does not hold.
    ///
    /// Apart from every other edit, which is written the moment a field is
    /// left and announced nowhere. Where the installation is not this account's
    /// to write, that same write is carried to a child process and Windows
    /// raises its consent dialog - and a consent dialog that arrives because
    /// the pointer moved out of a text box is one people learn to dismiss
    /// without reading. So the words wait here behind a named `Save`, and
    /// nothing is lost if the answer is `Cancel`: the entry is still what it
    /// was. Never set where [`elevate::can_ask`] is false, because there is no
    /// dialog there to put off.
    ///
    /// The words and not the entry they would make. The question can stand
    /// while a run changes that entry, and `Save` revises it as it is by then
    /// rather than putting back a copy taken before the run.
    asking: Option<Unsaved>,
    /// What the last press came to. Stated as a banner until the user puts it
    /// away, because nothing else will stop being true and take it off screen.
    outcome: Option<Outcome>,
    /// The inspector, if it is open: which entry, and what is in its two
    /// editable fields.
    inspecting: Option<Inspection>,
    /// The catalog item the detail panel is open on. The inspector's opposite
    /// number, and held apart from it: they are one region of the window and
    /// one view at a time, but the answer to "which row did I open" belongs to
    /// the list it was opened in, and switching views and back should not have
    /// forgotten it.
    detailing: Option<Uuid>,
    /// The published catalog: what was kept from last time, and what this run
    /// has made of it.
    ///
    /// **Read on opening and refreshed on opening**, which is two acts and not
    /// one. Reading is two files and a signature, so a window with no network
    /// browses the catalog it browsed yesterday. The refresh behind it is on a
    /// worker and the window never waits for it, which is what the older rule
    /// here - do not reach for a socket before being asked - was actually
    /// protecting: a window that hangs on a train. This one does not hang; it
    /// states the age of what it has, and offers `Refresh`.
    catalog: Catalog,
    /// The item being fetched, while one is.
    ///
    /// Held apart from [`App::applying`] because it is the half of an install
    /// that has written nothing: a fetch that fails leaves the machine exactly
    /// as it was, and the design says so by holding that failure on the row
    /// rather than stopping the window. The write that follows is an ordinary
    /// [`Applying`], the same one a drop goes through.
    installing: Option<Install>,
    /// Items an install attempt refused, until something happens that could
    /// change the answer.
    ///
    /// The design's two failure states are per item and not per window - a
    /// catalog of forty rows where one did not verify is thirty-nine rows that
    /// are still fine - so they are held against the identity that failed. Kept
    /// until that item is pressed again or a refresh brings back a *different*
    /// index, because nothing else that happens makes them stop being true - and
    /// a refresh that confirms the index they were made against does not either.
    refused: std::collections::BTreeMap<Uuid, catalog::Refused>,
    /// What the drop overlay says about the files over the window, while there
    /// are any.
    ///
    /// Kept rather than drawn from the paths each frame, because saying it means
    /// listing every hovered folder, and a drag redraws the window for as long
    /// as it is held. Worked out again only when a different set of paths is
    /// over the window.
    hovered: Option<Hovered>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // The preferences first, because the session is read at whichever
        // installation they point to. Both are read once here; nothing reads
        // either again unless something has happened to change it.
        let home = orng_tools::OrngHome::discover()
            .inspect_err(|why| {
                // A machine this application cannot store anything on. It still
                // runs: the preferences are the defaults for this run, and
                // everything that matters is read off the disk anyway.
                eprintln!("no home directory, so no preferences: {why}");
            })
            .ok();
        let preferences = match home.clone() {
            Some(home) => Preferences::read(home),
            None => Preferences::unwritten(Settings::default()),
        };
        let session = Session::read(preferences.chosen());
        let mut app = App::with(&cc.egui_ctx, session).having(preferences);
        // The kept index first, so the first frame has a catalog, and the check
        // behind it, so what the first frame has is not silently last month's.
        // Both on every launch: the file is under a kilobyte, and the only
        // thing the user ever sees of the check is the age beside the view
        // switch changing.
        app.catalog = Catalog::opened(home);
        app.catalog.refresh(&cc.egui_ctx);
        app
    }

    /// The window over a session that is already known.
    ///
    /// What the tests build, so the interface can be rendered against a state
    /// this machine does not happen to be in.
    pub fn with(ctx: &egui::Context, session: Session) -> Self {
        let palette = Palette::DARK;
        theme::install_fonts(ctx);
        theme::apply(ctx, palette);
        App {
            session,
            preferences: Preferences::unwritten(Settings::default()),
            view: View::Local,
            screen: Screen::Browsing,
            palette,
            dark: true,
            filter: Filter::default(),
            staged: Vec::new(),
            removing: BTreeSet::new(),
            awaiting_restart: BTreeSet::new(),
            reading: None,
            applying: None,
            confirming: None,
            asking: None,
            outcome: None,
            inspecting: None,
            detailing: None,
            // Nothing read and nothing fetched. `App::new` is the one caller
            // that hands this a home directory, so a render fixture cannot
            // reach the cache of whoever ran it, and cannot start a fetch by
            // being put in the Catalog view either.
            catalog: Catalog::opened(None),
            installing: None,
            refused: std::collections::BTreeMap::new(),
            hovered: None,
        }
    }

    /// The same window, carrying the preferences that were read off the disk and
    /// the place to write them back to.
    ///
    /// Separate from [`App::with`] so that the tests, which build a session
    /// themselves, get the defaults and nowhere to write at all. A render fixture
    /// that drew from `~/.orng/settings.toml` would be a picture of whoever ran
    /// it, and one that wrote to it would be worse than that.
    fn having(mut self, preferences: Preferences) -> Self {
        self.preferences = preferences;
        self
    }

    pub fn show_view(&mut self, view: View) {
        self.view = view;
    }

    /// Open Settings without going through the menu. Tests only.
    #[cfg(test)]
    pub fn show_settings(&mut self) {
        self.screen = Screen::Settings(Diagnostics::of(&self.session));
    }

    /// The same for Restore. Tests only.
    #[cfg(test)]
    pub fn show_restore(&mut self) {
        self.screen = Screen::Restore(Backups::of(&self.session));
    }

    /// Which copy the Restore screen is pointed at, named by the build it is of.
    ///
    /// Not the index: what the picture of a choice is is one glyph, and what a
    /// press has to have changed is which archive the primary would copy over
    /// the installation. Tests only.
    #[cfg(test)]
    pub fn pointed_at(&self) -> Option<&str> {
        match &self.screen {
            Screen::Restore(backups) => backups.chosen().map(|taken| taken.what.as_str()),
            _ => None,
        }
    }

    /// And for About, with the identity line supplied rather than read.
    ///
    /// **Supplied, because the real one names the machine.** It ends in
    /// `macOS arm64` here and `Windows x86_64` on the runner, which is two runs
    /// of different width in the one string this screen is built around - so a
    /// picture of the real line is a picture of whoever took it. The same reason
    /// `render::fixture` returns a relative path, and the line itself is
    /// asserted in `about.rs` where it is built. Tests only.
    #[cfg(test)]
    pub fn show_about(&mut self, identity: &str) {
        let mut about = About::of(&self.session);
        about.identity = identity.to_owned();
        self.screen = Screen::About(about);
    }

    /// Put a preference on screen without pressing anything. Tests only, and
    /// unwritten, so a fixture cannot reach the preferences of whoever ran it.
    #[cfg(test)]
    pub fn set_settings(&mut self, settings: Settings) {
        self.preferences = Preferences::unwritten(settings);
    }

    /// Where a document would go, read off the destination every write is handed
    /// rather than off the preference that was chosen - which is the whole of what
    /// there is to check. Tests only.
    #[cfg(test)]
    pub fn placement(&self) -> Option<Strategy> {
        match &self.session {
            Session::Found(found) => Some(found.to.placement),
            _ => None,
        }
    }

    /// Put work on screen without having started any. Tests only.
    #[cfg(test)]
    pub fn set_applying(&mut self, applying: Applying) {
        self.applying = Some(applying);
    }

    /// Whether the plan is up. Tests only.
    #[cfg(test)]
    pub fn is_confirming(&self) -> bool {
        self.confirming.is_some()
    }

    /// The list as this window has it, so a test can hand it back to a run that
    /// is pretending to have written it. Tests only.
    #[cfg(test)]
    pub fn registered(&self) -> Option<&orng_tools::Manifest> {
        match &self.session {
            Session::Found(found) => Some(found.entries()),
            _ => None,
        }
    }

    /// Put a catalog on screen without fetching one. Tests only.
    #[cfg(test)]
    pub fn set_catalog(&mut self, catalog: Catalog) {
        self.catalog = catalog;
    }

    /// Hand the window a fetch that has already answered, so that what it does
    /// with the answer can be driven without a network. Tests only.
    #[cfg(test)]
    pub fn set_installing(&mut self, installing: Install) {
        self.installing = Some(installing);
    }

    /// Whether either worker is still out there.
    ///
    /// What a test waits on, because a worker here is detached on purpose -
    /// the window polls a channel when it draws and never blocks on a thread,
    /// so there is no handle to join and this is the only thing that answers
    /// "has it landed yet". Tests only.
    #[cfg(test)]
    pub fn is_working(&self) -> bool {
        self.applying.is_some() || self.installing.is_some()
    }

    /// Stage rows without a drop. Tests only.
    #[cfg(test)]
    pub fn set_staged(&mut self, staged: Vec<Staged>) {
        self.staged = staged;
    }

    /// Narrow the list without typing. Tests only.
    #[cfg(test)]
    pub fn set_query(&mut self, query: &str) {
        self.filter.query = query.to_owned();
    }

    /// Open the inspector without clicking a row. Tests only.
    #[cfg(test)]
    pub fn set_inspecting(&mut self, uuid: Uuid) {
        let Session::Found(found) = &self.session else {
            panic!("there is no list to inspect a row of")
        };
        let entry = found.entries().get(uuid).expect("the row to inspect is registered");
        self.inspecting = Some(Inspection::of(entry, found));
    }

    /// The same for the catalog's detail panel. Tests only.
    #[cfg(test)]
    pub fn set_detailing(&mut self, uuid: Uuid) {
        self.detailing = Some(uuid);
    }

    /// Choose an appearance. Settings and the render tests share this, so neither
    /// can change palettes in a way the other does not.
    pub fn set_appearance(&mut self, appearance: Appearance, ctx: &egui::Context) {
        self.preferences.change(|chosen| chosen.appearance = appearance);
        self.settle_palette(ctx);
    }

    /// Bring the palette into line with the appearance that was chosen.
    ///
    /// Every frame, and not only when the switch is pressed, because
    /// [`Appearance::System`] is a question and not a palette: the desktop can
    /// answer it differently while the window stands open. **That is not
    /// polling.** egui carries the system theme in its own state and repaints
    /// when the platform tells it the theme changed, so this reads a value that
    /// is already in hand and nothing here goes looking.
    ///
    /// The applying is guarded on the answer having changed, because
    /// [`theme::apply`] rebuilds every style egui keeps.
    fn settle_palette(&mut self, ctx: &egui::Context) {
        let dark = match self.preferences.chosen().appearance {
            Appearance::Light => false,
            Appearance::Dark => true,
            // A platform that does not say is dark. That is what this
            // application opens in and what every mockup in the bundle is drawn
            // in, so it is the answer least likely to surprise.
            Appearance::System => {
                ctx.system_theme().is_none_or(|theme| theme == egui::Theme::Dark)
            }
        };
        if self.dark != dark {
            self.dark = dark;
            self.palette = if dark { Palette::DARK } else { Palette::LIGHT };
            theme::apply(ctx, self.palette);
        }
    }

    /// Everything the window draws.
    ///
    /// Separate from [`eframe::App::update`] so that it can be driven without a
    /// window, which is how it gets looked at.
    ///
    /// Three things only, because everything else belongs to a surface: which
    /// palette is in force, what the workers have said since the last frame, and
    /// the surface itself. The progress dialog is over all of them.
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.settle_palette(ui.ctx());
        self.pump(ui.ctx());
        self.take_drop(ui.ctx());

        match &self.screen {
            Screen::Browsing => self.browse(ui),
            Screen::Settings(_) => self.settings(ui),
            Screen::Restore(_) => self.restore(ui),
            Screen::About(_) => self.about(ui),
        }

        // The plan, before the work: it can only be up while the list is
        // showing, because the press that opens it is on the list's bar, and
        // its scrim is what keeps every other press from being made.
        if self.confirming.is_some() {
            self.confirm(ui);
        }

        // Last, and over everything - including a screen. Work can only be
        // started from the action bar, but it can still be running when the
        // overflow opens one, and a dialog that held the window still everywhere
        // except the one surface with no way back to the list would be worse than
        // one drawn over a screen the design never drew it over.
        if let Some(applying) = self.applying.as_ref().filter(|a| a.steps.is_some()) {
            progress(ui, self.palette, applying);
        }
    }

    /// The surface the window is for: the install bar, whichever list the view
    /// selects, and the action bar under it.
    ///
    /// Whole, and in one place, because it is one of several. The three
    /// surfaces behind the overflow are not panels over the list - each is
    /// `width:100%; height:100%` on the page colour with a header of its own -
    /// so each replaces every one of these, the bars included. Drawing this
    /// from `draw` directly meant that swapping in another would have been four
    /// separate conditions that all had to agree about one fact.
    fn browse(&mut self, ui: &mut egui::Ui) {
        // Every published item's state, worked out once for the frame because
        // three surfaces are drawn from it: the action bar counts what the
        // install filter is letting through, the toolbar counts the kinds it is,
        // and the list draws the rows. Only in the Catalog view - in Local there
        // is nothing on screen it answers for, and it walks the whole index
        // against the whole entry list.
        //
        // Before the first panel rather than beside the list, because the action
        // bar is laid out first and asks the same question.
        let states = match self.view {
            View::Catalog => self.published_states(),
            View::Local => std::collections::BTreeMap::new(),
        };

        egui::Panel::top("install")
            .exact_size(metric::INSTALL_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.install_bar(ui));

        egui::Panel::bottom("action")
            .exact_size(metric::ACTION_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.action_bar(ui, &states));

        // Between the list and the action bar, which is where the design puts
        // it: a banner is about the press that is one control below it, and
        // above the list it would push the working area down the window every
        // time a condition appeared.
        //
        // One at a time, and what just happened wins. A condition that is still
        // true will still be true after the result has been put away.
        self.said(ui);

        // Before the toolbar and the page, because both are laid out in what
        // it leaves. The design draws the inspector as the list's sibling and
        // not as an overlay, which is the same statement: the toolbar beside it
        // is 548 wide, and so is every row under it.
        let inspector = self.aside(ui);

        if let Some(listing) = self.shows_a_list() {
            egui::Panel::top("toolbar")
                .exact_size(metric::TOOLBAR)
                .frame(widget::toolbar(self.palette))
                .show(ui, |ui| match listing {
                    View::Local => self.list_toolbar(ui),
                    View::Catalog => self.catalog_toolbar(ui, &states),
                });
        }

        egui::CentralPanel::default()
            .frame(widget::page(self.palette))
            .show(ui, |ui| self.page(ui, &states));

        // After the page it falls on, for the reason written on it.
        if let Some(aside) = inspector {
            aside.shadow(ui, self.palette);
        }
    }

    /// The Settings screen: every preference, and what this machine is.
    ///
    /// A full-window surface and not a panel, so it claims the window's whole
    /// `Ui` and draws a header of its own where the install bar would be. The
    /// view it was opened from is on the way back, which is what the design
    /// labels that control with.
    ///
    /// Every press is collected and acted on afterwards, because each of them
    /// changes something this screen is drawn from - a path, a placement, the
    /// palette - and the screen is still being drawn while they arrive.
    fn settings(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let from = self.view.label();
        // Three fields, borrowed separately: the screen holds what was resolved,
        // the preferences are what the controls state, and neither is reached
        // through a method - which is what lets the one that is written to be
        // borrowed beside the one that is read.
        let Screen::Settings(facts) = &self.screen else { return };
        let chosen = self.preferences.chosen();

        let mut chose = Chose::Nothing;
        egui::Panel::top("screen")
            .exact_size(metric::SCREEN_HEADER)
            .frame(widget::screen(palette))
            .show(ui, |ui| {
                // Nothing at the right end: the design puts a control there on
                // the Restore screen and on neither of the other two.
                if widget::screen_header(ui, palette, from, icon::SETTINGS, "Settings", |_| {})
                    .clicked()
                {
                    chose = Chose::Back;
                }
            });

        egui::CentralPanel::default().frame(widget::screen(palette)).show(ui, |ui| {
            widget::screen_body(ui, Measure::Controls, |ui| {
                widget::label_above(ui, palette, "Paths", metric::UNDER_A_GROUP_HEADING);
                widget::group_frame(palette, Padding::Rows).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = metric::BETWEEN_GROUP_ROWS;

                    widget::path_row(ui, palette, "Bitwig install", facts.install.as_deref(), |ui| {
                        if widget::reset_control(ui, palette).clicked() {
                            chose = Chose::Rediscover(Which::Install);
                        }
                        if widget::group_control(ui, palette, icon::BROWSE, BROWSE, Emphasis::Quiet)
                            .clicked()
                        {
                            chose = Chose::Locate(Which::Install);
                        }
                    });
                    widget::path_row(ui, palette, "User library", facts.library.as_deref(), |ui| {
                        if widget::reset_control(ui, palette).clicked() {
                            chose = Chose::Rediscover(Which::Library);
                        }
                        if widget::group_control(ui, palette, icon::BROWSE, BROWSE, Emphasis::Quiet)
                            .clicked()
                        {
                            chose = Chose::Locate(Which::Library);
                        }
                    });
                    widget::path_row(ui, palette, "Backups", facts.backups.as_deref(), |ui| {
                        // Where backups live is not a preference - the class in
                        // the installation joins `user.home` with a fixed name
                        // to find the entry list beside them - so there is
                        // nothing to reset and nothing to browse to. The slot
                        // is held anyway, so that this row's control lines up
                        // with the two above it.
                        widget::reset_slot(ui);
                        if facts.backup {
                            if widget::group_control(
                                ui,
                                palette,
                                icon::RESTORE,
                                "Restore...",
                                Emphasis::Quiet,
                            )
                            .clicked()
                            {
                                chose = Chose::Restore;
                            }
                        } else {
                            ui.label(
                                font::run("No backup yet", font::plain(font::CHIP))
                                    .color(palette.ink_3),
                            );
                        }
                    });
                });

                ui.add_space(metric::BETWEEN_SETTINGS_GROUPS);
                widget::label_above(
                    ui,
                    palette,
                    "Document placement",
                    metric::UNDER_A_GROUP_HEADING,
                );
                widget::group_frame(palette, Padding::Choices).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = metric::BETWEEN_CHOICES;
                    for (strategy, title, note) in PLACEMENTS {
                        let chosen = chosen.placement == strategy;
                        if widget::choice(ui, palette, chosen, title, note).clicked() {
                            chose = Chose::Placement(strategy);
                        }
                    }
                });

                ui.add_space(metric::BETWEEN_SETTINGS_GROUPS);
                widget::label_above(ui, palette, "Removing entries", metric::UNDER_A_GROUP_HEADING);
                let on = chosen.delete_file;
                let pressed = widget::group_frame(palette, Padding::Rows)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        widget::switched(
                            ui,
                            palette,
                            on,
                            "Also delete the document file",
                            if on { DELETES_THE_FILE } else { KEEPS_THE_FILE },
                        )
                    })
                    .inner;
                if pressed.clicked() {
                    chose = Chose::DeleteFile(!on);
                }

                ui.add_space(metric::BETWEEN_SETTINGS_GROUPS);
                widget::label_above(ui, palette, "Appearance", metric::UNDER_A_GROUP_HEADING);
                widget::group_frame(palette, Padding::Switch).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    let options = Appearance::ALL.map(|a| (a, appearance_icon(a), a.label()));
                    if let Some(picked) =
                        widget::segmented(ui, palette, chosen.appearance, &options)
                    {
                        chose = Chose::Appearance(picked);
                    }
                });

                ui.add_space(metric::BETWEEN_SETTINGS_GROUPS);
                widget::label_above(ui, palette, "Diagnostics", metric::UNDER_A_GROUP_HEADING);
                widget::group_frame(palette, Padding::Rows).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.horizontal(|ui| {
                        // The control first, from the right, so the sentence
                        // beside it wraps in what is left rather than pushing it
                        // off the group.
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            if widget::group_control(
                                ui,
                                palette,
                                icon::COPY_REPORT,
                                "Copy report",
                                Emphasis::Loud,
                            )
                            .clicked()
                            {
                                chose = Chose::CopyReport;
                            }
                            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                                ui.label(
                                    font::wrapping(
                                        "Send this when a Bitwig build is not recognised.",
                                        font::NOTE,
                                        font::Leading::Explaining,
                                    )
                                    .color(palette.ink_3),
                                );
                            });
                        });
                    });
                    ui.add_space(metric::ABOVE_A_REPORT);
                    widget::report(ui, palette, &facts.report);
                });

                ui.add_space(metric::BETWEEN_SETTINGS_GROUPS);
                if widget::screen_link(
                    ui,
                    palette,
                    icon::ABOUT,
                    "About ORNG Registry",
                    env!("CARGO_PKG_VERSION"),
                )
                .clicked()
                {
                    chose = Chose::About;
                }
            });
        });

        self.chose(chose, ui);
    }

    /// Do whatever Settings was pressed for.
    ///
    /// Apart from the drawing, because every one of these writes to something the
    /// screen was just drawn from.
    fn chose(&mut self, chose: Chose, ui: &egui::Ui) {
        match chose {
            Chose::Nothing => {}
            Chose::Back => self.screen = Screen::Browsing,
            Chose::Locate(which) => self.locate(which),
            Chose::Rediscover(which) => {
                self.preferences.change(|chosen| match which {
                    Which::Install => chosen.install = None,
                    Which::Library => chosen.library = None,
                });
                self.reread();
            }
            Chose::Placement(strategy) => {
                self.preferences.change(|chosen| chosen.placement = strategy);
                // **Not a re-read.** Where a document goes is a preference, not
                // something read off the machine, and reading the machine again
                // means opening the archive and resolving its anchors - seconds,
                // for an answer already in hand. The one place it has to reach is
                // the destination every write is handed.
                if let Session::Found(found) = &mut self.session {
                    found.to.placement = strategy;
                }
                self.settle_screen();
            }
            Chose::DeleteFile(delete) => {
                self.preferences.change(|chosen| chosen.delete_file = delete);
            }
            Chose::Appearance(appearance) => self.set_appearance(appearance, ui.ctx()),
            Chose::CopyReport => {
                if let Screen::Settings(facts) = &self.screen {
                    ui.ctx().copy_text(facts.report.clone());
                }
            }
            Chose::Settings => self.screen = Screen::Settings(Diagnostics::of(&self.session)),
            Chose::Restore => self.screen = Screen::Restore(Backups::of(&self.session)),
            Chose::About => self.screen = Screen::About(About::of(&self.session)),
            Chose::RevealBackups => {
                if let Session::Found(found) = &self.session {
                    reveal(&found.to.home.backups());
                }
            }
        }
    }

    /// The Restore screen: which pristine copies this machine holds, and the one
    /// press that puts one back.
    ///
    /// A full-window surface like Settings, and the only one with a bar at the
    /// foot: it is the one screen that ends in a decision. The warning above the
    /// list is outside the scroll on purpose - what it says is about the press at
    /// the foot, and a warning that could be scrolled away is one the user can
    /// press without.
    fn restore(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let from = self.view.label();
        let Screen::Restore(backups) = &self.screen else { return };
        // What stops the press, if anything does. Restoring replaces the archive
        // Bitwig is running out of, so it is held by a running Bitwig for the
        // same reason preparation is - and the design says so in the line at the
        // foot. Drawn as the design's own disabled primary rather than as a
        // surface of its own.
        let blocked = match &self.session {
            Session::Found(found) => match &found.running {
                RunState::Running(processes) => {
                    Some(format!("Quit Bitwig Studio first. Running: {}.", processes.join(", ")))
                }
                _ => None,
            },
            _ => Some("There is no installation to restore into.".to_owned()),
        };

        let mut pressed = Restoring::Nothing;
        egui::Panel::top("screen")
            .exact_size(metric::SCREEN_HEADER)
            .frame(widget::screen(palette))
            .show(ui, |ui| {
                let back = widget::screen_header(
                    ui,
                    palette,
                    from,
                    icon::RESTORE,
                    "Restore backup",
                    |ui| {
                        // Padded one pixel tighter each side than the bundle's
                        // nine, which is `Emphasis::Quiet`'s eight. The design
                        // states nine only where the label is long, and a third
                        // variant for two pixels would be a distinction nobody
                        // can see.
                        if widget::group_control(
                            ui,
                            palette,
                            icon::BACKUPS,
                            OPEN_BACKUPS,
                            Emphasis::Quiet,
                        )
                        .clicked()
                        {
                            pressed = Restoring::Reveal;
                        }
                    },
                );
                if back.clicked() {
                    pressed = Restoring::Back;
                }
            });

        egui::Panel::bottom("foot")
            .exact_size(metric::SCREEN_FOOT)
            .frame(widget::bar(palette))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.spacing_mut().item_spacing.x = metric::ALONG_A_SCREEN_FOOT;
                    // From the right, so the sentence takes what the pair of
                    // controls leaves rather than pushing them off the window.
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        let reason = blocked.clone().unwrap_or_default();
                        if widget::screen_primary(
                            ui,
                            palette,
                            icon::RESTORE,
                            "Restore this backup",
                            blocked.is_none() && backups.chosen_day().is_some(),
                            &reason,
                        )
                        .clicked()
                        {
                            pressed = Restoring::Restore;
                        }
                        let foot = widget::Foot::Screen;
                        if widget::cancel_button(ui, palette, "Cancel", foot).clicked() {
                            pressed = Restoring::Back;
                        }
                        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                            ui.add(
                                egui::Label::new(
                                    font::run(foot_note(backups), font::plain(font::CHIP))
                                        .color(palette.ink_3),
                                )
                                .truncate(),
                            );
                        });
                    });
                });
            });

        egui::CentralPanel::default().frame(widget::screen(palette)).show(ui, |ui| {
            if backups.taken().is_empty() {
                widget::empty_state(ui, palette, &NOTHING_TO_RESTORE);
                return;
            }
            widget::restore_body(
                ui,
                |ui| {
                    widget::notice(
                        ui,
                        palette,
                        &widget::Notice {
                            tone: Tone::Warn,
                            glyph: icon::WARNING,
                            headline: Some(RESTORE_COSTS),
                            body: RESTORE_KEEPS,
                            leading: font::Leading::Noticing,
                        },
                    );
                },
                |ui| {
                    widget::label_above(
                        ui,
                        palette,
                        "Available backups",
                        metric::UNDER_A_GROUP_HEADING,
                    );
                    widget::backup_list(ui, palette, |ui| {
                        let of = backups.taken().len();
                        for (at, taken) in backups.taken().iter().enumerate() {
                            let row = widget::Taken {
                                when: &taken.when,
                                what: &taken.what,
                                size: &taken.size,
                                latest: at == 0,
                                chosen: backups.is_chosen(at),
                                at,
                                of,
                            };
                            if widget::backup_row(ui, palette, &row).clicked() {
                                pressed = Restoring::Choose(at);
                            }
                        }
                    });
                },
            );
        });

        self.restoring(pressed, ui.ctx());
    }

    /// Do whatever the Restore screen was pressed for.
    fn restoring(&mut self, pressed: Restoring, ctx: &egui::Context) {
        match pressed {
            Restoring::Nothing => {}
            Restoring::Back => self.screen = Screen::Browsing,
            Restoring::Reveal => {
                if let Session::Found(found) = &self.session {
                    reveal(&found.to.home.backups());
                }
            }
            Restoring::Choose(at) => {
                if let Screen::Restore(backups) = &mut self.screen {
                    backups.choose(at);
                }
            }
            Restoring::Restore => self.put_back(ctx),
        }
    }

    /// Put the chosen copy back over the installation.
    ///
    /// **On this thread, and that is a decision rather than an oversight.** What
    /// it does is copy four files - an archive of about 35 MB and three
    /// description bundles - where a preparation rewrites the archive and then
    /// verifies it under Bitwig's own JVM, which is the seconds [`work`] exists
    /// for. A worker here would buy a frame and cost a second progress
    /// protocol, and the design draws no progress for it.
    ///
    /// Afterwards the machine has changed under every answer this window holds:
    /// the archive is Bitwig's own again, so the guard is armed and the helper
    /// is gone, and the entry list is untouched - which is exactly the
    /// `Needs re-apply` state. So it goes back to the list, which is where that
    /// is said.
    fn put_back(&mut self, ctx: &egui::Context) {
        let Screen::Restore(backups) = &self.screen else { return };
        let Session::Found(found) = &self.session else { return };
        // A restore replaces files inside the installation, so it needs the
        // same rights preparing does and is handed over the same way when this
        // process does not hold them. Still on this thread either way: the
        // design draws no progress for it, and a child that is waited on is no
        // more blocking than a copy that is.
        let outcome = match &found.rights {
            Rights::Held => {
                backups.restore(&found.to.install).map(|done| done.map_err(|e| e.to_string()))
            }
            Rights::Withheld { .. } => backups.chosen_directory().map(|from| {
                elevate::run(
                    &elevate::Task::Restore { from: from.to_path_buf() },
                    &found.to.install,
                    // The copies the child holds `from` against are the ones
                    // under this account's home, which it cannot discover for
                    // itself if it was elevated as somebody else.
                    &found.to.home,
                    &|_| {},
                )
                .map(drop)
            }),
        };
        self.screen = Screen::Browsing;
        match outcome {
            // Nothing was pointed at, so nothing happened. Unreachable from the
            // window - the press is disabled with no copy chosen - and said
            // rather than panicked on, because the alternative to saying it is
            // a window that swallowed a press.
            None => return,
            Some(Ok(())) => self.outcome = Some(Outcome::Restored),
            Some(Err(why)) => self.outcome = Some(Outcome::NotRestored { why }),
        }
        self.reread();
        ctx.request_repaint();
    }

    /// The About screen: which build of this application is running, and what it
    /// made of the installation it found.
    fn about(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let from = self.view.label();
        let Screen::About(about) = &self.screen else { return };

        let mut asked = Asked::Nothing;
        egui::Panel::top("screen")
            .exact_size(metric::SCREEN_HEADER)
            .frame(widget::screen(palette))
            .show(ui, |ui| {
                if widget::screen_header(ui, palette, from, icon::ABOUT, "About", |_| {}).clicked()
                {
                    asked = Asked::Back;
                }
            });

        egui::CentralPanel::default().frame(widget::screen(palette)).show(ui, |ui| {
            widget::screen_body(ui, Measure::Prose, |ui| {
                widget::identity(ui, palette, PRODUCT_NAME, &about.identity);

                ui.add_space(metric::BETWEEN_ABOUT_BLOCKS);
                ui.label(
                    font::wrapping(WHAT_THIS_IS, font::FIELD_VALUE, font::Leading::Introducing)
                        .color(palette.ink_2),
                );

                ui.add_space(metric::BETWEEN_ABOUT_BLOCKS);
                widget::label_above(
                    ui,
                    palette,
                    "Detected installation",
                    metric::UNDER_A_GROUP_HEADING,
                );
                widget::group_frame(palette, Padding::Rows).show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.spacing_mut().item_spacing.y = metric::BETWEEN_ABOUT_FACTS;
                    widget::fact_row(ui, palette, "Version", &Fact::Read(&about.version));
                    widget::fact_row(ui, palette, "Build", &Fact::Read(&about.build));
                    widget::fact_row(
                        ui,
                        palette,
                        "Resolution",
                        &Fact::Resolution(about.resolved),
                    );
                });

                ui.add_space(metric::BETWEEN_ABOUT_BLOCKS);
                widget::notice(
                    ui,
                    palette,
                    &widget::Notice {
                        tone: Tone::Neutral,
                        glyph: icon::LICENCE,
                        headline: None,
                        body: NOT_AFFILIATED,
                        leading: font::Leading::Describing,
                    },
                );

                ui.add_space(metric::BETWEEN_ABOUT_BLOCKS);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = metric::BETWEEN_SCREEN_BUTTONS;
                    // `Licences` is drawn beside this one in the bundle and is
                    // not drawn here: nothing in this build carries the text it
                    // would show. Recorded in `design-review.md` round 3 rather
                    // than left as a gap.
                    if widget::screen_button(ui, palette, icon::COPY_REPORT, "Copy diagnostics")
                        .clicked()
                    {
                        asked = Asked::CopyReport;
                    }
                });
            });
        });

        match asked {
            Asked::Nothing => {}
            Asked::Back => self.screen = Screen::Browsing,
            Asked::CopyReport => {
                if let Screen::About(about) = &self.screen {
                    ui.ctx().copy_text(about.report.clone());
                }
            }
        }
    }

    /// Read the machine again, because where to look has changed.
    fn reread(&mut self) {
        self.session = Session::read(self.preferences.chosen());
        // The queued removals go with the list they were queued against. Staged
        // rows stay, and the difference is what each one means: a staged row is
        // a document to register wherever this application is pointed, and a
        // removal is an instruction about one particular list.
        self.removing.clear();
        // And so does what was waiting on a restart: it was a statement about
        // rows in the list that has just been replaced.
        self.awaiting_restart.clear();
        // With it goes a description waiting to be saved. It revises one row of
        // the list that has just been replaced, and the panel it was typed in
        // is about to be settled against the new one.
        self.asking = None;
        self.settle_screen();
    }

    /// Answer the open screen's questions about the machine again.
    ///
    /// Called where this application has changed one of the answers - a path, a
    /// placement, a run that reported, a copy put back - and nowhere else. A
    /// screen holds what was resolved rather than asking per frame, so something
    /// has to say when it has gone stale, and the honest list of those somethings
    /// is short. The same trade [`Inspection::placement`] makes, written down
    /// there.
    fn settle_screen(&mut self) {
        self.screen = match &self.screen {
            Screen::Browsing => return,
            Screen::Settings(_) => Screen::Settings(Diagnostics::of(&self.session)),
            // Deliberately *not* carried over: the chosen row is an index into
            // the list, and the list is what was just re-read. A restore is the
            // only thing that re-reads while this screen is open, and it closes
            // the screen.
            Screen::Restore(_) => Screen::Restore(Backups::of(&self.session)),
            Screen::About(_) => Screen::About(About::of(&self.session)),
        };
    }

    /// The inspector, if a row has been opened.
    ///
    /// Everything it states is worked out here rather than in the widget: where
    /// the document came from is a fact about the entry and where it actually
    /// is is a fact about the disk, and a panel that went looking for either
    /// could not be drawn from a fixture.
    /// Answers where the panel ended up, so that the shadow it casts on the
    /// list can be painted once the list is there to catch it.
    fn inspect(&mut self, ui: &mut egui::Ui) -> Option<widget::Aside> {
        let palette = self.palette;
        // Read before the panel borrows, because the panel borrows mutably to
        // type into the words and this asks the preferences the same `self`
        // holds.
        let document = self.deleting();
        let Session::Found(found) = &self.session else { return None };
        let uuid = self.inspecting.as_ref()?.uuid;
        let Some(entry) = found.entries().get(uuid) else {
            // Applied, removed, or gone from a list that was read again. There
            // is nothing left to inspect, so the panel closes rather than
            // standing empty - and the words it was holding go with it.
            self.inspecting = None;
            return None;
        };

        let (source, source_icon) = match &entry.provenance {
            Provenance::Local => ("Local file".to_owned(), widget::icon::LOCAL_FILE),
            Provenance::Catalog { version, .. } => {
                (format!("ORNG Catalog {} {version}", widget::SEPARATOR), widget::icon::CATALOG)
            }
        };
        // Off the entry rather than off the catalog, and that is the point of
        // recording it: this says which review *this copy* came through, and the
        // catalog has gone on publishing since. An item superseded last month is
        // still traceable here and is not in the index at all.
        let provenance = match &entry.provenance {
            Provenance::Catalog { reviewed_in: Some(revision), .. } => Some((
                format!("orng-catalog@{}", revision.short()),
                crate::catalog::commit(revision),
            )),
            _ => None,
        };
        let identity = uuid.to_string();
        // Before the panel is borrowed to type into, for the reason `document`
        // is: this asks the same `self` the words are held in.
        let status = self.status_of(found, entry);
        // Split apart so the fields can be borrowed separately: the panel
        // states the placement and types into the words in one call.
        let Inspection { words, placement, .. } =
            self.inspecting.as_mut().expect("the panel was open a moment ago");
        let item = widget::Inspected {
            kind: entry.kind(),
            name: &entry.name,
            uuid: &identity,
            path: entry.library_path.as_str(),
            source: &source,
            source_icon,
            provenance: provenance.as_ref().map(|(at, url)| (at.as_str(), url.as_str())),
            placement,
            status,
            document,
        };

        let (panel, pressed) =
            widget::aside(ui, palette, "inspector", |ui| widget::inspector(ui, palette, &item, words));

        match pressed {
            // The panel closing is the last chance a field has to be finished
            // with, and the one people take: the way to stop editing is to shut
            // the thing you were editing in.
            widget::Inspecting::Closed => {
                self.write_words(ui.ctx());
                self.inspecting = None;
            }
            widget::Inspecting::Edited => self.write_words(ui.ctx()),
            widget::Inspecting::CopiedUuid => ui.ctx().copy_text(uuid.to_string()),
            // Leaves the application, as the detail panel's own does and for
            // the same reason: the review is in the catalog's pull request.
            widget::Inspecting::Provenance => {
                if let Some((_, url)) = &provenance {
                    browse(url);
                }
            }
            // Through the same call the row's own controls go through, rather
            // than a second implementation of each: what the panel offers is
            // the row's list with words on it, so what it does must be the
            // row's press. A removal queued here leaves the row struck through
            // and the panel standing open on it, because that is what queueing
            // does wherever it is pressed from.
            widget::Inspecting::Acted(action) => {
                self.act(Acting::Registered(uuid), action, ui.ctx());
            }
            widget::Inspecting::Nothing => {}
        }
        Some(panel)
    }

    /// The catalog's detail panel, if a row has been opened.
    ///
    /// Two of the things it says are not in the index row at all and are worked
    /// out here: whether this installation is new enough to load the item, and
    /// whether some other published item has taken its place. The second is a
    /// fact about the whole index rather than about the row.
    fn detail(&mut self, ui: &mut egui::Ui) -> Option<widget::Aside> {
        let palette = self.palette;
        let uuid = self.detailing?;
        let Session::Found(found) = &self.session else { return None };
        let index = self.index()?;
        let Some(entry) = index.items.iter().find(|item| item.uuid == uuid) else {
            // The index was fetched again and this item is not in it. Nothing
            // left to detail, so the panel closes rather than standing empty.
            self.detailing = None;
            return None;
        };

        // A build that does not state its version is not evidence that the item
        // will not load, and a warning drawn from a guess is worse than none.
        let compatible = match &found.condition.build {
            Some(build) => build.version >= entry.min_bitwig,
            None => true,
        };
        // The item that lists this one under `supersedes`. A revision that
        // changes the parameter set takes a new identity rather than reusing
        // the old one, so both stay published and the old one points here.
        let replacement = self.replacement_for(uuid);
        let status = self.published_status(found, entry);
        let document = self.deleting();
        let provenance = entry.merged_in.as_ref().map(|revision| {
            (format!("orng-catalog@{}", revision.short()), crate::catalog::commit(revision))
        });

        let version = entry.version.to_string();
        let requires = entry.min_bitwig.to_string();
        let author = entry.author.to_string();
        let item = widget::Detailed {
            kind: entry.kind.into(),
            name: &entry.name,
            author: &author,
            version: &version,
            description: &entry.description,
            requires: &requires,
            compatible,
            licence: &entry.license,
            keywords: &entry.keywords,
            uuid: &entry.uuid.to_string(),
            provenance: provenance.as_ref().map(|(at, url)| (at.as_str(), url.as_str())),
            homepage: entry.homepage.as_deref(),
            // Only where the row actually reads `Replacement available`, which
            // is not simply "something in the index replaces it": the offer is
            // to whoever already has the old one, and telling somebody who has
            // never installed it that a newer version exists as a separate
            // device is a paragraph about nothing. The bundle gates its own
            // notice on the status for the same reason.
            replaced_by: match status {
                Published::Superseded => replacement.map(|(name, _)| name),
                _ => None,
            },
            status: &status,
            document,
        };

        let (panel, pressed) =
            widget::aside(ui, palette, "detail", |ui| widget::detail(ui, palette, &item));

        match pressed {
            widget::Detailing::Closed => self.detailing = None,
            // Both of these leave the application, which is the point: the
            // review of an item is in the catalog's pull request and what it
            // does is on its author's own page.
            widget::Detailing::Provenance => {
                if let Some((_, url)) = &provenance {
                    browse(url);
                }
            }
            widget::Detailing::Homepage => {
                if let Some(homepage) = entry.homepage.as_deref() {
                    browse(homepage);
                }
            }
            widget::Detailing::Replacement => {
                self.detailing = replacement.map(|(_, uuid)| uuid);
            }
            // Through the same call the row's own control goes through, for the
            // reason the inspector routes its action list into `act`: what the
            // panel offers is the row's control with room for a word.
            widget::Detailing::Acted(offer) => self.offer(uuid, offer, ui.ctx()),
            // And this is the *Local* row's removal, not a second kind of one.
            // An installed item is a registered entry, so it is queued and
            // struck through until the apply that takes it away - which is what
            // queueing does wherever it is pressed from.
            widget::Detailing::Removed => {
                self.act(Acting::Registered(uuid), Action::Remove, ui.ctx());
            }
            widget::Detailing::Nothing => {}
        }
        Some(panel)
    }

    /// Write what is in the inspector's fields, if it differs from what the
    /// entry says.
    ///
    /// Through the worker a press of the primary action already uses, because
    /// it is the same operation: the description and the search keywords live
    /// in the installation's own bundles, so changing them rewrites all three
    /// of those and then the entry list, in that order and idempotently.
    ///
    /// Silent when nothing changed, which is most of the time - leaving a field
    /// untouched is still leaving it.
    fn write_words(&mut self, ctx: &egui::Context) {
        // Nothing starts on top of something already running. The window has
        // one piece of work at a time, and a preparation must not be replaced
        // by a description. The buffer keeps what was typed and the next time
        // a field is left it is written, so nothing is lost and nothing is
        // claimed to have been saved that was not.
        if self.applying.is_some() {
            return;
        }
        let Some(open) = &self.inspecting else { return };
        let Session::Found(found) = &self.session else { return };
        let Some(entry) = found.entries().get(open.uuid) else {
            return;
        };
        let Some(revised) = revised(entry, &open.words) else { return };
        // Where the write has to be carried to a process holding rights this
        // one does not, it is not made here. A field losing focus is not a
        // press, and this write raises Windows' consent dialog - so the words
        // wait for one. See [`App::asking`]. Only where there is a dialog to
        // raise: elsewhere the write is made and refused, and a `Save` that
        // could only fail would be an offer that leads nowhere.
        if !found.rights.are_held() && elevate::can_ask() {
            // One question at a time, and a second entry's words do not take
            // the first one's place: that would drop words the user was asked
            // about and never answered. They wait in the panel, as they wait
            // behind a run, and are asked about when a field is next left.
            if self.asking.as_ref().is_some_and(|asked| asked.uuid != open.uuid) {
                return;
            }
            self.asking = Some(Unsaved { uuid: open.uuid, words: open.words.clone() });
            return;
        }
        self.write(revised, ctx);
    }

    /// Write one revised entry, now.
    ///
    /// The tail of [`App::write_words`], apart from it because the answer to
    /// [`App::asking`] arrives at a different moment and has to do the same
    /// thing.
    fn write(&mut self, revised: Registration, ctx: &egui::Context) {
        assert!(self.applying.is_none(), "an edit was written on top of a run in flight");
        let Session::Found(found) = &self.session else { return };
        let mut job = Job::against(Work::Entries, &found.to, found.entries());
        job.revise(revised);
        self.applying = Some(Applying::start(
            Errand::Edit,
            found.to.clone(),
            job,
            found.rights.clone(),
            ctx.clone(),
        ));
    }

    /// Ask whether to write the words that are waiting, and do it if told to.
    ///
    /// Drawn where a banner is drawn and answered before one: this is a
    /// question about something that has not happened, and [`App::outcome`] is
    /// a statement about something that has. A question the user has been
    /// asked outranks a report they have already read.
    ///
    /// **Not while a run is in flight.** `Save` starts one, and nothing starts
    /// on top of another - the rule [`App::write_words`] states. A run with no
    /// steps draws no scrim, so the question would otherwise stand pressable
    /// over it. It waits, and is asked again when the run has reported.
    fn ask_to_save(&mut self, ui: &mut egui::Ui) -> bool {
        if self.applying.is_some() {
            return false;
        }
        let (Session::Found(found), Some(waiting)) = (&self.session, &self.asking) else {
            return false;
        };
        // The entry it revises may have gone since - a removal applied while
        // this sat here. Nothing to save, and `Update::revise` would panic on
        // an identity the list has no row for. Or it may already say these
        // words, and then there is nothing to ask.
        let Some((entry, revised)) = found
            .entries()
            .get(waiting.uuid)
            .and_then(|entry| Some((entry, revised(entry, &waiting.words)?)))
        else {
            self.asking = None;
            return false;
        };
        let title = format!("Save the changes to {}?", entry.name);
        let body = format!(
            "{} is not writable by this account, so saving asks Windows for administrator \
             rights. The description and keywords Bitwig shows are kept inside the \
             installation.",
            widget::drawn_path(found.to.install.root())
        );

        let mut answered = widget::Answered::Nothing;
        egui::Panel::bottom("banner")
            .frame(egui::Frame::new().fill(self.palette.bg))
            .show(ui, |ui| {
                let banner = widget::Banner {
                    tone: Tone::Warn,
                    title: &title,
                    body: &body,
                    action: Some("Save"),
                    cancel: Some("Cancel"),
                    // The two words are the whole answer. A dismiss mark beside
                    // them would be a third way out that says neither.
                    dismissible: false,
                };
                answered = widget::banner(ui, self.palette, &banner);
            });

        match answered {
            widget::Answered::Action => {
                self.asking = None;
                self.write(revised, ui.ctx());
            }
            // The words are dropped and the entry is still what it was, which
            // is what makes this safe to offer rather than only a delay.
            widget::Answered::Cancelled | widget::Answered::Dismissed => self.asking = None,
            widget::Answered::Nothing => {}
        }
        true
    }

    /// The one banner the window is carrying, if it is carrying one.
    ///
    /// The panel is filled before the banner washes over it. A panel with no
    /// frame of its own shows whatever was behind the window, and a translucent
    /// wash over that is not a colour anybody chose.
    fn said(&mut self, ui: &mut egui::Ui) {
        /// Which of the two a banner is about, since what answering it means
        /// depends on that and not on which control was pressed.
        enum Regarding {
            /// Something that has happened and will not un-happen.
            Outcome,
            /// Something that is true of the machine and may stop being.
            Condition,
        }

        // A question about something that has not happened yet comes before a
        // statement about something that has: the user is being asked, and the
        // bottom of the window holds one banner.
        if self.ask_to_save(ui) {
            return;
        }

        let (about, banner) = match (&self.outcome, self.blocking()) {
            (Some(outcome), _) => {
                let (tone, title, body, action) = outcome.banner();
                (Regarding::Outcome, (tone, title, body, action, true))
            }
            (None, Some(blocked)) => (
                Regarding::Condition,
                (blocked.tone, blocked.title.to_owned(), blocked.body, blocked.action, false),
            ),
            (None, None) => return,
        };
        let (tone, title, body, action, dismissible) = banner;

        let mut answered = widget::Answered::Nothing;
        egui::Panel::bottom("banner")
            .frame(egui::Frame::new().fill(self.palette.bg))
            .show(ui, |ui| {
                let banner = widget::Banner {
                    tone,
                    title: &title,
                    body: &body,
                    action,
                    cancel: None,
                    dismissible,
                };
                answered = widget::banner(ui, self.palette, &banner);
            });
        if answered == widget::Answered::Nothing {
            return;
        }

        match about {
            Regarding::Outcome => match answered {
                // The words of whatever failed, for a bug report.
                widget::Answered::Action => {
                    if let Some(details) = self.outcome.as_ref().and_then(Outcome::details) {
                        ui.ctx().copy_text(details.to_owned());
                    }
                }
                _ => self.outcome = None,
            },
            // The condition is about the machine, not about this window, so the
            // only honest way to answer "has it changed" is to look again.
            Regarding::Condition => self.reread(),
        }
    }
}

impl eframe::App for App {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        // Colour management is AppKit's, and it is undone when the window moves
        // between displays - so it is re-applied per frame rather than once at
        // startup. Not in `draw`, which the render harness also calls and which
        // has no window behind it.
        #[cfg(target_os = "macos")]
        crate::macos::manage_colour(_frame);

        self.draw(ui);
    }
}

impl App {
    /// Take whatever the workers have said.
    ///
    /// Drawing happens from `self`, so anything a worker produced has to be
    /// moved into `self` before the frame is built - which is why this is the
    /// first thing `draw` does rather than the last. Polling afterwards would
    /// render the previous frame's state and always trail by one.
    ///
    /// No timer. A worker wakes the window when it has something to say, so a
    /// frame that gets here has a reason to have been drawn, and idle work
    /// costs nothing at all.
    fn pump(&mut self, ctx: &egui::Context) {
        // A refusal is a claim about one row of one index, so an index that has
        // been replaced takes them with it. Not merely tidiness: the map is
        // keyed by identity and the bar reads `Install refused` while anything
        // is in it, so a refusal against an item the catalog has since stopped
        // publishing would sit on that bar for the rest of the run with no row
        // under it to explain itself.
        if self.catalog.poll() {
            self.refused.clear();
        }
        // Not while a run is in flight. A press builds its job out of the list
        // as it stands and clears that list when it reports, so a row folded in
        // between the two was never part of the run and would be counted as
        // registered and then thrown away. The reader holds it instead - the
        // rows are still in its channel - and it is taken the frame after the
        // run is over.
        if self.applying.is_none()
            && let Some(read) = self.reading.as_mut().and_then(Reading::take)
        {
            self.staged.extend(read);
            self.reading = None;
        }
        // Before the write below, because a finished fetch is what *starts* one:
        // the two halves of an install are one press, and waiting a frame
        // between them would draw a row that had stopped downloading and had not
        // begun registering.
        let answered = self.installing.as_mut().is_some_and(|fetching| {
            fetching.poll();
            !fetching.is_running()
        });
        if answered {
            let finished = self.installing.take().expect("it answered a moment ago");
            self.installed(finished, ctx);
        }
        let Some(applying) = &mut self.applying else { return };
        applying.poll();
        let Some(result) = &applying.outcome else { return };
        // Taken once, here, and the work is then over: an `Applying` that has
        // reported is not work in flight, and leaving it in that field is what
        // made every screen after a press have to ask whether it had finished.
        let result = result.clone();
        let errand = applying.errand;
        // Taken before the work is dropped: what a run wrote is the update's
        // own answer, and the update went with the worker.
        let wrote: Vec<Uuid> = applying.writing.iter().copied().collect();
        let written = self.ready().count();
        let removed = match &self.session {
            Session::Found(found) => self.removals(found).count(),
            _ => 0,
        };
        self.applying = None;

        match result {
            // An edit is not announced. The panel is already showing what the
            // entry now says, and a banner after every description would be the
            // window reading its own fields back.
            Ok(entries) if errand == Errand::Edit => {
                if let Session::Found(found) = &mut self.session {
                    found.relist(entries);
                }
                self.awaiting_restart.extend(wrote);
            }
            // Announced, unlike an edit: the user pressed a control on a row
            // and nothing else on screen would show that the file is back.
            // Nothing pending is cleared, because locating a file is not one of
            // the things the primary action does.
            Ok(entries) if errand == Errand::Locate => {
                if let Session::Found(found) = &mut self.session {
                    found.relist(entries);
                }
                self.outcome = Some(Outcome::Located);
                self.awaiting_restart.extend(wrote);
            }
            // Announced, and it names the item: the press was about one row of
            // a list the user is looking at, and "1 entry registered" would be
            // the window declining to say which. Nothing pending is cleared,
            // because an install is not one of the things the primary action
            // does - somebody with three documents staged may install from the
            // catalog and still expect to press Apply afterwards.
            Ok(entries) if errand == Errand::Install => {
                let [uuid] = wrote[..] else {
                    panic!("an install writes exactly one row, and this one wrote {}", wrote.len())
                };
                let name = entries.get(uuid).expect("the row this run just wrote").name.clone();
                if let Session::Found(found) = &mut self.session {
                    found.relist(entries);
                }
                self.outcome = Some(Outcome::Installed { name });
                self.awaiting_restart.extend(wrote);
            }
            Ok(entries) => {
                // What was written is no longer pending. Held until here rather
                // than cleared when the press started, so that a failure leaves
                // the same rows to press again instead of asking for the drop
                // back. The removal queue goes the same way and for the same
                // reason: those entries are gone from the list now, so a queue
                // still naming them would strike through rows that do not
                // exist.
                //
                // Everything staged is what this run was built from, which is
                // what `App::read` and the reader below are between them
                // holding to: nothing joins the list while a run is in flight,
                // so there is nothing here that the run did not consider.
                self.staged.clear();
                self.removing.clear();
                let in_effect = entries.entries().len();
                if errand.prepares() {
                    // A preparation changes what is true of the installation:
                    // the archive, the guard, the links. Nothing short of
                    // reading it again answers that.
                    self.session = Session::read(self.preferences.chosen());
                    // Nothing is waiting on a restart after this: Bitwig had to
                    // be closed for it, and the banner says to start it.
                    self.awaiting_restart.clear();
                    self.outcome = Some(Outcome::Prepared { entries: in_effect, removed });
                } else {
                    if let Session::Found(found) = &mut self.session {
                        // An entry update changes one text file, and the worker
                        // answered with what it wrote. Reading the machine
                        // again would cost seconds to arrive at the value
                        // already in hand.
                        found.relist(entries);
                    }
                    self.awaiting_restart.extend(wrote);
                    self.outcome = Some(Outcome::Registered { written, removed });
                }
            }
            // A failure is announced either way: an edit that did not reach
            // the disk is the one thing about it the panel cannot show.
            Err(why) => {
                self.outcome = Some(Outcome::Failed { what: errand, why });
            }
        }
        // After the session, because both answer against the list that is now
        // in hand. A failed run is asked too: what stopped half way through it
        // may still have moved the document, and may still have written the
        // backup that Settings is standing there saying does not exist.
        self.settle_placement();
        self.settle_screen();
    }

    /// Give the open panel the placement the session has just looked up again.
    ///
    /// Called when a run has reported, because this application writing is the
    /// one thing that moves a document out from under a panel that is standing
    /// open: a preparation relinks the library, and a registration places the
    /// documents it registers. The looking itself was done by the session -
    /// this only carries the answer across.
    fn settle_placement(&mut self) {
        let Session::Found(found) = &self.session else { return };
        let Some(open) = self.inspecting.as_mut() else { return };
        if found.entries().get(open.uuid).is_none() {
            return;
        }
        open.placement = found.standing(open.uuid).placement().clone();
    }

    /// Whatever has been dropped on the window this frame.
    fn take_drop(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input.raw.dropped_files.iter().map(|file| file.path().to_path_buf()).collect()
        });
        if !dropped.is_empty() {
            // Dropping is Local-view work, and the rows it produces only exist
            // there. A drop that landed while the catalog was showing would
            // otherwise look like a drop that did nothing.
            self.view = View::Local;
            self.read(staging::documents_in(&dropped), ctx);
        }
    }

    /// Start reading dropped or chosen files.
    ///
    /// **Nothing starts on top of something already running**, the rule
    /// `write_words` states and `install` and `relocate` follow. Here it is
    /// about the list rather than the worker: a press takes the staged rows as
    /// they stand, so a row read while it runs is one the press did not write
    /// and the press's own answer would clear. Refused, and a second reader
    /// would in any case displace the one already holding rows.
    fn read(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        if paths.is_empty() || self.applying.is_some() {
            return;
        }
        // What is already staged goes with it, so a second drop collides with
        // the first rather than quietly winning when the list is written.
        let already: Vec<Registration> =
            self.staged.iter().filter_map(Staged::registration).cloned().collect();
        self.reading = Some(Reading::start(
            paths,
            found.entries().clone(),
            found.to.clone(),
            already,
            ctx.clone(),
        ));
    }

    /// Which list the middle region is, if it is one - and so which toolbar
    /// belongs above it.
    ///
    /// Nothing to filter is nothing to filter with. The design keeps the
    /// toolbar when a filter has narrowed the list to nothing, because that is
    /// how the filter gets cleared, and drops it on the onboarding screen and on
    /// the catalog's own two full-region states, where there is no list behind
    /// it - `ORNG Registry.dc.html:484` sets `toolbar` on the no-match scenario
    /// and `:470` leaves it off the never-fetched one.
    ///
    /// Work in flight does not take it away either: the design draws the
    /// preparation over the list rather than instead of it.
    fn shows_a_list(&self) -> Option<View> {
        let Session::Found(found) = &self.session else { return None };
        let anything = match self.view {
            View::Local => !found.entries().is_empty() || !self.staged.is_empty(),
            // A catalog that has not arrived, did not verify or is empty has no
            // list under it: the whole region is the one thing there is to say.
            View::Catalog => self.index().is_some_and(|index| !index.items.is_empty()),
        };
        anything.then_some(self.view)
    }

    /// The rows that are ready to be written.
    fn ready(&self) -> impl Iterator<Item = &Staged> {
        self.staged.iter().filter(|staged| staged.is_ready())
    }

    /// What one press would do, given what is pending and what the installation
    /// is.
    ///
    /// `None` when there is nothing to press: a prepared installation with
    /// nothing staged has no work, and the design draws the action disabled
    /// rather than gone.
    fn pending(&self, found: &Found) -> Option<Work> {
        if !found.condition.is_prepared() {
            // An installation that does not read the entry list has work to do
            // whether or not anything is staged, because the entries already on
            // record are not in effect until it does.
            return Some(Work::PrepareThenEntries);
        }
        (self.changes(found) > 0).then_some(Work::Entries)
    }

    /// How many entries one press would change: the rows to add plus the rows
    /// to forget.
    ///
    /// A queued removal is work in its own right. Counting only the additions
    /// is what would leave the primary action disabled beside a list of struck
    /// through rows, saying there was nothing to apply.
    fn changes(&self, found: &Found) -> usize {
        self.ready().count() + self.removals(found).count()
    }

    /// What stands between the user and the primary action, if anything does.
    ///
    /// Two of the three are about the archive, so only the preparing mode can
    /// be held up by them: registering entries writes no part of it and is
    /// never held up by a running Bitwig, which is the whole of what the cheap
    /// mode buys.
    ///
    /// **Rights are the exception, and they hold up either mode.** The
    /// description bundles live inside the installation and are rewritten by
    /// every change to the entry list (4.4), so an installation this account
    /// may not write refuses the cheap mode for the same reason as the
    /// expensive one. That is only a refusal where the platform has no way to
    /// ask for more; where it has, the press asks, and is not blocked at all.
    fn blocking(&self) -> Option<Blocked> {
        let Session::Found(found) = &self.session else { return None };
        if self.applying.as_ref().is_some_and(Applying::is_running) {
            return None;
        }
        let pending = self.pending(found)?;

        // First, because it is the one condition that is true of both modes and
        // because it is the one the other two would otherwise hide: an
        // installation nothing may write is not made writable by quitting
        // Bitwig.
        if let Rights::Withheld { directory, .. } = &found.rights
            && !elevate::can_ask()
        {
            return Some(Blocked {
                tone: Tone::Err,
                title: "This installation is not yours to change.",
                body: format!(
                    "{} cannot be written by this account, and nothing registered takes \
                     effect until it can. Preparing writes the archive, and every entry \
                     change rewrites the description bundles beside it.",
                    widget::drawn_path(directory)
                ),
                // Nothing from here resolves it: the remedy is the installation's
                // permissions or the account this runs as, and neither is a
                // press. An offer that leads nowhere is worse than none - the
                // rule the unrecognised guard already follows.
                action: None,
            });
        }

        if pending != Work::PrepareThenEntries {
            return None;
        }
        match (&found.running, found.condition.guard) {
            (RunState::Running(processes), _) => Some(Blocked {
                tone: Tone::Warn,
                title: "Quit Bitwig Studio before preparing the installation.",
                body: format!(
                    "The audio engine holds the files this step has to replace. Running: {}.",
                    processes.join(", ")
                ),
                // Whether Bitwig is still open is a question about the machine,
                // and the user is the one who will have closed it.
                action: Some("Check again"),
            }),
            (_, orng_tools::GuardState::Unknown) => Some(Blocked {
                tone: Tone::Err,
                title: "This installation cannot be prepared.",
                body: "The tamper guard is not in a shape this build recognises, so preparation \
                       refuses rather than editing it blind."
                    .to_owned(),
                // Nothing the user can do from here resolves it. An offer that
                // leads nowhere is worse than none.
                action: None,
            }),
            _ => None,
        }
    }
}

/// Words typed into the inspector for one entry, waiting on [`App::asking`].
struct Unsaved {
    uuid: Uuid,
    words: widget::Words,
}

/// The inspector while it is open: which entry it is about, and the words in
/// its two editable fields.
///
/// One thing rather than an identity and a buffer held side by side. The buffer
/// exists exactly when the panel does, because it is what the panel types into.
/// Kept apart, the pair could say that the panel had been shut while a dead
/// entry's half-typed words were still being held, and the identity had to be
/// carried twice and checked every frame to get the invariant back.
///
/// The words sit beside the session rather than in it. The session is read off
/// the machine and re-read whenever the machine changes, and a half-typed
/// description is neither: writing into the entry as the user typed would mean
/// the list on screen disagreed with the list on disk, and re-reading would
/// throw away what was being written.
struct Inspection {
    /// The identity, and not the entry itself nor its place in the list. The
    /// session is re-read whenever anything is done to the machine, so a
    /// borrowed row would be stale by the next frame and a position would point
    /// at whatever had moved into it. An identity either is still registered or
    /// is not, and the panel closes when it is not.
    uuid: Uuid,
    words: widget::Words,
    /// Where the document actually is, as against where the registry says it
    /// is - which is a question about the disk rather than about the entry.
    ///
    /// Taken from what the session already looked at rather than asked again -
    /// the list resolves this for every entry when it reads it, and asking
    /// here would be a second `stat` and `lstat` for a row already answered.
    ///
    /// Copied rather than borrowed because the panel outlives the borrow: the
    /// arms that follow the draw take `&mut self` to put the panel away and to
    /// write the words.
    ///
    /// The trade is that a document moved by something *other* than this
    /// application, while the panel stands open, is not noticed until the panel
    /// is opened again. Nothing else in the window watches the disk either, so
    /// this is the same freshness the rest of the session has.
    placement: Placement,
}

impl Inspection {
    /// Open on an entry: its words as the entry states them, and where the
    /// session found its document.
    fn of(entry: &Registration, found: &Found) -> Inspection {
        Inspection {
            uuid: entry.uuid,
            words: widget::Words::of(&entry.description, &entry.keywords),
            placement: found.standing(entry.uuid).placement().clone(),
        }
    }
}

/// A condition the window has to state, and what can be done about it.
struct Blocked {
    tone: Tone,
    title: &'static str,
    body: String,
    action: Option<&'static str>,
}

/// What a press came to, once it is over.
///
/// Held apart from the work itself: work in flight is a dialog the window holds
/// still for, and a result is a banner it carries on around.
enum Outcome {
    /// The installation was prepared, so what it now reads is the whole list
    /// rather than the few rows this press added.
    Prepared { entries: usize, removed: usize },
    /// Entries were written into an installation that was already prepared.
    Registered { written: usize, removed: usize },
    /// An entry that had lost its document was pointed back at one.
    ///
    /// Its own variant rather than a registration of one: nothing was
    /// registered, the entry was already there, and the only thing that changed
    /// is that the file it names exists again.
    Located,
    /// A published item was installed. Named, because the press was about one
    /// item and a count of one is the window declining to say which.
    Installed { name: String },
    /// An item's bytes were not the ones the catalog states, so nothing was
    /// installed.
    ///
    /// Its own variant rather than a [`Outcome::Failed`]: what failed is not a
    /// run, because no run started. The design is explicit that this must not
    /// read like a network error, and the thing that makes it not read like one
    /// is the promise underneath - nothing reached the library or the
    /// installation, because nothing got as far as being written.
    Refused { item: String, why: String },
    Failed {
        /// Which run stopped, which is what decides what can honestly be
        /// promised about the state left behind: the three differ in what they
        /// had already done. Carried from the run rather than worked out at the
        /// end of it - a boolean said only whether a preparation was involved,
        /// which left an edit that could not be written reporting that nothing
        /// had been registered, true and about the wrong thing entirely.
        what: Errand,
        /// The worker's own words, which go into a bug report rather than onto
        /// the screen.
        why: String,
    },
    /// A pristine copy was put back over the installation.
    ///
    /// Its own pair rather than a fourth [`Errand`]: a restore is not something
    /// the worker runs, and widening the enum that says what a *run* is for
    /// would put a variant in it that no run can ever be. What is promised here
    /// is also different in kind - a preparation's transaction promises that
    /// nothing reached the installation, and this one promises that something
    /// did.
    Restored,
    NotRestored {
        why: String,
    },
}

impl Outcome {
    /// The words of whatever went wrong, where something did.
    ///
    /// What the banner's one control copies. Answered here rather than matched
    /// at the call site so that a fourth thing that can fail cannot be added
    /// with a control that copies nothing.
    fn details(&self) -> Option<&str> {
        match self {
            Outcome::Failed { why, .. }
            | Outcome::NotRestored { why }
            | Outcome::Refused { why, .. } => Some(why),
            Outcome::Prepared { .. }
            | Outcome::Registered { .. }
            | Outcome::Located
            | Outcome::Installed { .. }
            | Outcome::Restored => None,
        }
    }

    /// The two lines it is stated in, and what can be done about it.
    fn banner(&self) -> (Tone, String, String, Option<&'static str>) {
        match self {
            Outcome::Prepared { entries, removed } => (
                Tone::Ok,
                "Start Bitwig Studio. Your devices are in the browser.".to_owned(),
                format!(
                    "{} in effect{}. Descriptions and search keywords were written too, so \
                     typing a name finds the device.",
                    counted(*entries),
                    also_removed(*removed)
                ),
                None,
            ),
            Outcome::Registered { written, removed } => (
                Tone::Ok,
                "Restart Bitwig Studio to see your changes.".to_owned(),
                format!(
                    "{} registered{}. Bitwig reads the entry list when it launches, so an open \
                     Bitwig will not show the change yet.",
                    counted(*written),
                    also_removed(*removed)
                ),
                None,
            ),
            Outcome::Located => (
                Tone::Ok,
                "The document is back where the entry says it is.".to_owned(),
                "The entry itself was never touched, so its description and search keywords \
                 are the ones you had. Restart Bitwig Studio to load the document again."
                    .to_owned(),
                None,
            ),
            // What failed is the headline and the promise is the line under it,
            // which is the design's order. The worker's own words are behind
            // the control, because they are for a bug report and not for the
            // person reading this.
            Outcome::Failed { what: Errand::Preparation, .. } => (
                Tone::Err,
                "The preparation stopped, and your installation was not changed.".to_owned(),
                "The patched archive is written beside the original and only moved into place \
                 once it verifies, so nothing reached the installation."
                    .to_owned(),
                Some("Copy details"),
            ),
            Outcome::Failed { what: Errand::Registration, .. } => (
                Tone::Err,
                "Nothing was registered.".to_owned(),
                "The entry list is written last, so it is unchanged. Any document already \
                 placed is left where it is, and applying again finishes the job."
                    .to_owned(),
                Some("Copy details"),
            ),
            Outcome::Failed { what: Errand::Edit, .. } => (
                Tone::Err,
                "The change was not saved.".to_owned(),
                "Descriptions and search keywords live in the installation's own files, and \
                 that is the write that can be refused. The entry list is written after it \
                 and is unchanged."
                    .to_owned(),
                Some("Copy details"),
            ),
            Outcome::Installed { name } => (
                Tone::Ok,
                format!("{name} is registered. Restart Bitwig Studio to load it."),
                "Its description and search keywords were written too, so typing the name \
                 finds it in the browser. Nothing in the installation was changed: an item is \
                 a file and a row in the entry list."
                    .to_owned(),
                None,
            ),
            // Deliberately not worded as a network problem, and deliberately
            // offering no way to try again. What is being said is that the
            // catalog's review did not reach this machine intact, and the one
            // useful thing to do with that is to report it.
            Outcome::Refused { item, .. } => (
                Tone::Err,
                format!("{item} was not installed, and nothing was written."),
                "The file does not match the hash the catalog states for it, so it was \
                 refused before anything reached your library or your installation. The \
                 catalog's review is what stands behind an item, and this file is not the \
                 one that was reviewed."
                    .to_owned(),
                Some("Copy details"),
            ),
            // Says nothing about which half failed, because both leave the same
            // state: a file that was not this entry's was never written, and a
            // write that could not finish placed nothing the entry points at.
            Outcome::Failed { what: Errand::Locate, .. } => (
                Tone::Err,
                "The entry was not pointed at that file.".to_owned(),
                "Nothing was changed. The entry still names the document it always named, \
                 and that document is still missing."
                    .to_owned(),
                Some("Copy details"),
            ),
            // The bytes verified and the write is what stopped, so what can be
            // promised is what a registration promises: the entry list goes
            // last, and the item is not in it.
            Outcome::Failed { what: Errand::Install, .. } => (
                Tone::Err,
                "The item was not installed.".to_owned(),
                "It was fetched and checked against the digest the catalog states, and the \
                 write is what stopped. The entry list is written last, so it is unchanged \
                 and nothing is registered."
                    .to_owned(),
                Some("Copy details"),
            ),
            // Not `Ok`: nothing is wrong, and what the user has now is an
            // installation that no longer recalls anything they registered.
            // The design's warm tone is what says that.
            Outcome::Restored => (
                Tone::Warn,
                "This installation is back the way Bitwig shipped it.".to_owned(),
                "Nothing it was carrying is registered any more. Your documents and this \
                 application's own record are untouched, so applying again puts them back."
                    .to_owned(),
                None,
            ),
            Outcome::NotRestored { .. } => (
                Tone::Err,
                "The backup was not put back.".to_owned(),
                "The archive is replaced by a rename after the copy is complete, so the \
                 installation is running either the archive it had or the one from the \
                 backup - never a half-written one."
                    .to_owned(),
                Some("Copy details"),
            ),
        }
    }
}

/// How many entries a press dealt with, in words rather than as a bare number.
fn counted(registered: usize) -> String {
    match registered {
        1 => "1 entry".to_owned(),
        many => format!("{many} entries"),
    }
}

/// How old the catalog is, in the design's own two sentences.
///
/// Two, and the difference is not decoration: `Catalog updated 20 minutes ago`
/// reports a check that happened, and `Catalog from 12 days ago` describes what
/// is on screen. The bundle writes both - `InstallBar.dc.html:61`'s default and
/// `ORNG Registry.dc.html:467` - and puts the second beside the control that
/// has grown its label, which is the state that is asking for something.
fn stated(freshness: Freshness) -> String {
    match freshness {
        // `:472`. No age, because there is nothing to date.
        Freshness::Never => "Never fetched".to_owned(),
        Freshness::Current(age) => format!("Catalog updated {}", elapsed(age)),
        Freshness::Stale(age) => format!("Catalog from {}", elapsed(age)),
    }
}

/// A duration as the design writes one: the largest unit that gives a whole
/// number, and never a zero.
///
/// **Ours** - the bundle states two finished strings and no rule -
/// `docs/design-review.md` round 3 item 6. Nothing smaller than a minute is
/// counted, because a fetch that landed nine seconds ago rounds to
/// `0 minutes ago`, which reads as broken rather than as recent.
fn elapsed(age: std::time::Duration) -> String {
    const MINUTE: u64 = 60;
    const HOUR: u64 = 60 * MINUTE;
    const DAY: u64 = 24 * HOUR;

    let seconds = age.as_secs();
    let (count, unit) = match seconds {
        MINUTE..HOUR => (seconds / MINUTE, "minute"),
        HOUR..DAY => (seconds / HOUR, "hour"),
        DAY.. => (seconds / DAY, "day"),
        ..MINUTE => return "just now".to_owned(),
    };
    let plural = if count == 1 { "" } else { "s" };
    format!("{count} {unit}{plural} ago")
}

/// The clause naming what a press also took away, where it took anything away.
///
/// Empty when it took nothing, rather than ", 0 removed". Every count in this
/// window suppresses its zero, for the same reason the summary does: a part of
/// a sentence that is only ever there to say "none" is a part of the sentence
/// nobody reads.
fn also_removed(removed: usize) -> String {
    match removed {
        0 => String::new(),
        1 => ", 1 removed".to_owned(),
        many => format!(", {many} removed"),
    }
}

impl App {
    /// Region one: the installation this window is pointed at.
    ///
    /// One line, as the design draws it. The tamper guard and the backup date
    /// are not here: the bundle routes both to Settings, under Diagnostics, and
    /// a second line carrying them was this application's invention.
    fn install_bar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        // The gaps in a bar are the design's, stated one by one. egui would
        // otherwise add its own between every pair on top of them, which is six
        // pixels the bundle does not have and which compounds along the row.
        ui.spacing_mut().item_spacing.x = 0.0;
        // Taken out of the menu and acted on after the bar, because opening a
        // screen replaces the bar the menu is hanging off - and because the menu's
        // closure is being run inside a borrow of everything else here.
        let mut opening = Chose::Nothing;
        ui.horizontal_centered(|ui| {
            for (view, label) in [(View::Local, "Local"), (View::Catalog, "Catalog")] {
                if widget::view_tab(ui, palette, label, self.view == view).clicked() {
                    self.view = view;
                }
                ui.add_space(metric::SNUG);
            }
            ui.add_space(metric::GAP - metric::SNUG);

            // The controls are placed first, from the right, so the path gives
            // way to them rather than pushing them off the edge of the window.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                widget::overflow(ui, palette, |ui| {
                    if widget::menu_item(ui, palette, icon::SETTINGS, "Settings").clicked() {
                        opening = Chose::Settings;
                    }
                    if widget::menu_item(ui, palette, icon::RESTORE, "Restore backup...").clicked()
                    {
                        opening = Chose::Restore;
                    }
                    if widget::menu_item(ui, palette, icon::BACKUPS, OPEN_BACKUPS).clicked() {
                        opening = Chose::RevealBackups;
                    }
                    widget::menu_rule(ui, palette);
                    if widget::menu_item(ui, palette, icon::ABOUT, "About ORNG Registry").clicked()
                    {
                        opening = Chose::About;
                    }
                });
                // The install bar's own gap, which is the wide one: it is a bar
                // of separate things rather than a toolbar of related ones.
                ui.add_space(metric::GAP);
                if widget::small_button(
                    ui,
                    palette,
                    widget::icon::CHANGE_INSTALL,
                    "Change install",
                )
                .clicked()
                {
                    self.locate(Which::Install);
                }
                ui.add_space(metric::GAP);

                // Catalog view only, which is the bundle's own condition -
                // `InstallBar.dc.html:35`. Placed here rather than beside the
                // path because this layout runs from the right: the design's
                // order left to right is the age, the control, `Change
                // install`, so inserting them after that button puts them
                // before it on screen.
                if self.view == View::Catalog {
                    let freshness = self.catalog.freshness();
                    if widget::refresh(ui, palette, freshness.is_stale()).clicked() {
                        self.catalog.refresh(ui.ctx());
                    }
                    ui.add_space(metric::GAP);
                    widget::freshness(ui, palette, &stated(freshness), freshness.is_stale());
                    ui.add_space(metric::GAP);
                }

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    self.install_identity(ui);
                });
            });
        });
        self.chose(opening, ui);
    }

    /// Let the user choose documents, which drag and drop must never be the
    /// only way to do.
    fn add_files(&mut self, ui: &egui::Ui) {
        let chosen = rfd::FileDialog::new()
            .set_title("Add documents to register")
            .add_filter("Bitwig documents", &staging::ACCEPTED)
            .pick_files()
            .unwrap_or_default();
        self.read(staging::documents_in(&chosen), ui.ctx());
    }

    /// Let the user point at an installation, or at their library, themselves.
    ///
    /// **The choice is written down**, which is what Settings changed about this:
    /// before there was anywhere to keep it, the next launch went back to
    /// discovery and the user pointed at the same folder again.
    ///
    /// A folder the user insisted on is then read as the one that counts.
    /// Refusing it says why against that folder rather than falling back to the
    /// one already loaded, which would look like the picker did nothing -
    /// [`Session::read`] is where that happens, and `Reset to auto-detected` is
    /// the way back out of it.
    fn locate(&mut self, which: Which) {
        let title = match which {
            Which::Install => "Locate Bitwig Studio",
            Which::Library => "Locate the Bitwig user library",
        };
        let Some(root) = rfd::FileDialog::new().set_title(title).pick_folder() else {
            return;
        };
        self.preferences.change(|chosen| match which {
            Which::Install => chosen.install = Some(root),
            Which::Library => chosen.library = Some(root),
        });
        self.reread();
    }

    /// What this installation is: its name, its build, its path, its state.
    fn install_identity(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let found = match &self.session {
            Session::Found(found) => found,
            // An installation that cannot be read is still an installation, and
            // the bar has to keep naming it: this is the state a user reaches
            // the morning after a Bitwig release, and the path is the thing
            // they will be asked about.
            Session::Unreadable { root, .. } => {
                title(ui, "Bitwig Studio", palette.ink);
                ui.add_space(metric::GAP);
                path(ui, palette, root);
                ui.add_space(metric::GAP);
                badge(ui, palette, "Unknown build");
                return;
            }
            Session::NoInstallation { .. } => {
                title(ui, "No installation selected", palette.ink_3);
                return;
            }
        };

        title(ui, &found.title(), palette.ink);
        ui.add_space(metric::GAP);
        let revision = found.revision();
        if !revision.is_empty() {
            ui.label(
                font::run(revision, font::mono(font::MONO_TIGHT)).color(palette.ink_2),
            )
            .on_hover_text(found.revision_in_full());
            ui.add_space(metric::GAP);
        }

        // The badge takes whatever the path leaves, so it is never the thing
        // that gets truncated: the path is the longest item in the bar and the
        // least urgent.
        let state = found.badge();
        // `Registered` is the ordinary state and the design does not label it.
        // The count is in the list's own heading, which is where somebody
        // counting would look.
        let label = (state != Badge::Registered(found.entries().entries().len()))
            .then(|| state.label());
        let width = label.as_ref().map_or(0.0, |text| text.len() as f32 * BADGE_WIDTH_PER_CHAR);
        let room = (ui.available_width() - width - metric::GAP).max(0.0);
        ui.allocate_ui_with_layout(
            vec2(room, ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| path(ui, palette, &widget::drawn_path(found.to.install.root())),
        );
        if let Some(label) = label {
            ui.add_space(metric::GAP);
            badge(ui, palette, &label);
        }
    }

    /// Above the list: what to show of it, and the other way in.
    fn list_toolbar(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        ui.spacing_mut().item_spacing.x = 0.0;
        let Session::Found(found) = &self.session else { return };
        let counts: Vec<(Kind, usize)> = Kind::ALL
            .into_iter()
            .map(|kind| {
                let registered =
                    found.entries().entries().iter().filter(|e| e.kind() == kind).count();
                let staged = self
                    .staged
                    .iter()
                    .filter_map(Staged::registration)
                    .filter(|r| r.kind() == kind)
                    .count();
                (kind, registered + staged)
            })
            .collect();

        // Read out of the filter before anything below borrows it, because the
        // field beside the chips is written into while they are being drawn.
        let kinds: Vec<(Kind, usize, bool)> = counts
            .into_iter()
            .map(|(kind, count)| (kind, count, self.filter.kinds.contains(&kind)))
            .collect();

        let width = self.width();
        let hint = match width {
            widget::Width::Full => "Search name or UUID",
            widget::Width::Narrow => "Search",
        };
        // The design drops the labels from the controls at the right end when
        // the inspector is open, and leaves the glyph to say what they are.
        let add_files = match width {
            widget::Width::Full => "Add files...",
            widget::Width::Narrow => "",
        };

        // Drawn as two closures because they are laid out twice: once to find
        // out how wide they are and once for real. The field between them is
        // the flexible one and cannot be given its share until everything that
        // is not flexible has taken its own, and a second function stating
        // those widths would be free to drift from the one that draws them.
        let chips = |ui: &mut egui::Ui| -> Option<Kind> {
            let mut toggled = None;
            for (at, (kind, count, on)) in kinds.iter().enumerate() {
                if at > 0 {
                    ui.add_space(metric::SNUG);
                }
                if widget::filter_chip(ui, palette, plural(*kind), *count, *on, width).clicked() {
                    toggled = Some(*kind);
                }
            }
            toggled
        };
        let tail = |ui: &mut egui::Ui| -> bool {
            let pressed = widget::small_button(ui, palette, widget::icon::ADD_FILES, add_files);
            match width {
                widget::Width::Full => pressed.clicked(),
                widget::Width::Narrow => pressed.on_hover_text("Add files...").clicked(),
            }
        };

        // Five boxes in the design and four here, because the factory toggle is
        // deliberately not drawn - `docs/design-review.md` round 3 item 3. The
        // field, the chips, the flexible gap, and `Add files...`, with three
        // gaps between the four.
        const BETWEEN_TOOLBAR_GROUPS: f32 = 3.0 * metric::TOOL_GAP;
        let fixed = widget::measured(ui, "chips", |ui| {
            chips(ui);
        }) + widget::measured(ui, "tail", |ui| {
            tail(ui);
        }) + BETWEEN_TOOLBAR_GROUPS;
        let field = widget::search_width(
            ui.available_width(),
            fixed,
            widget::search_content(ui, hint),
            metric::SEARCH_FIELD,
        );

        let mut toggled = None;
        let mut adding = false;
        ui.horizontal_centered(|ui| {
            widget::search_field(ui, palette, &mut self.filter.query, hint, field);
            ui.add_space(metric::TOOL_GAP);
            toggled = chips(ui);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| adding = tail(ui));
        });

        if let Some(kind) = toggled {
            self.toggle_kind(kind);
        }
        if adding {
            self.add_files(ui);
        }
    }

    /// Above the catalog: what to show of it, and how much of it is already here.
    ///
    /// The Local toolbar's shape with the box at the right end exchanged.
    /// `Add files...` and the design's factory toggle are about documents on
    /// this machine; what somebody browsing needs instead is to be able to ask
    /// which of this list they already have - `CatalogToolbar.dc.html`.
    fn catalog_toolbar(
        &mut self,
        ui: &mut egui::Ui,
        states: &std::collections::BTreeMap<Uuid, Published>,
    ) {
        let palette = self.palette;
        ui.spacing_mut().item_spacing.x = 0.0;
        let Some(index) = self.index() else { return };

        // Counted against the install filter and against nothing else, which is
        // the shell's own arithmetic - `ORNG Registry.dc.html:672-675` facets
        // `installFiltered` rather than the drawn list. A facet answers "how
        // many would I see if I switched this kind on", so narrowing it by the
        // kind filter it is the control for would make every count read the
        // number the row already shows or nothing at all. The search does not
        // narrow it either, for the reason the Local toolbar's counts do not.
        let shown = self.filter.shown;
        let kinds: Vec<(Kind, usize, bool)> = Kind::ALL
            .into_iter()
            .map(|kind| {
                let count = index
                    .items
                    .iter()
                    .filter(|item| Kind::from(item.kind) == kind)
                    .filter(|item| {
                        let status = states.get(&item.uuid).expect(
                            "every published item was answered for before the bar was drawn",
                        );
                        shown.lets_through(status)
                    })
                    .count();
                (kind, count, self.filter.kinds.contains(&kind))
            })
            .collect();

        let width = self.width();
        // The design says what it searches, because it searches more than the
        // other one does. Beside the panel there is no room to say it, and the
        // placeholder falls back to the word the Local toolbar uses there too.
        let hint = match width {
            widget::Width::Full => "Search name, author, description or keywords",
            widget::Width::Narrow => "Search",
        };

        // Laid out twice for the reason the Local toolbar's are: the field
        // between them is the flexible one and cannot be given its share until
        // everything that is not flexible has taken its own.
        //
        // **At the full chip padding in both widths**, which is the one place
        // this bar and the Local one disagree: `CatalogToolbar.dc.html:43` takes
        // no `narrow` argument where `ListToolbar.dc.html:56` does. It has one
        // box fewer to fit, and the probe puts the field at 82 beside the panel
        // - still clear of the 80 a search field stops being worth having at.
        let chips = |ui: &mut egui::Ui| -> Option<Kind> {
            let mut toggled = None;
            for (at, (kind, count, on)) in kinds.iter().enumerate() {
                if at > 0 {
                    ui.add_space(metric::SNUG);
                }
                let label = plural(*kind);
                let chip =
                    widget::filter_chip(ui, palette, label, *count, *on, widget::Width::Full);
                if chip.clicked() {
                    toggled = Some(*kind);
                }
            }
            toggled
        };
        // The design's three, in the design's order, which is also the order of
        // how much they narrow: everything, what is here, what has moved on.
        const OFFERED: [(Shown, &str); 3] = [
            (Shown::All, "All"),
            (Shown::Installed, "Installed"),
            (Shown::Updatable, "Updatable"),
        ];
        let tail = |ui: &mut egui::Ui| widget::install_filter(ui, palette, shown, &OFFERED);

        // Four boxes and all four drawn, unlike the Local bar: the field, the
        // chips, the flexible gap, and the install filter.
        const BETWEEN_TOOLBAR_GROUPS: f32 = 3.0 * metric::TOOL_GAP;
        let chips_wide = widget::measured(ui, "catalog-chips", |ui| {
            chips(ui);
        });
        let filter_wide = widget::measured(ui, "catalog-tail", |ui| {
            tail(ui);
        });
        let fixed = chips_wide + filter_wide + BETWEEN_TOOLBAR_GROUPS;
        let field = widget::search_width(
            ui.available_width(),
            fixed,
            widget::search_content(ui, hint),
            metric::CATALOG_SEARCH_FIELD,
        );

        let mut toggled = None;
        let mut picked = None;
        ui.horizontal_centered(|ui| {
            widget::search_field(ui, palette, &mut self.filter.query, hint, field);
            ui.add_space(metric::TOOL_GAP);
            toggled = chips(ui);
            // The design's second flexible box, laid out rather than achieved by
            // reversing the row. A `right_to_left` wrapper is what the Local bar
            // ends with, and it cannot be what this one ends with: the install
            // filter is three controls, and a reversed layout draws them
            // `Updatable Installed All` - a switch whose answers are in the
            // order nobody wrote them in - while stretching the well behind them
            // across everything the row had left.
            ui.add_space((ui.available_width() - filter_wide).max(metric::FLEXIBLE_GAP_FLOOR));
            picked = tail(ui);
        });

        if let Some(kind) = toggled {
            self.toggle_kind(kind);
        }
        if let Some(shown) = picked {
            self.filter.shown = shown;
        }
    }

    /// Switch one kind filter, which both toolbars offer and neither owns.
    fn toggle_kind(&mut self, kind: Kind) {
        if !self.filter.kinds.remove(&kind) {
            self.filter.kinds.insert(kind);
        }
    }

    /// How much of the window the list has, which is the one thing a panel
    /// beside it changes about everything else.
    fn width(&self) -> widget::Width {
        let open = match self.view {
            View::Local => self.inspecting.is_some(),
            View::Catalog => self.detailing.is_some(),
        };
        if open { widget::Width::Narrow } else { widget::Width::Full }
    }

    /// The panel beside the list, whichever view is showing.
    ///
    /// One at a time, because they are one region of the window: the inspector
    /// in Local and the catalog's detail in Catalog, both 272 wide and both
    /// claimed before the page so that the list is laid out in what is left.
    fn aside(&mut self, ui: &mut egui::Ui) -> Option<widget::Aside> {
        match self.view {
            View::Local => self.inspect(ui),
            View::Catalog => self.detail(ui),
        }
    }

    /// Region two: whatever the current view has to show.
    ///
    /// `states` is every published item's state, worked out once for the frame
    /// by [`App::browse`] because the toolbar above this is drawn from it too.
    fn page(&mut self, ui: &mut egui::Ui, states: &std::collections::BTreeMap<Uuid, Published>) {
        match &self.session {
            Session::NoInstallation { searched } => {
                let body = format!(
                    "ORNG Registry looked in {searched}. Point it at the installation root \
                     if it lives somewhere else."
                );
                let empty = widget::Empty {
                    icon: widget::icon::NO_INSTALL,
                    inviting: false,
                    marks: true,
                    title: "No Bitwig Studio installation found",
                    body: &body,
                    extensions: false,
                    aside: None,
                    action: Some("Locate Bitwig Studio..."),
                    action_is_primary: true,
                    alt: Some("Copy diagnostics"),
                    foot: Some("The installation root contains bitwig.jar"),
                    minor: false,
                };
                match widget::empty_state(ui, self.palette, &empty) {
                    widget::Pressed::Action => self.locate(Which::Install),
                    widget::Pressed::Alt => ui.ctx().copy_text(body.clone()),
                    widget::Pressed::Nothing => {}
                }
            }
            // The state a user reaches the morning after a Bitwig release. It is
            // not an error they caused, so nothing can be listed and the region
            // is given over to saying what could not be read.
            Session::Unreadable { root, why } => {
                let (root, why) = (root.clone(), why.clone());
                let empty = widget::Empty {
                    icon: widget::icon::UNREADABLE,
                    inviting: false,
                    marks: true,
                    title: "This Bitwig installation could not be read",
                    body: "ORNG Registry finds what it needs by structure rather than by \
                           version number, and this installation is arranged in a way it does \
                           not recognise. That usually means a new Bitwig release. Nothing has \
                           been changed.",
                    extensions: false,
                    aside: None,
                    action: Some("Copy diagnostics"),
                    action_is_primary: true,
                    alt: Some("Change install..."),
                    foot: Some(
                        "The diagnostics report names what was looked for and what was found",
                    ),
                    minor: false,
                };
                match widget::empty_state(ui, self.palette, &empty) {
                    widget::Pressed::Action => ui.ctx().copy_text(format!("{root}\n{why}")),
                    widget::Pressed::Alt => self.locate(Which::Install),
                    widget::Pressed::Nothing => {}
                }
            }
            Session::Found(_) if self.view == View::Catalog => {
                let palette = self.palette;
                let width = self.width();
                let open = self.detailing;
                // Nothing is started here. Opening the window reads the kept
                // index and asks the catalog about it, so by the time this
                // region is drawn the answer is either in hand, on its way, or
                // already known to be no.
                // Taken after the list has been drawn, for the reason the
                // Local list takes its own: opening the panel changes how wide
                // every row is, and changing that half way down a list draws
                // the rest of it to a different grid.
                let pressed =
                    published(ui, palette, &self.catalog, width, open, states, &self.filter);
                if let Some(opened) = pressed.opened {
                    self.detailing = opened;
                }
                if let Some((uuid, offer)) = pressed.acted {
                    self.offer(uuid, offer, ui.ctx());
                }
                if pressed.cleared {
                    self.filter = Filter::default();
                }
                if pressed.retried {
                    self.catalog.refresh(ui.ctx());
                }
            }
            Session::Found(_) => self.local(ui),
        }

        // Over everything, including the bars, because the whole window is the
        // target and a drop is not aimed at a region of it.
        self.hovering(ui);
    }

    /// The Local view: what is pending, then what is registered.
    fn local(&mut self, ui: &mut egui::Ui) {
        let Session::Found(found) = &self.session else { return };
        let listing = Listing::of(&self.staged, found.entries().entries(), &self.filter);

        if found.entries().is_empty() && self.staged.is_empty() {
            // The primary onboarding surface, and the only screen whose icon
            // takes the accent: it is an invitation rather than a report.
            let empty = widget::Empty {
                icon: widget::icon::DROP,
                inviting: true,
                marks: true,
                title: "Drop a device here to register it",
                body: "ORNG Registry reads the document's identity and makes this installation \
                       recognise it. Bitwig Studio must be closed the first time, while the \
                       installation is prepared.",
                extensions: true,
                aside: Some(
                    "Nothing of your own yet? The catalog has devices, modulators and Grid \
                     modules you can install in one click.",
                ),
                action: Some("Browse the catalog"),
                action_is_primary: true,
                alt: Some("Add files..."),
                foot: Some("A backup is written before anything is changed"),
                minor: false,
            };
            match widget::empty_state(ui, self.palette, &empty) {
                widget::Pressed::Action => self.view = View::Catalog,
                widget::Pressed::Alt => self.add_files(ui),
                widget::Pressed::Nothing => {}
            }
            return;
        }

        if listing.is_empty() {
            let empty = widget::Empty {
                icon: widget::icon::NO_MATCH,
                inviting: false,
                marks: false,
                title: "No entries match",
                body: "Nothing here matches the current search and kind filters.",
                extensions: false,
                aside: None,
                action: Some("Clear filters"),
                action_is_primary: false,
                alt: None,
                foot: None,
                minor: true,
            };
            if widget::empty_state(ui, self.palette, &empty) == widget::Pressed::Action {
                self.filter = Filter::default();
            }
            return;
        }

        let Listing { pending, registered } = &listing;
        let palette = self.palette;
        let open = self.inspecting.as_ref().map(|open| open.uuid);
        let width = self.width();
        let document = self.deleting();
        // What the list was clicked on, taken after it has been drawn: opening
        // the panel changes how wide every row is, and changing that half way
        // down a list draws the rest of it to a different grid. A row control
        // is taken the same way and for a stronger reason - what several of
        // them do is add or remove a row, under the loop that is walking them.
        let mut opened = None;
        let mut pressed = None;
        widget::list(ui, |ui| {
            // Pending work first, which is the designer's recommendation and
            // the only ordering under which the list answers "what am I about to
            // do" without scrolling.
            if !pending.is_empty() {
                widget::section(ui, palette, "Pending", palette.accent_text, pending.len());
                for (at, row) in pending {
                    if let Some(action) = staged_row(ui, palette, width, row, document) {
                        pressed = Some((Acting::Pending(*at), action));
                    }
                }
            }
            widget::section(ui, palette, "Registered", palette.ink_2, registered.len());
            for entry in registered {
                let selected = open == Some(entry.uuid);
                let status = self.status_of(found, entry);
                let (response, action) =
                    row(ui, palette, width, selected, entry, status, document);
                if let Some(action) = action {
                    pressed = Some((Acting::Registered(entry.uuid), action));
                }
                if response.clicked() {
                    // The same row again closes it, which is what makes the
                    // panel answerable from the list it is about. The words
                    // are taken here, where the entry to take them off is in
                    // hand, so opening a panel is one statement.
                    opened = Some((!selected).then(|| Inspection::of(entry, found)));
                }
            }
        });
        if let Some(entry) = opened {
            self.inspecting = entry;
        }
        if let Some((on, action)) = pressed {
            self.act(on, action, ui.ctx());
        }
    }

    /// What the list says about one registered entry.
    ///
    /// One row says one thing, so these are in the order the design's own
    /// colours put them in. A queued removal comes first because it is what the
    /// user has just asked for, and a row about to stop existing has nothing
    /// useful to say about its file. Then what is broken, then what is waiting
    /// on a decision, and `Registered` when none of it applies.
    ///
    /// Nothing here touches the disk: the looking was done when the list was
    /// read, and this is the reading of it.
    /// The published index, whether it came off the network this run or off the
    /// disk.
    ///
    /// `None` covers the two states the callers treat alike - the fetch is
    /// still running, or nothing has ever verified here - and they are alike:
    /// in both this application knows nothing about what is published and must
    /// not say that anything is up to date either.
    fn index(&self) -> Option<&Index> {
        self.catalog.index()
    }

    /// Whether the catalog publishes a newer revision of this entry.
    ///
    /// Only a catalog entry can have one. A local file has nothing upstream, so
    /// a difference in it is the user's own change - which is the whole reason
    /// the entry list records where a document came from.
    ///
    /// Answered against the index in hand rather than by asking the network,
    /// so a window nobody has opened the catalog in says nothing about updates
    /// instead of reaching for a socket to draw a list.
    fn update_available(&self, entry: &Registration) -> bool {
        let Provenance::Catalog { version, .. } = &entry.provenance else { return false };
        // By identity and never by name. The catalog allows two items to share
        // a display name and this application renames an entry when they
        // collide, so matching on the name would eventually mark the wrong row
        // - and would do it first to the user who hit the rename.
        self.index()
            .and_then(|index| index.items.iter().find(|item| item.uuid == entry.uuid))
            .is_some_and(|published| published.version > *version)
    }

    fn status_of(&self, found: &Found, entry: &Registration) -> Status {
        if self.removing.contains(&entry.uuid) {
            return Status::PendingRemoval;
        }
        let standing = found.standing(entry.uuid);
        if !standing.placement().is_resolved() {
            return Status::MissingFile;
        }
        // Before an available update, and that order is the point of keeping
        // the two apart: updating a document somebody has edited discards the
        // edit, so the edit is what the row has to say first.
        if standing.content() == Content::Rewritten {
            return Status::Changed;
        }
        if self.update_available(entry) {
            return Status::UpdateAvailable;
        }
        // Last of the four, because the design's colours put it last: an update
        // is a decision waiting and this is only work in flight with nothing to
        // decide. A row that is both has the decision to state.
        if self.awaiting_restart.contains(&entry.uuid) {
            return Status::PendingRestart;
        }
        Status::Registered
    }

    /// Which of the design's seven states a published item is in, on this
    /// machine.
    ///
    /// Every one of them is a fact about this machine held against the index,
    /// which is why it is answered here rather than anywhere near the drawing:
    /// nothing in a signed index knows what is registered locally, and nothing
    /// registered locally knows what has been published since.
    ///
    /// The order is what the states mean rather than what they are called. A
    /// failed attempt outranks everything, because it is the only one of the
    /// seven that is about a press the user just made and is entitled to an
    /// answer about. After that the question is whether the item is here at all:
    /// an installed item that this Bitwig is too old for is still installed, and
    /// telling somebody they need a newer Bitwig for something already in their
    /// browser is telling them nothing they can act on.
    fn published_status(&self, found: &Found, item: &orng_catalog::IndexEntry) -> Published {
        if let Some(refused) = self.refused.get(&item.uuid) {
            return match refused {
                catalog::Refused::Download(_) => Published::DownloadFailed,
                catalog::Refused::Verification(_) => Published::VerificationFailed,
            };
        }

        // By identity and never by name, for the reason `update_available`
        // gives: the catalog allows two items to share a display name.
        if let Some(entry) = found.entries().get(item.uuid) {
            // An update before a replacement, because they are offers of
            // different sizes: an update keeps the identity and every project
            // that uses it, and a replacement is a different device the user has
            // to choose to adopt. The one that can be taken without a decision
            // is the one the row should be making.
            if self.update_available(entry) {
                return Published::UpdateAvailable;
            }
            if self.replacement_for(item.uuid).is_some() {
                return Published::Superseded;
            }
            return Published::Installed;
        }

        // A build that does not state its version is not evidence that the item
        // will not load, so it is not held against it - the same judgement the
        // detail panel makes about the line it draws.
        match &found.condition.build {
            Some(build) if build.version < item.min_bitwig => {
                Published::Incompatible(item.min_bitwig.clone())
            }
            _ => Published::Available,
        }
    }

    /// Which state every published item is in, keyed by identity.
    ///
    /// Worked out in one pass before anything is drawn, because a row's state is
    /// a fact about the whole machine and the whole index at once - what is
    /// registered, what has been superseded by something else in the list, what
    /// the last press came to - and a list that asked per row while it was being
    /// drawn would be asking `self` questions with `self` already borrowed.
    ///
    /// Empty when there is no index or no installation, which is the two states
    /// where there are no rows to say anything about.
    fn published_states(&self) -> std::collections::BTreeMap<Uuid, Published> {
        let (Session::Found(found), Some(index)) = (&self.session, self.index()) else {
            return std::collections::BTreeMap::new();
        };
        index
            .items
            .iter()
            .map(|item| (item.uuid, self.published_status(found, item)))
            .collect()
    }

    /// The published item that lists this one as replaced, if one does.
    ///
    /// A fact about the whole index rather than about the row, which is why both
    /// the row and the panel ask for it here instead of each walking the list.
    fn replacement_for(&self, uuid: Uuid) -> Option<(&str, Uuid)> {
        self.index()?
            .items
            .iter()
            .find(|other| other.supersedes.contains(&uuid))
            .map(|other| (other.name.as_str(), other.uuid))
    }

    /// What a removal does with the document, as the preferences have it.
    fn deleting(&self) -> TheDocument {
        if self.preferences.chosen().delete_file {
            TheDocument::Deleted
        } else {
            TheDocument::Kept
        }
    }

    /// The queued removals the list is actually showing as `Pending removal`.
    ///
    /// Filtered rather than trusted, on both counts, so that this answers
    /// exactly the rows the user can see struck through. Anything else is the
    /// press doing something no row said it would.
    ///
    /// Not in the list at all: the queue is what was asked for and the list is
    /// what is there, and handing [`Update::remove`] an identity it has no row
    /// for is a panic rather than a no-op.
    ///
    /// Staged under the same identity: re-dropping the document of a queued
    /// entry replaces its row with the staged one, which is what
    /// [`App::local`] does to every registered row a drop covers. The row now
    /// reads `Staged`, so the press must register it and not forget it.
    fn removals<'a>(&'a self, found: &'a Found) -> impl Iterator<Item = Uuid> + 'a {
        self.removing.iter().copied().filter(|uuid| {
            found.entries().get(*uuid).is_some()
                && !self
                    .staged
                    .iter()
                    .filter_map(Staged::registration)
                    .any(|staged| staged.uuid == *uuid)
        })
    }

    /// Carry out what a row's own control asked for.
    ///
    /// Taken after the list has been drawn, never during it: three of the five
    /// change which rows there are, and one of them changes how every remaining
    /// row reads.
    fn act(&mut self, on: Acting, action: Action, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        match (on, action) {
            // Nothing has been written for a staged row, so this is not a
            // removal at all and the design calls it Cancel: the row goes out
            // of the pending list and the file it was read from is untouched.
            // The rest are read again, because a row that collided with this
            // one no longer does.
            (Acting::Pending(at), Action::Remove) => {
                let kept = std::mem::take(&mut self.staged)
                    .into_iter()
                    .enumerate()
                    .filter(|(which, _)| *which != at)
                    .map(|(_, row)| row)
                    .collect();
                self.staged = staging::restaged(kept, found.entries(), &found.to);
            }
            (Acting::Pending(at), Action::Assign) => {
                let staged = std::mem::take(&mut self.staged);
                self.staged = staging::reassign(staged, at, found.entries(), &found.to);
            }
            // Queued, not done. The design keeps the entry registered and
            // struck through until the apply that removes it, which is what
            // makes one press of the primary action the confirmation for every
            // removal in the list.
            (Acting::Registered(uuid), Action::Remove) => {
                self.removing.insert(uuid);
            }
            (Acting::Registered(uuid), Action::Undo) => {
                self.removing.remove(&uuid);
            }
            // Where the session found it, which is where the row that offered
            // this control said it was. Asking the disk again on the press
            // would be a second answer to a question already answered, and one
            // the row was not drawn from.
            (Acting::Registered(uuid), Action::Reveal) => {
                if found.entries().get(uuid).is_some() {
                    reveal(found.standing(uuid).placement().path());
                }
            }
            (Acting::Registered(uuid), Action::Locate) => self.relocate(uuid, ctx),
            // The design's table gives each state its own controls and the two
            // kinds of row are never in the same state, so these combinations
            // cannot be pressed: a pending row has nothing placed to reveal or
            // locate and nothing registered to forget, and a registered one has
            // no identity left to mint. Reached means the table and the list
            // have come apart, which is worth the crash.
            (on, action) => unreachable!("{action:?} was offered on {on:?}"),
        }
    }

    /// Carry out what a catalog row's own control asked for.
    ///
    /// [`App::act`]'s opposite number, and separate from it for the reason the
    /// two tables are separate: not one of the five controls a registered row
    /// offers applies to something that is not registered.
    fn offer(&mut self, on: Uuid, offer: Offer, ctx: &egui::Context) {
        match offer {
            // A press on a row that failed is a press on the row as it will be
            // once it is tried again, so the failure goes before the attempt
            // starts. Leaving it would draw `Download failed` over a download
            // that is running.
            Offer::Install | Offer::Retry => {
                self.refused.remove(&on);
                self.install(on, ctx);
            }
            Offer::SeeReplacement => {
                self.detailing = self.replacement_for(on).map(|(_, uuid)| uuid);
            }
            Offer::CopyDetails => {
                if let Some(refused) = self.refused.get(&on) {
                    ctx.copy_text(refused.details().to_owned());
                }
            }
        }
    }

    /// Fetch a published item, prove it is the one the index described, and
    /// register it.
    ///
    /// The fetch is a worker of its own and the registration is the [`Applying`]
    /// every other write goes through - which is the whole of why installing
    /// needs no new machinery on this side. The design's own sentence: on a
    /// prepared installation this is *Update entries* work, so there is no
    /// backup, no confirmation and no requirement that Bitwig be closed.
    ///
    /// **Nothing starts on top of something already running**, for the reason
    /// `write_words` gives. An install that is queued behind a preparation would
    /// be an install nobody asked for by the time it ran.
    fn install(&mut self, uuid: Uuid, ctx: &egui::Context) {
        if self.applying.is_some() || self.installing.is_some() {
            return;
        }
        let Some(index) = self.index() else { return };
        let Some(item) = index.items.iter().find(|item| item.uuid == uuid) else { return };
        // The index's own commit, which is where the documents it describes
        // live. Not the item's `merged_in`: that names the change that published
        // the item and is what the panel links to, and the tree at that commit
        // is not the tree this index was built from.
        self.installing =
            Some(Install::start(item.clone(), index.revision.clone(), ctx.clone()));
    }

    /// Take what the fetch came back with, and write it or hold the refusal.
    ///
    /// Split out of [`App::pump`] because it is the seam between the two halves
    /// of an install, and the registration it starts has to be built from the
    /// row the digest was checked against rather than from whatever the catalog
    /// says now.
    fn installed(&mut self, finished: Install, ctx: &egui::Context) {
        let outcome = finished.outcome.expect("only a finished install is taken");
        let item = finished.item;
        let document = match outcome {
            Ok(document) => document,
            Err(refused) => {
                // A hash that does not match is a trust event and the design
                // says it must not read like a network error, so it is said out
                // loud as well as held on the row. An ordinary download failure
                // is not: it is per-item, it offers `Retry`, and a banner for
                // every dropped connection is a window talking over itself.
                if let catalog::Refused::Verification(why) = &refused {
                    self.outcome = Some(Outcome::Refused {
                        item: item.name.clone(),
                        why: why.clone(),
                    });
                }
                self.refused.insert(item.uuid, refused);
                return;
            }
        };

        let Session::Found(found) = &self.session else { return };
        let registration = match published_registration(&item, &document) {
            Ok(registration) => registration,
            // A published item this machine cannot register under: a name with a
            // tab in it, or a file name that will not make a library path. The
            // bytes verified, so this is the catalog carrying something the
            // entry list cannot hold, and nothing has been written.
            Err(why) => {
                self.outcome = Some(Outcome::Failed { what: Errand::Install, why: why.to_string() });
                return;
            }
        };

        let mut job = Job::against(Work::Entries, &found.to, found.entries());
        job.add(registration, &document);
        self.applying = Some(Applying::start(
            Errand::Install,
            found.to.clone(),
            job,
            found.rights.clone(),
            ctx.clone(),
        ));
    }

    /// Point a registered entry back at its document.
    ///
    /// The remedy for `Missing file`, and the only row control that reads a
    /// file. **The recorded registration is kept** and only the document is
    /// placed: the entry still exists, its description and search keywords may
    /// have been edited since it was registered, and deriving them again from
    /// whatever file was found would quietly undo that. This is the one thing
    /// locating a file is not - it is not a re-drop, which is how an *edited*
    /// document is re-applied and which does replace the words.
    ///
    /// **The file has to carry this entry's identity.** Anything else is
    /// somebody pointing at the wrong document, and placing it under this
    /// registration would put one device into the browser under another's name.
    fn relocate(&mut self, uuid: Uuid, ctx: &egui::Context) {
        // Nothing starts on top of something already running, for the reason
        // `write_words` gives: the window has one piece of work at a time.
        if self.applying.is_some() {
            return;
        }
        let Session::Found(found) = &self.session else { return };
        let Some(entry) = found.entries().get(uuid) else { return };
        let Some(chosen) = rfd::FileDialog::new()
            .set_title(format!("Locate the document for {}", entry.name))
            .add_filter("Bitwig documents", &[entry.kind().extension()])
            .pick_file()
        else {
            return;
        };

        let document = match this_entrys_document(entry, &chosen) {
            Ok(document) => document,
            // The same banner a failed run gets, because what the user is
            // entitled to know is the same either way: nothing was changed, and
            // the entry is still missing its document.
            Err(why) => {
                self.outcome = Some(Outcome::Failed { what: Errand::Locate, why });
                return;
            }
        };

        let mut job = Job::against(Work::Entries, &found.to, found.entries());
        job.add(entry.clone(), &document);
        self.applying = Some(Applying::start(
            Errand::Locate,
            found.to.clone(),
            job,
            found.rights.clone(),
            ctx.clone(),
        ));
    }

    /// The drop target, while something is over the window.
    ///
    /// **Not while a run is in flight**, when [`App::read`] refuses the drop.
    /// A target that listed the documents and said `Drop to stage` would be
    /// inviting a drop the window then says nothing about, and no target at
    /// all is the ordinary way a window says it takes nothing just now.
    fn hovering(&mut self, ui: &mut egui::Ui) {
        let paths: Vec<PathBuf> = ui.ctx().input(|input| {
            input.raw.hovered_files.iter().filter_map(|file| file.path.clone()).collect()
        });
        if paths.is_empty() || self.applying.is_some() {
            self.hovered = None;
            return;
        }
        if self.hovered.as_ref().is_none_or(|hovered| hovered.paths != paths) {
            self.hovered = Some(Hovered::over(paths));
        }
        let hovered = self.hovered.as_ref().expect("set a moment ago");
        widget::drop_target(ui, self.palette, &hovered.heading, &hovered.files);
    }

    /// Region three: what one press would do, and the press.
    fn action_bar(
        &mut self,
        ui: &mut egui::Ui,
        states: &std::collections::BTreeMap<Uuid, Published>,
    ) {
        let palette = self.palette;
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.horizontal_centered(|ui| {
            // The action is placed first, from the right. Laying the summary out
            // first leaves the button whatever width is left over, and a summary
            // is long enough that there is none: the button then hangs off the
            // edge of the window.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                self.action(ui);
                ui.add_space(metric::GAP);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    let (summary, tone, note) = self.summary(states);
                    widget::centred_block(ui, |ui| {
                        ui.add(
                            egui::Label::new(
                                font::run(&summary, font::plain(font::CONTROL))
                                    .color(tone.colour(palette)),
                            )
                            .truncate(),
                        );
                        if !note.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    font::run(&note, font::plain(font::NOTE))
                                        .color(palette.ink_3),
                                )
                                .truncate(),
                            );
                        }
                    });
                });
            });
        });
    }

    /// What one press would do, in words, beside the button that would do it.
    fn summary(
        &self,
        states: &std::collections::BTreeMap<Uuid, Published>,
    ) -> (String, Tone, String) {
        let found = match &self.session {
            Session::Found(found) => found,
            Session::Unreadable { .. } => {
                return (
                    "Installation not recognised".to_owned(),
                    Tone::Warn,
                    "Nothing can be registered until this build can be read.".to_owned(),
                );
            }
            Session::NoInstallation { .. } => {
                return ("No installation selected".to_owned(), Tone::Warn, String::new());
            }
        };
        if let Some(reading) = &self.reading {
            let count = reading.count;
            return (format!("Reading {count} documents"), Tone::Neutral, String::new());
        }
        // Only while it is in flight. What a press came to is a banner, and the
        // bar goes back to saying what the next press would do.
        if let Some(applying) = &self.applying {
            return match applying.stage {
                Stage::Preparing => {
                    ("Preparing the installation".to_owned(), Tone::Warn, String::new())
                }
                Stage::Registering => ("Registering".to_owned(), Tone::Warn, String::new()),
            };
        }

        // Everything above is true of the window rather than of a list, which
        // is why it comes first: an installation that cannot be read is not
        // readable from the catalog either, and the second half of an install
        // is the same `Registering` a drop's is. Below here the two views are
        // counting different things.
        if self.view == View::Catalog {
            return self.catalog_summary(states);
        }

        // The design's own order and wording: "2 to add, 1 to remove, 2 to
        // fix". Each part is left out when it is nothing, so the line never
        // says a count of zero.
        let mut parts = Vec::new();
        let ready = self.ready().count();
        if ready > 0 {
            parts.push(format!("{ready} to add"));
        }
        let to_remove = self.removals(found).count();
        if to_remove > 0 {
            parts.push(format!("{to_remove} to remove"));
        }
        let to_fix = self.staged.len() - ready;
        if to_fix > 0 {
            parts.push(format!("{to_fix} to fix"));
        }
        // What the press costs, which is the difference between the two modes
        // and the thing a user is entitled to know before pressing rather than
        // after.
        let separator = widget::SEPARATOR;
        let (note, tone) = match self.pending(found) {
            Some(Work::PrepareThenEntries) => (
                format!("Prepare install {separator} a backup is written first"),
                Tone::Warn,
            ),
            Some(Work::Entries) => {
                (format!("Update entries {separator} Bitwig may stay open"), Tone::Neutral)
            }
            None => (String::new(), Tone::Neutral),
        };
        // `:490`, the one Local scenario whose note is not what a press costs.
        // A filter that has left nothing on screen takes the line: the empty
        // state under it offers the way out but states no number, and how much
        // of the list is behind the filter is what says whether clearing it is
        // worth doing. The cost is not lost with the note - the summary keeps
        // the warn tone that only the preparing mode takes, and the button goes
        // on saying `Prepare installation` in so many words.
        let note = match self.emptied_by_filter(found) {
            Some(1) => "1 entry hidden by the current filter".to_owned(),
            Some(hidden) => format!("{hidden} entries hidden by the current filter"),
            None => note,
        };
        if !parts.is_empty() {
            return (parts.join(", "), tone, note);
        }
        // Nothing is staged. What the press is *for* then depends on the mode:
        // preparing puts the entries already on record back into effect, which
        // is the second most common session there is, and saying "nothing
        // pending" beside an enabled button that does something would be wrong.
        let registered = found.entries().entries().len();
        match (self.pending(found), registered) {
            (Some(Work::PrepareThenEntries), 0) => (
                "Nothing staged yet".to_owned(),
                Tone::Neutral,
                "Drop documents onto the window, or use Add files...".to_owned(),
            ),
            (Some(Work::PrepareThenEntries), 1) => {
                ("1 entry to restore".to_owned(), tone, note)
            }
            (Some(Work::PrepareThenEntries), many) => {
                (format!("{many} entries to restore"), tone, note)
            }
            _ => ("Nothing pending".to_owned(), Tone::Neutral, note),
        }
    }

    /// The same line for the Catalog view, which has no pending work to count.
    ///
    /// Nothing in the catalog is staged and nothing is queued: an install is one
    /// press on one row, so the arithmetic the Local bar runs on has no answer
    /// here and the bar would say `Nothing pending` over a list of nine things
    /// to install. What the design puts there instead is how big the catalog is
    /// and what a press would cost - `ORNG Registry.dc.html:433`.
    ///
    /// **The count is taken against the install filter and against nothing
    /// else**, which is the same arithmetic the toolbar's facets run on and the
    /// shell's own: `:433` reads `Catalog {SEP} 9 items` over a nine-item index,
    /// `:439` reads `Catalog {SEP} 1 update available` over the same nine with
    /// the `Updatable` filter pressed, and `:486` still reads nine with a search
    /// in the field that matches none of them. So the search and the kind chips
    /// do not move this number - the summary says how big the catalog is, and
    /// the note says what the filters did to it.
    fn catalog_summary(
        &self,
        states: &std::collections::BTreeMap<Uuid, Published>,
    ) -> (String, Tone, String) {
        let separator = widget::SEPARATOR;
        let status = |item: &orng_catalog::IndexEntry| {
            states.get(&item.uuid).expect("every published item was answered for this frame")
        };

        // The first half of an install, which is the one piece of work in this
        // view that writes nothing - hence the neutral tone the reading of a
        // drop gets, where preparing and registering take the warn. Not drawn in
        // the bundle, which has no state for a fetch in flight; without it the
        // window says nothing at all for the second or two after the press.
        if let Some(fetching) = &self.installing {
            let name = &fetching.item.name;
            return (format!("Fetching {name}"), Tone::Neutral, String::new());
        }
        // A refusal outranks the count, for the reason `published_status` puts
        // it first among the seven: it is the only thing here about a press the
        // user just made, and the one they are owed an answer about. One line
        // for both kinds, which is the bundle's own - `:481` states
        // `Install refused` over a download failure and a verification failure
        // together, because what the bar has to say is that nothing was
        // installed and the rows say which was which.
        if !self.refused.is_empty() {
            return ("Install refused".to_owned(), Tone::Err, String::new());
        }

        match self.catalog.index() {
            // Nothing has ever verified here. The fetch is still running, or it
            // has answered and the answer was no - and the two are one line
            // apart, because an index that did not arrive and one that did not
            // verify read the same here too. The bar has one line and the
            // difference is in the empty state's own words. `:474` and its warn
            // tone.
            None if self.catalog.failure().is_some() => {
                ("Catalog unavailable".to_owned(), Tone::Warn, String::new())
            }
            None => ("Fetching the catalog".to_owned(), Tone::Neutral, String::new()),
            // No note: what installing costs is not worth saying over a list
            // with nothing in it to install.
            Some(index) if index.items.is_empty() => {
                (format!("Catalog {separator} 0 items"), Tone::Neutral, String::new())
            }
            Some(index) => {
                let counted = index
                    .items
                    .iter()
                    .filter(|item| self.filter.shown.lets_through(status(item)))
                    .count();
                // The filter names what is being counted, so it names the word.
                // Two of the three are the bundle's own; `Installed` is the same
                // sentence for the one state it captions no scenario for.
                let what = match self.filter.shown {
                    Shown::All if counted == 1 => "1 item".to_owned(),
                    Shown::All => format!("{counted} items"),
                    Shown::Installed => format!("{counted} installed"),
                    Shown::Updatable if counted == 1 => "1 update available".to_owned(),
                    Shown::Updatable => format!("{counted} updates available"),
                };

                let showing = index
                    .items
                    .iter()
                    .filter(|item| self.filter.accepts_published(item, status(item)))
                    .count();
                // What the filters did, then what being offline costs, then what
                // a press would cost. The first is the design's answer to a list
                // that has gone empty under the user - `:486`, where the summary
                // goes on stating the whole catalog and the note carries the
                // absence - and it stays first because it is about something the
                // user just typed. The second is `:468`.
                let note = if showing == 0 {
                    "No item matches the current search and filters".to_owned()
                } else if self.catalog.is_cached() {
                    format!("Offline {separator} installing a cached item still works")
                } else if self.filter.shown == Shown::Updatable {
                    // `:439`. The one sentence that matters about an update, on
                    // the filter that is looking for them: it reaches backwards
                    // into projects already saved, where an install does not.
                    "An update changes the device in projects that already use it".to_owned()
                } else {
                    // `:433`. Which mode a press runs in is the thing a user is
                    // entitled to know before pressing, and it is the Local
                    // bar's own note in catalog words.
                    let cost = "no backup, Bitwig may stay open";
                    format!("Installing is Update entries work {separator} {cost}")
                };
                // `:468` again, and it goes on the summary rather than only in
                // the note because the count is what it qualifies: nine items
                // as of some point in the past is a different claim from nine
                // items.
                let cached = match self.catalog.is_cached() {
                    true => format!(" {separator} cached"),
                    false => String::new(),
                };
                (format!("Catalog {separator} {what}{cached}"), Tone::Neutral, note)
            }
        }
    }

    /// How many entries the filter is keeping off the Local list, when what it
    /// did was empty it.
    ///
    /// `None` while anything is still on screen, which is the whole of the rule:
    /// the bar has one note line, and a list that still has rows in it does not
    /// need to be told what is missing from it. It is the state the bundle draws
    /// this for and the only one - `:490` is the single Local scenario whose
    /// note is not the cost of a press.
    ///
    /// **A count and not a sentence**, where the catalog's answer to the same
    /// state is a sentence: `catalog_summary` already states how big the catalog
    /// is in the line above, so there is nothing left for a number to add, and a
    /// browsed list's size is not something the user knew before they looked.
    /// Their own list is, and the count is what reconciles it with an empty
    /// screen.
    ///
    /// **The arithmetic is [`App::local`]'s**, and it has to be: a count taken
    /// over a pool the list does not draw from states a number that clearing the
    /// filter would not produce. So a staged document shadows the registered
    /// entry it updates here as it does there, and a dropped file that is not a
    /// document counts as shown - it carries no registration, so a filter over
    /// names and identities has nothing to hide it by.
    fn emptied_by_filter(&self, found: &Found) -> Option<usize> {
        let staged: Vec<&Registration> =
            self.staged.iter().filter_map(Staged::registration).collect();
        let drawn = found
            .entries()
            .entries()
            .iter()
            .filter(|entry| !staged.iter().any(|pending| pending.uuid == entry.uuid))
            .map(|entry| self.filter.accepts(entry))
            .chain(
                self.staged
                    .iter()
                    .map(|row| row.registration().is_none_or(|row| self.filter.accepts(row))),
            );
        let mut shown = 0usize;
        let mut hidden = 0usize;
        for accepted in drawn {
            if accepted {
                shown += 1;
            } else {
                hidden += 1;
            }
        }
        (shown == 0 && hidden > 0).then_some(hidden)
    }

    /// The one button, and what it would do.
    fn action(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let found = match &self.session {
            Session::Found(found) => found,
            // Preparing, and not applying, in both of these. Nothing is
            // registered on an installation that cannot be read or has not been
            // found, so preparation is what the press would be for once the
            // state is resolved - which is what the bundle labels them.
            Session::Unreadable { .. } => {
                let why = "This installation could not be read";
                widget::primary_button(ui, palette, PREPARE, icon::PREPARE, false, why);
                return;
            }
            Session::NoInstallation { .. } => {
                let why = "No installation selected";
                widget::primary_button(ui, palette, PREPARE, icon::PREPARE, false, why);
                return;
            }
        };

        if self.applying.is_some() {
            widget::primary_button(ui, palette, "Applying", icon::APPLY, false, "In progress");
            return;
        }

        let pending = self.pending(found);
        let label = match pending {
            Some(Work::PrepareThenEntries) => PREPARE.to_owned(),
            Some(Work::Entries) => match self.changes(found) {
                1 => "Apply 1 change".to_owned(),
                many => format!("Apply {many} changes"),
            },
            None => "Apply changes".to_owned(),
        };

        // An arrow when the press leads somewhere - a confirmation, a step
        // list - and a tick when it simply does the thing.
        let mark = match pending {
            Some(Work::PrepareThenEntries) => icon::PREPARE,
            _ => icon::APPLY,
        };
        let Some(work) = pending else {
            widget::primary_button(ui, palette, &label, mark, false, "Nothing to apply");
            return;
        };
        // Preparing an installation that would then read an empty list is work
        // with no result. The design disables the press and says so, rather
        // than letting a first-run user modify their installation for nothing.
        if work == Work::PrepareThenEntries && found.entries().is_empty() && self.changes(found) == 0
        {
            widget::primary_button(ui, palette, &label, mark, false, "Nothing to register yet");
            return;
        }
        // Only preparation is blocked by a running Bitwig or an unrecognised
        // guard, and the banner above has already said which. The button states
        // it too, because a disabled control with no reason on it is not a
        // statement.
        if let Some(blocked) = self.blocking() {
            widget::primary_button(ui, palette, &label, mark, false, blocked.title);
            return;
        }
        if widget::primary_button(ui, palette, &label, mark, true, "").clicked() {
            match work {
                // The one press that confirms. Everything the plan says is
                // worked out as it is drawn; what the disk has to be asked is
                // asked now, once.
                Work::PrepareThenEntries => self.confirming = Some(Confirming::read(found)),
                Work::Entries => self.start(work, ui.ctx()),
            }
        }
    }

    /// The plan, over the window, until it is answered.
    fn confirm(&mut self, ui: &mut egui::Ui) {
        let (Session::Found(found), Some(confirming)) = (&self.session, &self.confirming) else {
            return;
        };
        let plan = self.plan(found, confirming);
        let answer = widget::confirmation_dialog(
            ui,
            self.palette,
            &widget::Confirmation {
                title: "Prepare this installation",
                tag: "Plan",
                lead: "This is the one operation that modifies Bitwig Studio itself.",
                plan: &plan,
                note: "A Bitwig update resets the installation. Prepare it again afterwards. \
                       Your registered devices are kept.",
                cancel: "Cancel",
                primary: PREPARE,
                icon: icon::PREPARE,
            },
        );
        match answer {
            Some(widget::Answer::Proceed) => {
                self.confirming = None;
                self.start(Work::PrepareThenEntries, ui.ctx());
            }
            Some(widget::Answer::Cancel) => self.confirming = None,
            None => {}
        }
    }

    /// The plan, line by line, as the design numbers it -
    /// `ORNG Registry.dc.html:386`.
    ///
    /// Six lines at most, and never a count of zero: a line about nothing is
    /// left out, as the action bar leaves out a part that is nothing. The order
    /// is the design's - the backup, the archive, what is registered, what is
    /// removed, where the documents go, and the links - which is the order the
    /// press runs in, less the description bundles, which every press rewrites
    /// whatever else it does and the design draws no line for.
    ///
    /// Three things are said here that the bundle's one scenario does not say,
    /// each because the data says it: a backup that already exists is kept
    /// rather than written, a removal names whether the file goes with it in
    /// the words its own control used, and the links are counted rather than
    /// assumed to be one. `design-review.md` round 3 item 8.
    fn plan(&self, found: &Found, confirming: &Confirming) -> Vec<widget::PlanLine> {
        let plain = |text: String| widget::PlanLine { text, modifies_the_installation: false };
        let mut lines = Vec::new();

        // Where the pristine copy is, or goes. A build that could not be named
        // has no directory of its own, and the press will refuse before it
        // writes one; the root is what there is to say.
        let backups = match &found.condition.build {
            Some(build) => Backup::location(&found.to.home, build).directory().to_path_buf(),
            None => found.to.home.backups(),
        };
        let backups = format!("{}/", diagnostics::under_home(found, &backups));
        lines.push(plain(if confirming.backup_exists {
            format!(
                "The archive and the description bundles are already backed up in {backups}, \
                 and the patch is built from that copy."
            )
        } else {
            format!("The archive and the description bundles are backed up first to {backups}.")
        }));

        lines.push(widget::PlanLine {
            text: "The patched archive is written beside the original, verified under \
                   Bitwig's own JVM, then moved into place by a single rename. Nothing in \
                   the installation changes until it verifies."
                .to_owned(),
            modifies_the_installation: true,
        });

        let registered: Vec<&str> =
            self.ready().filter_map(Staged::registration).map(|row| row.name.as_str()).collect();
        match registered.len() {
            0 => {}
            1 => lines.push(plain(format!("1 entry registered: {}.", registered[0]))),
            many => {
                lines.push(plain(format!("{many} entries registered: {}.", registered.join(", "))))
            }
        }

        let removed: Vec<&str> = self
            .removals(found)
            .filter_map(|uuid| found.entries().get(uuid))
            .map(|entry| entry.name.as_str())
            .collect();
        if !removed.is_empty() {
            let what = match removed.len() {
                1 => format!("1 entry removed: {}.", removed[0]),
                many => format!("{many} entries removed: {}.", removed.join(", ")),
            };
            lines.push(plain(format!("{what} {}.", removal_consequence(self.deleting()))));
        }

        // Per kind, because one kind's folder cannot hold another's documents,
        // and in the order the kinds are always listed in.
        let folders: Vec<String> = Kind::ALL
            .into_iter()
            .filter_map(|kind| {
                let count = self
                    .ready()
                    .filter_map(Staged::registration)
                    .filter(|row| row.kind() == kind)
                    .count();
                (count > 0).then(|| {
                    format!("{count} to {}/{}", kind.library_subdir(), kind.user_folder())
                })
            })
            .collect();
        if !folders.is_empty() {
            let placed = match found.to.placement {
                Strategy::Link => "Placed in the user library",
                Strategy::Copy => "Copied into the installation",
            };
            lines.push(plain(format!("{placed}: {}.", folders.join(", "))));
        }

        // The step the copy strategy rules out has no line under it, as it has
        // no number in the progress list.
        if found.to.placement == Strategy::Link {
            lines.push(plain(match confirming.links_to_create {
                0 => "The installation's library folders are already linked to the user \
                      library."
                    .to_owned(),
                1 => "1 library link created inside the installation's Library folder, once \
                      the archive is in place."
                    .to_owned(),
                many => format!(
                    "{many} library links created inside the installation's Library folder, \
                     once the archive is in place."
                ),
            }));
        }
        lines
    }

    /// Hand the pending work to a thread that is not this one.
    fn start(&mut self, work: Work, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        let mut job = Job::against(work, &found.to, found.entries());
        // Only the rows that are ready. A conflict is pending work the user has
        // to resolve, and writing it would be resolving it for them.
        //
        // Cloned rather than taken. The rows stay in the list until the write
        // succeeds, so a failure leaves the same pending work rather than
        // asking the user to find the files again; a document is tens of
        // kilobytes and the copy is not worth avoiding at that price.
        for staged in &self.staged {
            if let crate::staging::State::Ready { registration, document } = &staged.state {
                job.add(registration.clone(), document);
            }
        }
        // And the rows the user asked to be rid of, which have been struck
        // through in the list since they were queued. The document goes with
        // them or does not, as the preference in force says, and the row's own
        // control named that preference when it was pressed.
        let document = self.deleting();
        for uuid in self.removals(found).collect::<Vec<_>>() {
            job.remove(uuid, document);
        }
        // What the last press came to is not what this one will come to.
        self.outcome = None;
        self.applying = Some(Applying::start(
            work.into(),
            found.to.clone(),
            job,
            found.rights.clone(),
            ctx.clone(),
        ));
    }
}

/// What the plan says that the disk has to be asked about, read once when the
/// confirmation opens rather than on every frame it is drawn - the rule the
/// Restore screen's list already follows.
struct Confirming {
    /// Whether `~/.orng/backups` already holds this build's pristine copy. The
    /// preparation then keeps it and patches from it rather than writing one -
    /// the decision is [`orng_tools::Plan`]'s, and the confirmation is where it
    /// is said in advance.
    backup_exists: bool,
    /// How many of the three library links the preparation will create. Fewer
    /// than three after a restore, which puts the archive back and leaves the
    /// links standing.
    links_to_create: usize,
}

impl Confirming {
    fn read(found: &Found) -> Confirming {
        let backup_exists = found
            .condition
            .build
            .as_ref()
            .is_some_and(|build| Backup::location(&found.to.home, build).exists());
        let links_to_create = Kind::ALL
            .into_iter()
            .filter(|kind| !placement::is_linked(&found.to.install, &found.to.library, *kind))
            .count();
        Confirming { backup_exists, links_to_create }
    }
}

/// What the drop overlay says about one set of paths under the pointer.
struct Hovered {
    /// The paths this was worked out for, so a different set is noticed.
    paths: Vec<PathBuf>,
    files: Vec<widget::Hovering>,
    heading: String,
}

impl Hovered {
    /// Accept and reject are stated before the drop, from the name alone,
    /// because that is all there is to go on while the file is still the
    /// operating system's. Every file is named, refused ones included: a
    /// listing of what will be taken cannot be checked against what the pointer
    /// is carrying.
    ///
    /// Each folder is listed once. The heading's count is the sum of the rows',
    /// which is what [`staging::documents_in`] over the whole set would say,
    /// because it takes each path on its own.
    fn over(paths: Vec<PathBuf>) -> Hovered {
        let mut staging = 0;
        let files = paths
            .iter()
            .map(|path| {
                let documents = staging::documents_in(std::slice::from_ref(path)).len();
                staging += documents;
                over(path, documents)
            })
            .collect();
        let heading = match staging {
            0 => "Nothing here can be registered".to_owned(),
            1 => "Drop to stage 1 document".to_owned(),
            many => format!("Drop to stage {many} documents"),
        };
        Hovered { paths, files, heading }
    }
}

/// One path under the pointer, as the drop overlay states it, given how many
/// documents a drop of it would read.
///
/// A folder is named as a folder and counted, rather than unfolded into the
/// documents inside it: what the user is dragging is the folder, and a listing
/// that says something else cannot be checked against the pointer.
fn over(path: &Path, documents: usize) -> widget::Hovering {
    let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
    if Kind::from_path(path).is_some() {
        return widget::Hovering { name, note: String::new(), accepted: true };
    }
    match documents {
        0 => widget::Hovering { name, note: "ignored".to_owned(), accepted: false },
        1 => widget::Hovering {
            name: format!("{name}/"),
            note: "1 file".to_owned(),
            accepted: true,
        },
        many => widget::Hovering {
            name: format!("{name}/"),
            note: format!("{many} files"),
            accepted: true,
        },
    }
}

/// The installation's name, the loudest thing in the window.
fn title(ui: &mut egui::Ui, text: &str, ink: egui::Color32) {
    ui.label(font::run(text, font::emphasis(ui.ctx(), font::INSTALL_TITLE)).color(ink));
}

/// Where it is, truncated, and whole on hover.
fn path(ui: &mut egui::Ui, palette: Palette, root: &str) {
    ui.add(
        egui::Label::new(font::run(root, font::mono(font::MONO)).color(palette.ink_3))
            .truncate(),
    )
    .on_hover_text(root);
}

/// What state the registry is in, coloured as the design colours it.
fn badge(ui: &mut egui::Ui, palette: Palette, label: &str) {
    ui.label(
        font::run(label, font::plain(font::CHIP))
            .color(widget::badge_colour(palette, label)),
    );
}

/// The one press that modifies Bitwig Studio itself, named the same wherever it
/// is offered and wherever it is refused.
const PREPARE: &str = "Prepare installation";

/// Point at a directory. Named once, because Settings offers it twice and the two
/// rows must not come to call the same control different things.
const BROWSE: &str = "Browse";

/// The two placements, as the design words them: what each does, and what a
/// Bitwig update then costs.
///
/// The consequence is the whole of the choice, so it is in the copy rather than
/// left to be discovered after the next release - which is the one moment the
/// difference between these two is visible at all.
const PLACEMENTS: [(Strategy, &str, &str); 2] = [
    (
        Strategy::Link,
        "Place documents in the user library and link them",
        "Documents survive a Bitwig update. Only the installation has to be prepared again.",
    ),
    (
        Strategy::Copy,
        "Copy documents into the installation",
        "A Bitwig update removes the copies. Everything has to be registered again.",
    ),
];

/// What the delete-file default means, in the design's own words.
///
/// Louder when it is on, because what it turns on is irreversible from here and
/// the file is the user's own work rather than anything this application made.
const DELETES_THE_FILE: &str =
    "On, removing an entry also deletes your document from the library. That file is your own \
     work, and deleting it cannot be undone from here.";
const KEEPS_THE_FILE: &str =
    "Off, removing an entry unregisters it and leaves your document in the library.";

/// Show the backups directory in the system's own file manager. Offered from the
/// overflow and from the Restore screen's header, and named once for the reason
/// [`BROWSE`] is.
const OPEN_BACKUPS: &str = "Open backups folder";

/// What restoring costs, and what it does not, in the design's own words.
///
/// Two lines because one would not do: the first is the whole of the loss and
/// the second is the whole of the reassurance, and a user deciding needs both
/// before they press.
const RESTORE_COSTS: &str = "Restoring removes every registration from this installation.";
const RESTORE_KEEPS: &str =
    "Projects that use custom devices will not recall them afterwards. Your documents and ORNG \
     Registry's own record are kept, so the installation can be prepared again.";

/// The Restore screen with nothing on it, which is every machine before its
/// first preparation.
///
/// No corner marks: those say "this region is where something would be", and
/// this region is inside a screen that already has a header and a bar at the
/// foot saying the same thing. No action either - what would fill this screen is
/// a preparation, which is two screens away, and an offer that led there would
/// be offering to change the installation from the screen for putting it back.
const NOTHING_TO_RESTORE: widget::Empty<'static> = widget::Empty {
    icon: icon::NO_BACKUP,
    inviting: false,
    marks: false,
    title: "Nothing to restore yet",
    body: "A backup is written just before the installation is prepared, and this installation \
           has not been prepared. There is nothing here until then.",
    extensions: false,
    aside: None,
    action: None,
    action_is_primary: false,
    alt: None,
    foot: Some(
        "Backups hold the jar and the three description bundles - about 35 MB, not the whole \
         installation.",
    ),
    minor: false,
};

/// What this application calls itself. The design writes it in full on the About
/// screen, where `CARGO_PKG_NAME` is the binary beside it.
const PRODUCT_NAME: &str = "ORNG Registry";

/// What it does, in one paragraph, for somebody who has just been handed it.
const WHAT_THIS_IS: &str =
    "Registers custom devices, modulators and Grid modules with a Bitwig Studio installation, so \
     projects recall them reliably. It finds what it needs by structure rather than by version, \
     so an unseen Bitwig release either works or fails loudly.";

/// Whose trademark this is, and whose installation is being modified.
const NOT_AFFILIATED: &str =
    "Not affiliated with or endorsed by Bitwig GmbH. Bitwig Studio is a trademark of Bitwig \
     GmbH. This tool modifies a local installation at your own discretion, and writes a backup \
     before it does.";

/// The line at the foot of the Restore screen, which names the copy the press
/// would put back.
///
/// The design's own sentence and its own split: the day out of the moment, so
/// the foot says which backup without repeating the time that is already on the
/// row above it.
fn foot_note(backups: &Backups) -> String {
    match backups.chosen_day() {
        Some(day) => format!("Restores {day} wholesale. Bitwig Studio must be closed."),
        None => "No backup exists for this installation.".to_owned(),
    }
}

/// The glyph on each segment of the appearance switch.
///
/// Here rather than on [`Appearance`] itself: the design's choice of a desktop, a
/// sun and a moon is the design's, and the preferences file has no business
/// knowing what an icon is.
fn appearance_icon(appearance: Appearance) -> &'static str {
    match appearance {
        Appearance::System => icon::FOLLOW_SYSTEM,
        Appearance::Light => icon::LIGHT,
        Appearance::Dark => icon::DARK,
    }
}

/// Roughly how wide a character of the badge is, for leaving room before it has
/// been laid out. An estimate, and only ever used to decide how much of the
/// path to show.
const BADGE_WIDTH_PER_CHAR: f32 = 6.0;

/// The plural the kind filters are labelled with. `Modules` and not `Grid
/// modules`, because the toolbar is tight and the design labels them so.
fn plural(kind: Kind) -> &'static str {
    match kind {
        Kind::Device => "Devices",
        Kind::Modulator => "Modulators",
        Kind::Module => "Modules",
    }
}

/// A preparation, step by step, over the window it is being done to.
fn progress(ui: &mut egui::Ui, palette: Palette, applying: &Applying) {
    use crate::work::State;
    let steps: Vec<widget::StepLine<'_>> = applying
        .steps
        .iter()
        .flatten()
        .map(|(step, state)| widget::StepLine { label: step_label(*step), state: *state })
        .collect();

    // Numbered over the steps this plan runs, because a plan that skips one
    // must not be five of four.
    let running = steps.len() - steps.iter().filter(|s| s.state == State::NotRun).count();
    let done = steps.iter().filter(|s| s.state == State::Done).count();
    let at = steps.iter().position(|s| s.state == State::Running);
    let step = match at {
        Some(at) => format!(
            "Step {} of {running} {} {}",
            done + 1,
            widget::SEPARATOR,
            steps[at].label
        ),
        // Between the last step and the end of the entry write there is no step
        // to name, and the stage is what is left to say.
        None => match applying.stage {
            Stage::Preparing => "Working out what has to be done".to_owned(),
            Stage::Registering => "Registering the entries".to_owned(),
        },
    };

    widget::progress_dialog(
        ui,
        palette,
        &widget::Progress {
            title: "Preparing the installation",
            step: &step,
            steps: &steps,
            note: "Nothing in the installation changes until the patched archive verifies. It \
                   is written beside the original, and moved into place by a single rename.",
            through: done as f32 / running.max(1) as f32,
        },
    );
}

/// The wording of each step is the application's, not the library's.
fn step_label(step: Step) -> &'static str {
    match step {
        Step::Backup => "Back up the archive and the description bundles",
        Step::Patch => "Prepare the installation",
        Step::Verify => "Verify",
        Step::Activate => "Activate",
        Step::Link => "Link library folders",
    }
}

/// The entry as the inspector's fields now state it, or `None` when they state
/// what it already says.
///
/// **Only the two fields, and the rest of the entry carried over.** The
/// description bundle Bitwig reads is keyed by the entry's display name, and
/// the library path is what says where the document is; re-deriving either from
/// anything would put the new words under a key nobody looks up, or point the
/// registry at a file that is not there.
fn revised(entry: &Registration, words: &widget::Words) -> Option<Registration> {
    if entry.description == words.description && entry.keywords == words.keywords {
        return None;
    }
    Some(Registration {
        description: words.description.clone(),
        keywords: words.keywords.clone(),
        ..entry.clone()
    })
}

/// What to register a published item as, once its bytes have been proved.
///
/// Derived from the **document** and not from the index row, which is the same
/// choice a drop already makes: the description and the search keywords Bitwig
/// will show live in the document's own identity, the index copied them out of
/// there when it was built, and deriving them twice from two places is two
/// places for them to differ. What the index adds is the only thing the document
/// cannot know - which publication this is, and which change published it.
fn published_registration(
    item: &orng_catalog::IndexEntry,
    document: &Document,
) -> orng_tools::Result<Registration> {
    // The index's path is a repository path, so what is after the last slash is
    // the file name the catalog publishes under. The same name goes into the
    // library, so the document is where somebody looking for it would look.
    let file_name = item.path.rsplit_once('/').map(|(_, tail)| tail).unwrap_or(&item.path);
    Ok(Registration {
        provenance: Provenance::Catalog {
            version: item.version,
            reviewed_in: item.merged_in.clone(),
        },
        ..Registration::from_document(document, file_name)?
    })
}

/// Show a document where it lives, in whatever the system uses to look at
/// files.
///
/// Reveal rather than open: opening a `.bwdevice` launches Bitwig Studio, which
/// is the one thing this application spends its time asking people to close.
///
/// A failure is not reported. There is nothing the user could do about a
/// desktop that will not show a folder, and the file manager is not this
/// application's to fix; the alternative is a banner about somebody else's
/// software over the panel that answered the question.
fn reveal(path: &std::path::Path) {
    if let Err(why) = opener::reveal(path) {
        eprintln!("could not reveal {}: {why}", path.display());
    }
}

/// Follow a link out of the application, in whatever browses the web here.
///
/// Not reported for the same reason a failed reveal is not: there is nothing
/// the user could do about a desktop with no browser, and a banner about
/// somebody else's software over the panel that offered the link would be the
/// window blaming itself.
fn browse(url: &str) {
    if let Err(why) = opener::open_browser(url) {
        eprintln!("could not open {url}: {why}");
    }
}

/// Read the file somebody pointed at, and prove it is this entry's document.
///
/// Both halves of what [`Update::add`] asserts, asked here instead, because the
/// file came from a picker rather than from the list: a user who chose the
/// wrong document is to be told which one they chose, not crashed at. Placing
/// it anyway would put one device into Bitwig's browser under another's name
/// and under an identity that already belongs to something else.
///
/// Separate from [`App::relocate`] because the picker is the half no test can
/// drive and this is the half worth driving.
fn this_entrys_document(
    entry: &Registration,
    chosen: &std::path::Path,
) -> Result<Document, String> {
    let name = widget::drawn_path(chosen);
    let document = Document::read(chosen).map_err(|why| format!("{name} could not be read: {why}"))?;
    let found = document.identity().uuid;
    if found != entry.uuid {
        return Err(format!(
            "{name} carries the identity {found}, and {} is {}",
            entry.name, entry.uuid
        ));
    }
    if document.kind() != entry.kind() {
        return Err(format!(
            "{name} is a {:?} and {} is a {:?}",
            document.kind(),
            entry.name,
            entry.kind()
        ));
    }
    Ok(document)
}

/// The first segment of an identity, which is what a row has room for.
fn short_uuid(registration: &Registration) -> String {
    registration.uuid.to_string().split('-').next().unwrap_or_default().to_owned()
}

/// One registered entry.
///
/// Answers whether it was clicked, which is how the inspector is opened - the
/// design makes the whole row the control rather than putting a disclosure
/// arrow on it - and which of the row's own controls was pressed, if one was.
fn row(
    ui: &mut egui::Ui,
    palette: Palette,
    width: widget::Width,
    selected: bool,
    entry: &Registration,
    status: Status,
    document: TheDocument,
) -> (egui::Response, Option<Action>) {
    let secondary = widget::supporting_ink(palette, selected);
    let mut pressed = None;
    let response = widget::row(ui, palette, width, selected, |ui, columns, controls| {
        widget::cell(ui, columns.kind, Align::Min, |ui| {
            widget::kind_label(ui, secondary, entry.kind());
        });
        widget::cell(ui, columns.name, Align::Min, |ui| {
            // Struck through while a removal is queued, which is the design's
            // way of showing a row that is about to stop existing without
            // taking it out of the list the press has not yet been made on.
            let name = font::run(&entry.name, font::emphasis(ui.ctx(), font::ROW_NAME))
                .color(palette.ink);
            let name = if status.struck_through() { name.strikethrough() } else { name };
            ui.add(egui::Label::new(name).truncate())
                .on_hover_text(entry.library_path.as_str());
        });
        if let Some(at) = columns.uuid {
            widget::cell(ui, at, Align::Min, |ui| {
                identity(ui, secondary, entry);
            });
        }
        widget::cell(ui, columns.status, Align::Min, |ui| {
            ui.label(
                font::run(status.word(), font::plain(font::CHIP))
                    .color(widget::status_colour(palette, status)),
            );
        });
        pressed =
            widget::row_actions(ui, palette, columns.actions, controls, status, document);
    });
    (response, pressed)
}

/// An identity, short enough for a column and whole on hover. Clicking copies
/// it, because a UUID is a thing people paste into bug reports and nobody
/// transcribes one by hand.
fn identity(ui: &mut egui::Ui, ink: egui::Color32, entry: &Registration) {
    let full = entry.uuid.to_string();
    let response = ui
        .add(
            egui::Label::new(
                font::run(short_uuid(entry), font::mono(font::MONO)).color(ink),
            )
            .sense(egui::Sense::click()),
        )
        .on_hover_text(format!("{full}\nClick to copy"));
    if response.clicked() {
        ui.ctx().copy_text(full);
    }
}

/// Which row one of the list's own controls was pressed on.
///
/// Not a UUID for both. Two dropped documents claiming one identity is exactly
/// the state the pending list exists to show - it is the collision `Assign new
/// UUID` settles - so an identity there can name two rows and did: the first
/// `Cancel` written against one took both away.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Acting {
    /// A row in the pending list, by its position in it.
    Pending(usize),
    /// A registered entry, which the entry list keys by identity.
    Registered(Uuid),
}

/// One dropped document, and what can be done with it.
///
/// Answers which of its own controls was pressed, if one was. It is never the
/// selected row: the inspector opens on registered entries, and a staged
/// document is not one yet.
fn staged_row(
    ui: &mut egui::Ui,
    palette: Palette,
    width: widget::Width,
    staged: &Staged,
    document: TheDocument,
) -> Option<Action> {
    let status = staged.status();
    let mut pressed = None;
    widget::row(ui, palette, width, false, |ui, columns, controls| {
        widget::cell(ui, columns.kind, Align::Min, |ui| {
            // A rejected row has no kind, because nothing readable said what it
            // was. Drawing one would be inventing it.
            match staged.registration() {
                Some(registration) => widget::kind_label(ui, palette.ink_3, registration.kind()),
                None => {
                    ui.label(font::run("-", font::plain(font::CHIP)).color(palette.ink_3));
                }
            }
        });
        widget::cell(ui, columns.name, Align::Min, |ui| {
            ui.label(
                font::run(&staged.label, font::emphasis(ui.ctx(), font::ROW_NAME))
                    .color(if staged.is_ready() { palette.ink } else { palette.ink_2 }),
            );
            // The reason sits beside the name, in the colour of the status it
            // explains, so an explanation is never louder than the word it
            // belongs to. It goes with the identity when the inspector is open:
            // the design drops both rather than truncating a sentence into
            // whatever the narrow name column has left.
            if let Some(why) = staged.reason().filter(|_| width == widget::Width::Full) {
                ui.add_space(BESIDE_THE_NAME);
                ui.add(
                    egui::Label::new(
                        font::run(why, font::plain(font::NOTE))
                            .color(widget::status_colour(palette, status)),
                    )
                    .truncate(),
                );
            }
        });
        if let Some(at) = columns.uuid {
            widget::cell(ui, at, Align::Min, |ui| match staged.registration() {
                Some(registration) => identity(ui, palette.ink_3, registration),
                None => {
                    ui.label(font::run("-", font::mono(font::MONO)).color(palette.ink_3));
                }
            });
        }
        widget::cell(ui, columns.status, Align::Min, |ui| {
            ui.label(
                font::run(status.word(), font::plain(font::CHIP))
                    .color(widget::status_colour(palette, status)),
            );
        });
        pressed =
            widget::row_actions(ui, palette, columns.actions, controls, status, document);
    });
    pressed
}

/// Between a name and the reason beside it, which is closer than two separate
/// things but further than one phrase.
const BESIDE_THE_NAME: f32 = 9.0;

/// Between a catalog item's name and the description under it.
const UNDER_THE_NAME: f32 = 2.0;

/// What a press in the catalog list was.
///
/// Two fields rather than one enum because they are not alternatives: the row's
/// own control stops the press reaching the row, so at most one of these
/// happens, but which one is not a choice the list makes.
#[derive(Default)]
struct Pressed {
    /// The row that was opened, if one was. `Some(None)` closes the panel,
    /// which is what clicking the open row again means.
    opened: Option<Option<Uuid>>,
    /// The item whose own control was pressed, and which control it was.
    acted: Option<(Uuid, Offer)>,
    /// Whether the empty state the filter left behind was asked to undo itself.
    /// Not a row press at all, and it is here because it is the one other thing
    /// this region answers.
    cleared: bool,
    /// Whether the empty state left behind by a catalog that never arrived was
    /// asked to go and look again. The other press that is not a row's.
    retried: bool,
}

/// The Catalog view: what ORNG Catalog publishes, once it has been proved.
fn published(
    ui: &mut egui::Ui,
    palette: Palette,
    catalog: &Catalog,
    width: widget::Width,
    open: Option<Uuid>,
    states: &std::collections::BTreeMap<Uuid, Published>,
    filter: &Filter,
) -> Pressed {
    // Nothing held is the only state with an empty region, and being offline is
    // not one of them: a kept index browses exactly as a fetched one does,
    // which is the whole of what the design means by a degraded state rather
    // than an error.
    match catalog.index() {
        None if catalog.failure().is_none() => {
            let empty = widget::Empty {
                icon: icon::CATALOG,
                inviting: false,
                marks: true,
                title: "Fetching the catalog",
                body: "Checking its signature before anything in it is believed.",
                extensions: false,
                aside: None,
                action: None,
                action_is_primary: false,
                alt: None,
                foot: None,
                minor: false,
            };
            widget::empty_state(ui, palette, &empty);
        }
        None => {
            let why = catalog.failure().expect("the arm above took the other case");
            let empty = widget::Empty {
                icon: icon::UNREADABLE,
                inviting: false,
                marks: true,
                title: "The catalog could not be read",
                // The last clause is `EmptyState.dc.html:89`'s, and it is worth
                // saying now that it is true: what this state costs is one
                // fetch and not a permanent requirement, which is the point of
                // the scenario the design draws it for.
                body: "Nothing is installed from an index that does not verify. The catalog \
                       is one small file over HTTPS, signed by the key this application was \
                       built with, and once fetched it is cached - so browsing works offline \
                       afterwards.",
                extensions: false,
                aside: Some(why),
                action: Some("Try again"),
                action_is_primary: true,
                alt: None,
                foot: Some("Everything already registered keeps working offline"),
                minor: false,
            };
            if widget::empty_state(ui, palette, &empty) == widget::Pressed::Action {
                return Pressed { retried: true, ..Pressed::default() };
            }
        }
        Some(index) if index.items.is_empty() => {
            let empty = widget::Empty {
                icon: icon::CATALOG,
                inviting: false,
                marks: true,
                title: "The catalog is empty",
                body: "Nothing is published yet.",
                extensions: false,
                aside: None,
                action: None,
                action_is_primary: false,
                alt: None,
                foot: None,
                minor: false,
            };
            widget::empty_state(ui, palette, &empty);
        }
        // No section heading here, and that is the design's decision: the Local
        // view divides into pending, registered and factory, and the catalog is
        // one list of one kind of thing.
        Some(index) => {
            let shown: Vec<&orng_catalog::IndexEntry> = index
                .items
                .iter()
                .filter(|item| {
                    let status = states
                        .get(&item.uuid)
                        .expect("every published item was answered for before the list was drawn");
                    filter.accepts_published(item, status)
                })
                .collect();
            if shown.is_empty() {
                // The design's own words, and a different sentence from the
                // Local list's: this one names three filters because the
                // catalog has three - `EmptyState.dc.html:93-97`.
                //
                // **`Browse all` is drawn as the alt there and is not drawn
                // here.** The shell's two handlers do the same thing -
                // `ORNG Registry.dc.html:770` and `:773` both put the kinds and
                // the install filter back to their defaults - so the second
                // control offers the user nothing the first does not.
                // `docs/design-review.md` round 3 item 3.
                let empty = widget::Empty {
                    icon: icon::NO_MATCH,
                    inviting: false,
                    marks: false,
                    title: "Nothing in the catalog matches",
                    body: "No item matches the current search, kind and install filters.",
                    extensions: false,
                    aside: None,
                    action: Some("Clear filters"),
                    action_is_primary: false,
                    alt: None,
                    foot: None,
                    minor: true,
                };
                let cleared = widget::empty_state(ui, palette, &empty) == widget::Pressed::Action;
                return Pressed { cleared, ..Pressed::default() };
            }

            let mut pressed = Pressed::default();
            let mut acted = None;
            widget::list(ui, |ui| {
                for entry in shown {
                    let selected = open == Some(entry.uuid);
                    let secondary = widget::supporting_ink(palette, selected);
                    let status = states
                        .get(&entry.uuid)
                        .expect("every published item was answered for before the list was drawn");
                    let row = widget::catalog_row(ui, palette, width, selected, |ui, columns| {
                        widget::cell(ui, columns.kind, Align::Min, |ui| {
                            widget::kind_label(ui, secondary, entry.kind.into());
                        });
                        // The name over the description, not beside it. A
                        // catalog row leads with what the item is; the
                        // description is how somebody choosing decides, and it
                        // needs the width of the column rather than what is
                        // left of one line.
                        widget::stacked_cell(ui, columns.name, |ui| {
                            ui.label(
                                font::run(&entry.name, font::emphasis(ui.ctx(), font::ROW_NAME))
                                    .color(widget::published_name_colour(palette, status)),
                            );
                            ui.add_space(UNDER_THE_NAME);
                            ui.add(
                                egui::Label::new(
                                    font::run(&entry.description, font::plain(font::NOTE))
                                        .color(secondary),
                                )
                                .truncate(),
                            );
                        });
                        // The author is the trust signal, because an item is
                        // DSP that Bitwig will run, so it gets a column of its
                        // own rather than a place at the end of the line.
                        if let Some(at) = columns.author {
                            widget::cell(ui, at, Align::Min, |ui| {
                                ui.add(
                                    egui::Label::new(
                                        font::run(entry.author.to_string(), font::plain(font::CHIP))
                                            .color(palette.ink_2),
                                    )
                                    .truncate(),
                                );
                            });
                        }
                        if let Some(at) = columns.version {
                            widget::cell(ui, at, Align::Min, |ui| {
                                ui.label(
                                    font::run(entry.version.to_string(), font::mono(font::MONO))
                                        .color(secondary),
                                );
                            });
                        }
                        widget::cell(ui, columns.status, Align::Min, |ui| {
                            ui.add(
                                egui::Label::new(
                                    font::run(status.word(), font::plain(font::CHIP))
                                        .color(widget::published_colour(palette, status)),
                                )
                                .truncate(),
                            );
                        });
                        if let Some(offer) = widget::catalog_action(ui, palette, columns.actions, status)
                        {
                            acted = Some((entry.uuid, offer));
                        }
                    });
                    if row.clicked() {
                        // The same row again closes it, which is what makes the
                        // panel answerable from the list it is about.
                        pressed.opened = Some(if selected { None } else { Some(entry.uuid) });
                    }
                }
            });
            pressed.acted = acted;
            return pressed;
        }
    }
    Pressed::default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use orng_tools::LibraryPath;

    fn entry() -> Registration {
        Registration {
            uuid: "8b330d22-73fa-4ba5-a42f-2f2300cbd8bf".parse().expect("a sample identity"),
            name: "VOLSHAPER".to_owned(),
            library_path: LibraryPath::new("devices/My Devices/VOLSHAPER.bwdevice")
                .expect("a library path"),
            description: "Beat-synced volume LFO".to_owned(),
            keywords: vec!["volshaper".to_owned()],
            digest: None,
            provenance: Provenance::Catalog {
                version: "1.0.0".parse().expect("a version"),
                reviewed_in: None,
            },
        }
    }

    /// Leaving a field untouched is still leaving it, and every field in the
    /// panel reports that it was left. Without this, closing the inspector on
    /// an entry nobody edited would rewrite three description bundles and the
    /// entry list, every time.
    #[test]
    fn words_that_say_what_the_entry_already_says_are_not_a_change() {
        let entry = entry();
        let words = widget::Words::of(&entry.description, &entry.keywords);
        assert!(revised(&entry, &words).is_none());
    }

    /// An edit changes the two fields the panel offers and nothing else.
    ///
    /// The name especially: Bitwig's description bundle is keyed by it, so an
    /// entry whose name moved under an edit would have its new words written
    /// under a key nothing reads, and the old ones would be what the browser
    /// went on showing.
    #[test]
    fn an_edit_changes_the_words_and_leaves_the_rest_of_the_entry_alone() {
        let entry = entry();
        let mut words = widget::Words::of(&entry.description, &entry.keywords);
        words.description = "Beat-synced volume shaper".to_owned();
        words.keywords.push("lfo".to_owned());

        let revised = revised(&entry, &words).expect("that is a change");
        assert_eq!(revised.description, "Beat-synced volume shaper");
        assert_eq!(revised.keywords, ["volshaper", "lfo"]);
        assert_eq!(revised.name, entry.name);
        assert_eq!(revised.uuid, entry.uuid);
        assert_eq!(revised.library_path, entry.library_path);
        assert_eq!(revised.provenance, entry.provenance);
    }

    /// The two sentences the bundle writes about the catalog's age, and the
    /// rule underneath them that it does not.
    ///
    /// Both literals are the design's: `InstallBar.dc.html:61` defaults to
    /// `Catalog updated 20 minutes ago` and `ORNG Registry.dc.html:467` writes
    /// `Catalog from 12 days ago` on the stale scenario. So the two forms are
    /// pinned against the bundle, and only the unit arithmetic between them is
    /// ours.
    #[test]
    fn the_catalogs_age_is_stated_in_the_designs_two_sentences() {
        let minutes = std::time::Duration::from_secs(20 * 60);
        assert_eq!(stated(Freshness::Current(minutes)), "Catalog updated 20 minutes ago");
        let days = std::time::Duration::from_secs(12 * 24 * 60 * 60);
        assert_eq!(stated(Freshness::Stale(days)), "Catalog from 12 days ago");
        assert_eq!(stated(Freshness::Never), "Never fetched");

        // The largest unit that gives a whole number, and never a zero: a fetch
        // that landed nine seconds ago reads as recent rather than as
        // `0 minutes ago`, which reads as broken.
        let ago = |seconds| elapsed(std::time::Duration::from_secs(seconds));
        assert_eq!(ago(0), "just now");
        assert_eq!(ago(59), "just now");
        assert_eq!(ago(60), "1 minute ago");
        assert_eq!(ago(119), "1 minute ago");
        assert_eq!(ago(59 * 60), "59 minutes ago");
        assert_eq!(ago(60 * 60), "1 hour ago");
        assert_eq!(ago(23 * 60 * 60), "23 hours ago");
        assert_eq!(ago(24 * 60 * 60), "1 day ago");
        assert_eq!(ago(90 * 24 * 60 * 60), "90 days ago");
    }

    /// The clause the result banner adds when a press took something away.
    ///
    /// `ORNG Registry.dc.html:532` writes it: "8 entries registered, 1
    /// removed". Nothing is said when nothing was removed, which is what every
    /// other count in this window does with its zero.
    #[test]
    fn a_press_that_removed_nothing_says_nothing_about_removals() {
        assert_eq!(also_removed(0), "");
        assert_eq!(also_removed(1), ", 1 removed");
        assert_eq!(also_removed(4), ", 4 removed");
    }

    /// A file somebody pointed at has to be the document the entry names.
    ///
    /// The picker is the half no test can drive; this is the half that decides
    /// whether a stranger's device is written into Bitwig's browser under this
    /// entry's name and this entry's identity.
    #[test]
    fn locating_refuses_a_file_that_is_not_this_entry() {
        let temp = tempfile::tempdir().expect("somewhere to write");
        let entry = entry();

        let write = |name: &str, kind: Kind, uuid: Uuid| {
            let path = temp.path().join(name);
            let document = orng_tools::testing::document(kind, uuid, "VOLSHAPER");
            std::fs::write(&path, document.bytes()).expect("could not write the sample");
            path
        };

        // The entry's own document, which is the whole point of the control.
        let its_own = write("VOLSHAPER.bwdevice", entry.kind(), entry.uuid);
        let found = this_entrys_document(&entry, &its_own).expect("that is this entry");
        assert_eq!(found.identity().uuid, entry.uuid);

        // Another device entirely. Placing it would register somebody else's
        // content under this entry's name and identity.
        let stranger = write("STRANGER.bwdevice", entry.kind(), Uuid::new_v4());
        let why = this_entrys_document(&entry, &stranger).expect_err("that is not this entry");
        assert!(why.contains("carries the identity"), "{why}");
        assert!(why.contains("VOLSHAPER"), "{why}");

        // The right identity on the wrong kind of document, which is the other
        // half of what `Update::add` asserts and would otherwise be a panic.
        let wrong_kind = write("VOLSHAPER.bwmodulator", Kind::Modulator, entry.uuid);
        let why = this_entrys_document(&entry, &wrong_kind).expect_err("that is a modulator");
        assert!(why.contains("Modulator"), "{why}");

        // And something that is not a document at all.
        let nonsense = temp.path().join("NONSENSE.bwdevice");
        std::fs::write(&nonsense, vec![b'x'; 4096]).expect("could not write the sample");
        let why = this_entrys_document(&entry, &nonsense).expect_err("that does not read");
        assert!(why.contains("could not be read"), "{why}");
    }
}
