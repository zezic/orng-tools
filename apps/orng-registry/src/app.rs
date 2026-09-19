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
use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, vec2};
use orng_tools::{
    Content, Document, Kind, Placement, Provenance, Registration, RunState, Step, Strategy,
    TheDocument, Update, Uuid, placement,
};

use crate::about::About;
use crate::catalog::Fetching;
use crate::diagnostics::Diagnostics;
use crate::restore::Backups;
use crate::session::{Badge, Found, Session};
use crate::settings::{Appearance, Preferences, Settings};
use crate::staging::{self, Reading, Staged};
use crate::status::{Action, Status};
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

/// What the list toolbar is showing of the list.
///
/// A filter is not a property of the entries, so it does not live in the
/// session: re-reading the machine must not silently clear what the user typed.
#[derive(Debug)]
struct Filter {
    /// Matched against the display name and against the identity, which is the
    /// pair of questions this view answers: what is this thing I have, and
    /// which row is the one this UUID names.
    query: String,
    /// Empty means none, not all: the toolbar shows every kind switched on and
    /// switching all three off is a thing a user can do and undo.
    kinds: BTreeSet<Kind>,
}

impl Default for Filter {
    fn default() -> Self {
        Filter { query: String::new(), kinds: Kind::ALL.into_iter().collect() }
    }
}

impl Filter {
    fn accepts(&self, entry: &Registration) -> bool {
        if !self.kinds.contains(&entry.kind) {
            return false;
        }
        let query = self.query.trim().to_lowercase();
        query.is_empty()
            || entry.name.to_lowercase().contains(&query)
            || entry.uuid.to_string().contains(&query)
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
    /// A drop being read, off the interface thread.
    reading: Option<Reading>,
    /// Set while work is in flight, and only while it is in flight: the moment
    /// it reports, what it did becomes an [`Outcome`] and the work is over.
    applying: Option<Applying>,
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
    /// The published catalog, once somebody has asked for it. Not fetched on
    /// opening: this application is useful with no network at all, and a window
    /// that reaches for one before being asked is a window that hangs on a
    /// train.
    catalog: Option<Fetching>,
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
        let preferences = match home {
            Some(home) => Preferences::read(home),
            None => Preferences::unwritten(Settings::default()),
        };
        let session = Session::read(preferences.chosen());
        App::with(&cc.egui_ctx, session).having(preferences)
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
            reading: None,
            applying: None,
            outcome: None,
            inspecting: None,
            detailing: None,
            catalog: None,
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

    /// Put a catalog on screen without fetching one. Tests only.
    #[cfg(test)]
    pub fn set_catalog(&mut self, catalog: Fetching) {
        self.catalog = Some(catalog);
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
        self.pump();
        self.take_drop(ui.ctx());

        match &self.screen {
            Screen::Browsing => self.browse(ui),
            Screen::Settings(_) => self.settings(ui),
            Screen::Restore(_) => self.restore(ui),
            Screen::About(_) => self.about(ui),
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
        egui::Panel::top("install")
            .exact_size(metric::INSTALL_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.install_bar(ui));

        egui::Panel::bottom("action")
            .exact_size(metric::ACTION_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.action_bar(ui));

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

        if self.shows_a_list() {
            egui::Panel::top("toolbar")
                .exact_size(metric::TOOLBAR)
                .frame(widget::toolbar(self.palette))
                .show(ui, |ui| self.list_toolbar(ui));
        }

        egui::CentralPanel::default()
            .frame(widget::page(self.palette))
            .show(ui, |ui| self.page(ui));

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
                        if widget::cancel_button(ui, palette, "Cancel").clicked() {
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
        let outcome = backups.restore(&found.to.install);
        self.screen = Screen::Browsing;
        match outcome {
            // Nothing was pointed at, so nothing happened. Unreachable from the
            // window - the press is disabled with no copy chosen - and said
            // rather than panicked on, because the alternative to saying it is
            // a window that swallowed a press.
            None => return,
            Some(Ok(())) => self.outcome = Some(Outcome::Restored),
            Some(Err(why)) => {
                self.outcome = Some(Outcome::NotRestored { why: why.to_string() });
            }
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
        let open = self.inspecting.as_mut()?;
        let uuid = open.uuid;
        let Some(entry) = found.entries().get(uuid) else {
            // Applied, removed, or gone from a list that was read again. There
            // is nothing left to inspect, so the panel closes rather than
            // standing empty - and the words it was holding go with it.
            self.inspecting = None;
            return None;
        };

        let (source, source_icon) = match &entry.provenance {
            Provenance::Local => ("Local file".to_owned(), widget::icon::LOCAL_FILE),
            Provenance::Catalog { version } => {
                (format!("ORNG Catalog {} {version}", widget::SEPARATOR), widget::icon::CATALOG)
            }
        };
        let identity = uuid.to_string();
        // Split apart so the fields can be borrowed separately: the panel
        // states the placement and types into the words in one call.
        let Inspection { words, placement, .. } = open;
        let item = widget::Inspected {
            kind: entry.kind,
            name: &entry.name,
            uuid: &identity,
            path: entry.library_path.as_str(),
            source: &source,
            source_icon,
            placement,
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
            // Read back off the panel rather than held across the draw: the
            // placement belongs to the panel now, and reaching for it again
            // here is what lets the arms above put the panel away.
            widget::Inspecting::Reveal => {
                if let Some(open) = &self.inspecting {
                    reveal(open.placement.path());
                }
            }
            // Queued, exactly as the row's own trash queues it, and the panel
            // stays open on it: the entry is still registered and still drawn,
            // struck through, until the apply that takes it away.
            widget::Inspecting::Remove => {
                self.removing.insert(uuid);
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
        let Some(Ok(index)) = self.catalog.as_ref().map(|c| &c.outcome).and_then(Option::as_ref)
        else {
            return None;
        };
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
        let replacement = index
            .items
            .iter()
            .find(|other| other.supersedes.contains(&uuid))
            .map(|other| (other.name.as_str(), other.uuid));
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
            replaced_by: replacement.map(|(name, _)| name),
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

        let mut update = Update::to(found.entries().clone());
        update.revise(revised);
        self.applying = Some(Applying::start(
            Errand::Edit,
            found.to.clone(),
            update,
            ctx.clone(),
        ));
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
                let banner =
                    widget::Banner { tone, title: &title, body: &body, action, dismissible };
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
    fn pump(&mut self) {
        if let Some(catalog) = &mut self.catalog {
            catalog.poll();
        }
        if let Some(read) = self.reading.as_mut().and_then(Reading::take) {
            self.staged.extend(read);
            self.reading = None;
        }
        let Some(applying) = &mut self.applying else { return };
        applying.poll();
        let Some(result) = &applying.outcome else { return };
        // Taken once, here, and the work is then over: an `Applying` that has
        // reported is not work in flight, and leaving it in that field is what
        // made every screen after a press have to ask whether it had finished.
        let result = result.clone();
        let errand = applying.errand;
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
            }
            Ok(entries) => {
                // What was written is no longer pending. Held until here rather
                // than cleared when the press started, so that a failure leaves
                // the same rows to press again instead of asking for the drop
                // back. The removal queue goes the same way and for the same
                // reason: those entries are gone from the list now, so a queue
                // still naming them would strike through rows that do not
                // exist.
                self.staged.clear();
                self.removing.clear();
                let in_effect = entries.entries().len();
                if errand.prepares() {
                    // A preparation changes what is true of the installation:
                    // the archive, the guard, the links. Nothing short of
                    // reading it again answers that.
                    self.session = Session::read(self.preferences.chosen());
                    self.outcome = Some(Outcome::Prepared { entries: in_effect, removed });
                } else {
                    if let Session::Found(found) = &mut self.session {
                        // An entry update changes one text file, and the worker
                        // answered with what it wrote. Reading the machine
                        // again would cost seconds to arrive at the value
                        // already in hand.
                        found.relist(entries);
                    }
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
    fn read(&mut self, paths: Vec<PathBuf>, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        if paths.is_empty() {
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

    /// Whether the middle region is a list, which is what the toolbar belongs to.
    fn shows_a_list(&self) -> bool {
        let Session::Found(found) = &self.session else { return false };
        // Nothing to filter is nothing to filter with. The design keeps the
        // toolbar when a filter has narrowed the list to nothing, because that
        // is how the filter gets cleared, and drops it on the onboarding
        // screen, where there is no list behind it.
        //
        // Work in flight does not take it away either: the design draws the
        // preparation over the list rather than instead of it.
        let anything = !found.entries().is_empty() || !self.staged.is_empty();
        self.view == View::Local && anything
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
    /// Only the preparing mode can be blocked. Registering entries writes no
    /// part of the archive and is never held up by a running Bitwig, which is
    /// the whole of what the cheap mode buys.
    fn blocking(&self) -> Option<Blocked> {
        let Session::Found(found) = &self.session else { return None };
        if self.pending(found) != Some(Work::PrepareThenEntries) {
            return None;
        }
        if self.applying.as_ref().is_some_and(Applying::is_running) {
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
            Outcome::Failed { why, .. } | Outcome::NotRestored { why } => Some(why),
            Outcome::Prepared { .. }
            | Outcome::Registered { .. }
            | Outcome::Located
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
                    found.entries().entries().iter().filter(|e| e.kind == kind).count();
                let staged = self
                    .staged
                    .iter()
                    .filter_map(Staged::registration)
                    .filter(|r| r.kind == kind)
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
        // not built: the field, the chips, the flexible gap, and `Add files...`.
        // Three gaps between the four.
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
            if self.filter.kinds.contains(&kind) {
                self.filter.kinds.remove(&kind);
            } else {
                self.filter.kinds.insert(kind);
            }
        }
        if adding {
            self.add_files(ui);
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
    fn page(&mut self, ui: &mut egui::Ui) {
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
                let catalog =
                    self.catalog.get_or_insert_with(|| Fetching::start(ui.ctx().clone()));
                // Taken after the list has been drawn, for the reason the
                // Local list takes its own: opening the panel changes how wide
                // every row is, and changing that half way down a list draws
                // the rest of it to a different grid.
                if let Some(opened) = published(ui, palette, catalog, width, open) {
                    self.detailing = opened;
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

        // A dropped document that is already registered updates that entry
        // rather than adding a second, so it is one piece of pending work and
        // gets one row - the staged one, which is the one that can be acted on.
        let staged: Vec<&Registration> =
            self.staged.iter().filter_map(Staged::registration).collect();
        let registered: Vec<&Registration> = found
            .entries()
            .entries()
            .iter()
            .filter(|entry| !staged.iter().any(|pending| pending.uuid == entry.uuid))
            .filter(|entry| self.filter.accepts(entry))
            .collect();
        // Numbered before the filter, so a row carries where it is in the
        // pending list rather than where it is on screen. A control pressed on
        // the third row of a filtered list acts on the third row of the list
        // the filter was applied to, which is not the same row.
        let shown: Vec<(usize, &Staged)> = self
            .staged
            .iter()
            .enumerate()
            .filter(|(_, s)| s.registration().is_none_or(|r| self.filter.accepts(r)))
            .collect();

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

        if shown.is_empty() && registered.is_empty() {
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
            if !shown.is_empty() {
                widget::section(ui, palette, "Pending", palette.accent_text, shown.len());
                for (at, pending) in &shown {
                    if let Some(action) = staged_row(ui, palette, width, pending, document) {
                        pressed = Some((Acting::Pending(*at), action));
                    }
                }
            }
            widget::section(ui, palette, "Registered", palette.ink_2, registered.len());
            for entry in &registered {
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
        Status::Registered
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
            // Resolved on the press rather than held, which is the rule every
            // screen here follows: asking the disk where a document is, once
            // per row per frame, is the fault this application has already
            // taken out of the inspector.
            (Acting::Registered(uuid), Action::Reveal) => {
                if let Some(entry) = found.entries().get(uuid) {
                    reveal(placement::inspect(&found.to.install, entry).path());
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
            .add_filter("Bitwig documents", &[entry.kind.extension()])
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

        let mut update = Update::to(found.entries().clone());
        update.add(entry.clone(), document);
        self.applying =
            Some(Applying::start(Errand::Locate, found.to.clone(), update, ctx.clone()));
    }

    /// The drop target, while something is over the window.
    fn hovering(&self, ui: &mut egui::Ui) {
        let hovered: Vec<PathBuf> = ui.ctx().input(|input| {
            input.raw.hovered_files.iter().filter_map(|file| file.path.clone()).collect()
        });
        if hovered.is_empty() {
            return;
        }
        // Accept and reject are stated before the drop, from the name alone,
        // because that is all there is to go on while the file is still the
        // operating system's. Every file is named, refused ones included: a
        // listing of what will be taken cannot be checked against what the
        // pointer is carrying.
        let files: Vec<widget::Hovering> = hovered.iter().map(over).collect();
        let staging = staging::documents_in(&hovered).len();
        let heading = match staging {
            0 => "Nothing here can be registered".to_owned(),
            1 => "Drop to stage 1 document".to_owned(),
            many => format!("Drop to stage {many} documents"),
        };
        widget::drop_target(ui, self.palette, &heading, &files);
    }

    /// Region three: what one press would do, and the press.
    fn action_bar(&mut self, ui: &mut egui::Ui) {
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
                    let (summary, tone, note) = self.summary();
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
    fn summary(&self) -> (String, Tone, String) {
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
            self.start(work, ui.ctx());
        }
    }

    /// Hand the pending work to a thread that is not this one.
    fn start(&mut self, work: Work, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        let mut update = Update::to(found.entries().clone());
        // Only the rows that are ready. A conflict is pending work the user has
        // to resolve, and writing it would be resolving it for them.
        //
        // Cloned rather than taken. The rows stay in the list until the write
        // succeeds, so a failure leaves the same pending work rather than
        // asking the user to find the files again; a document is tens of
        // kilobytes and the copy is not worth avoiding at that price.
        for staged in &self.staged {
            if let crate::staging::State::Ready { registration, document } = &staged.state {
                update.add(registration.clone(), (**document).clone());
            }
        }
        // And the rows the user asked to be rid of, which have been struck
        // through in the list since they were queued. The document goes with
        // them or does not, as the preference in force says, and the row's own
        // control named that preference when it was pressed.
        let document = self.deleting();
        for uuid in self.removals(found).collect::<Vec<_>>() {
            update.remove(uuid, document);
        }
        // What the last press came to is not what this one will come to.
        self.outcome = None;
        self.applying =
            Some(Applying::start(work.into(), found.to.clone(), update, ctx.clone()));
    }
}

/// One path under the pointer, as the drop overlay states it.
///
/// A folder is named as a folder and counted, rather than unfolded into the
/// documents inside it: what the user is dragging is the folder, and a listing
/// that says something else cannot be checked against the pointer.
fn over(path: &PathBuf) -> widget::Hovering {
    let name = path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned();
    if Kind::from_path(path).is_some() {
        return widget::Hovering { name, note: String::new(), accepted: true };
    }
    match staging::documents_in(std::slice::from_ref(path)).len() {
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
    if document.kind() != entry.kind {
        return Err(format!(
            "{name} is a {:?} and {} is a {:?}",
            document.kind(),
            entry.name,
            entry.kind
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
            widget::kind_label(ui, secondary, entry.kind);
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
                Some(registration) => widget::kind_label(ui, palette.ink_3, registration.kind),
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

/// The Catalog view: what ORNG Catalog publishes, once it has been proved.
///
/// Answers which row was opened, if one was - `Some(None)` closes the panel,
/// which is what clicking the open row again means.
fn published(
    ui: &mut egui::Ui,
    palette: Palette,
    catalog: &Fetching,
    width: widget::Width,
    open: Option<Uuid>,
) -> Option<Option<Uuid>> {
    match catalog.outcome.as_ref() {
        None => {
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
        Some(Err(why)) => {
            let empty = widget::Empty {
                icon: icon::UNREADABLE,
                inviting: false,
                marks: true,
                title: "The catalog could not be read",
                body: "Nothing is installed from an index that does not verify. The catalog \
                       is one small file over HTTPS, signed by the key this application was \
                       built with.",
                extensions: false,
                aside: Some(why),
                action: None,
                action_is_primary: false,
                alt: None,
                foot: Some("Everything already registered keeps working"),
                minor: false,
            };
            widget::empty_state(ui, palette, &empty);
        }
        Some(Ok(index)) if index.items.is_empty() => {
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
        Some(Ok(index)) => {
            let mut opened = None;
            widget::list(ui, |ui| {
                for entry in &index.items {
                    let selected = open == Some(entry.uuid);
                    let secondary = widget::supporting_ink(palette, selected);
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
                                    .color(palette.ink),
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
                    });
                    if row.clicked() {
                        // The same row again closes it, which is what makes the
                        // panel answerable from the list it is about.
                        opened = Some(if selected { None } else { Some(entry.uuid) });
                    }
                }
            });
            return opened;
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use orng_tools::LibraryPath;

    fn entry() -> Registration {
        Registration {
            uuid: "8b330d22-73fa-4ba5-a42f-2f2300cbd8bf".parse().expect("a sample identity"),
            kind: Kind::Device,
            name: "VOLSHAPER".to_owned(),
            library_path: LibraryPath::new("devices/My Devices/VOLSHAPER.bwdevice")
                .expect("a library path"),
            description: "Beat-synced volume LFO".to_owned(),
            keywords: vec!["volshaper".to_owned()],
            digest: None,
            provenance: Provenance::Catalog {
                version: "1.0.0".parse().expect("a version"),
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
        assert_eq!(revised.kind, entry.kind);
        assert_eq!(revised.library_path, entry.library_path);
        assert_eq!(revised.provenance, entry.provenance);
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
        let its_own = write("VOLSHAPER.bwdevice", entry.kind, entry.uuid);
        let found = this_entrys_document(&entry, &its_own).expect("that is this entry");
        assert_eq!(found.identity().uuid, entry.uuid);

        // Another device entirely. Placing it would register somebody else's
        // content under this entry's name and identity.
        let stranger = write("STRANGER.bwdevice", entry.kind, Uuid::new_v4());
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
