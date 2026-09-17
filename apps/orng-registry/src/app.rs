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

use eframe::egui::{self, Align, Layout, RichText, vec2};
use orng_tools::{Kind, Registration, RunState, Step, Update};

use crate::catalog::Fetching;
use crate::session::{Badge, Found, Session};
use crate::staging::{self, Reading, Staged};
use crate::theme::{self, Palette, font, metric};
use crate::widget::{self, Tone, icon};
use crate::work::{Applying, Stage, Work};

/// Which top-level view is showing. Two, as the design has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// What is registered on this machine.
    Local,
    /// What ORNG Catalog publishes.
    Catalog,
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
    view: View,
    palette: Palette,
    dark: bool,
    filter: Filter,
    /// Documents dropped and not yet written. The pending work.
    staged: Vec<Staged>,
    /// A drop being read, off the interface thread.
    reading: Option<Reading>,
    /// Set while work is in flight, and kept afterwards so the result stays on
    /// screen until the user does something else.
    applying: Option<Applying>,
    /// Whether the result of the last press has been taken into the session.
    /// Once, not every frame: re-reading the machine costs seconds.
    absorbed: bool,
    /// The published catalog, once somebody has asked for it. Not fetched on
    /// opening: this application is useful with no network at all, and a window
    /// that reaches for one before being asked is a window that hangs on a
    /// train.
    catalog: Option<Fetching>,
}

impl App {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        App::with(&cc.egui_ctx, Session::read())
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
            view: View::Local,
            palette,
            dark: true,
            filter: Filter::default(),
            staged: Vec::new(),
            reading: None,
            applying: None,
            absorbed: false,
            catalog: None,
        }
    }

    pub fn show_view(&mut self, view: View) {
        self.view = view;
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

    /// Switch palettes. The toolbar and the render tests share this, so neither
    /// can change themes in a way the other does not.
    pub fn set_theme(&mut self, dark: bool, ctx: &egui::Context) {
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
    pub fn draw(&mut self, ui: &mut egui::Ui) {
        self.pump();
        self.take_drop(ui.ctx());

        egui::Panel::top("install")
            .exact_size(metric::INSTALL_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.install_bar(ui));

        if let Some((tone, title, body)) = self.blocking() {
            // The panel is filled before the banner washes over it. A panel
            // with no frame of its own shows whatever was behind the window,
            // and a translucent wash over that is not a colour anybody chose.
            egui::Panel::top("banner")
                .frame(egui::Frame::new().fill(self.palette.bg))
                .show(ui, |ui| widget::banner(ui, self.palette, tone, title, &body));
        }

        egui::Panel::bottom("action")
            .exact_size(metric::ACTION_BAR)
            .frame(widget::bar(self.palette))
            .show(ui, |ui| self.action_bar(ui));

        if self.shows_a_list() {
            egui::Panel::top("toolbar")
                .exact_size(metric::TOOLBAR)
                .frame(widget::toolbar(self.palette))
                .show(ui, |ui| self.list_toolbar(ui));
        }

        egui::CentralPanel::default()
            .frame(widget::page(self.palette))
            .show(ui, |ui| self.page(ui));
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
        if self.absorbed {
            return;
        }
        let Some(Ok(entries)) = &applying.outcome else { return };
        let entries = entries.clone();
        let prepared = applying.prepared();
        self.absorbed = true;

        // What was written is no longer pending. Held until here rather than
        // cleared when the press started, so that a failure leaves the same
        // rows to press again instead of asking for the drop back.
        self.staged.clear();
        if prepared {
            // A preparation changes what is true of the installation: the
            // archive, the guard, the links. Nothing short of reading it again
            // answers that.
            self.session = Session::read();
        } else if let Session::Found(found) = &mut self.session {
            // An entry update changes one text file, and the worker answered
            // with what it wrote. Reading the machine again would cost seconds
            // to arrive at the value already in hand.
            found.entries = entries;
        }
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
            found.entries.clone(),
            found.to.clone(),
            already,
            ctx.clone(),
        ));
    }

    /// Whether the middle region is a list, which is what the toolbar belongs to.
    fn shows_a_list(&self) -> bool {
        let running = self.applying.as_ref().is_some_and(|a| a.steps.is_some());
        let Session::Found(found) = &self.session else { return false };
        // Nothing to filter is nothing to filter with. The design keeps the
        // toolbar when a filter has narrowed the list to nothing, because that
        // is how the filter gets cleared, and drops it on the onboarding
        // screen, where there is no list behind it.
        let anything = !found.entries.is_empty() || !self.staged.is_empty();
        self.view == View::Local && !running && anything
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
        (self.ready().count() > 0).then_some(Work::Entries)
    }

    /// What stands between the user and the primary action, if anything does.
    ///
    /// Only the preparing mode can be blocked. Registering entries writes no
    /// part of the archive and is never held up by a running Bitwig, which is
    /// the whole of what the cheap mode buys.
    fn blocking(&self) -> Option<(Tone, &'static str, String)> {
        let Session::Found(found) = &self.session else { return None };
        if self.pending(found) != Some(Work::PrepareThenEntries) {
            return None;
        }
        if self.applying.as_ref().is_some_and(Applying::is_running) {
            return None;
        }
        match (&found.running, found.condition.guard) {
            (RunState::Running(processes), _) => Some((
                Tone::Warn,
                "Quit Bitwig Studio before preparing the installation.",
                format!(
                    "The audio engine holds the files this step has to replace. Running: {}.",
                    processes.join(", ")
                ),
            )),
            (_, orng_tools::GuardState::Unknown) => Some((
                Tone::Err,
                "This installation cannot be prepared.",
                "The tamper guard is not in a shape this build recognises, so preparation \
                 refuses rather than editing it blind."
                    .to_owned(),
            )),
            _ => None,
        }
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
                    let items = [
                        (widget::icon::SETTINGS, "Settings"),
                        (widget::icon::RESTORE, "Restore backup..."),
                        (widget::icon::CHANGE_INSTALL, "Open backups folder"),
                    ];
                    for (icon, label) in items {
                        let _ = widget::menu_item(ui, palette, icon, label);
                    }
                    ui.separator();
                    let _ =
                        widget::menu_item(ui, palette, widget::icon::ABOUT, "About ORNG Registry");
                });
                ui.add_space(metric::TOOL_GAP);
                if widget::small_button(
                    ui,
                    palette,
                    widget::icon::CHANGE_INSTALL,
                    "Change install",
                )
                .clicked()
                {
                    self.locate(ui);
                }
                ui.add_space(metric::GAP);

                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    self.install_identity(ui);
                });
            });
        });
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

    /// Let the user point at an installation themselves.
    fn locate(&mut self, ui: &egui::Ui) {
        let Some(root) =
            rfd::FileDialog::new().set_title("Locate Bitwig Studio").pick_folder()
        else {
            return;
        };
        // A folder the user insisted on. Refusing it has to say why against
        // that folder rather than fall back to the one already loaded, which
        // would look like the picker did nothing.
        self.session = match orng_tools::Installation::at(&root) {
            Ok(install) => Session::at(install),
            Err(e) => {
                Session::Unreadable { root: root.display().to_string(), why: e.to_string() }
            }
        };
        let _ = ui;
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
                RichText::new(revision).font(font::mono(font::MONO_TIGHT)).color(palette.ink_2),
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
        let label = (state != Badge::Registered(found.entries.entries().len()))
            .then(|| state.label());
        let width = label.as_ref().map_or(0.0, |text| text.len() as f32 * BADGE_WIDTH_PER_CHAR);
        let room = (ui.available_width() - width - metric::GAP).max(0.0);
        ui.allocate_ui_with_layout(
            vec2(room, ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| path(ui, palette, &found.to.install.root().display().to_string()),
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
                    found.entries.entries().iter().filter(|e| e.kind == kind).count();
                let staged = self
                    .staged
                    .iter()
                    .filter_map(Staged::registration)
                    .filter(|r| r.kind == kind)
                    .count();
                (kind, registered + staged)
            })
            .collect();

        ui.horizontal_centered(|ui| {
            widget::search_field(ui, palette, &mut self.filter.query, "Search name or UUID");
            ui.add_space(metric::TOOL_GAP);
            for (kind, count) in counts {
                let on = self.filter.kinds.contains(&kind);
                if widget::filter_chip(ui, palette, plural(kind), count, on).clicked() {
                    if on {
                        self.filter.kinds.remove(&kind);
                    } else {
                        self.filter.kinds.insert(kind);
                    }
                }
                ui.add_space(metric::SNUG);
            }

            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if widget::small_button(ui, palette, widget::icon::ADD_FILES, "Add files...")
                    .clicked()
                {
                    self.add_files(ui);
                }
            });
        });
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
                    widget::Pressed::Action => self.locate(ui),
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
                    widget::Pressed::Alt => self.locate(ui),
                    widget::Pressed::Nothing => {}
                }
            }
            Session::Found(_) if self.view == View::Catalog => {
                let palette = self.palette;
                let catalog =
                    self.catalog.get_or_insert_with(|| Fetching::start(ui.ctx().clone()));
                published(ui, palette, catalog);
            }
            Session::Found(_) => self.local(ui),
        }

        // Over everything, including the bars, because the whole window is the
        // target and a drop is not aimed at a region of it.
        self.hovering(ui);
    }

    /// The Local view: what is pending, then what is registered.
    fn local(&mut self, ui: &mut egui::Ui) {
        if let Some(applying) = self.applying.as_ref().filter(|a| a.steps.is_some()) {
            progress(ui, self.palette, applying);
            return;
        }
        let Session::Found(found) = &self.session else { return };

        // A dropped document that is already registered updates that entry
        // rather than adding a second, so it is one piece of pending work and
        // gets one row - the staged one, which is the one that can be acted on.
        let staged: Vec<&Registration> =
            self.staged.iter().filter_map(Staged::registration).collect();
        let registered: Vec<&Registration> = found
            .entries
            .entries()
            .iter()
            .filter(|entry| !staged.iter().any(|pending| pending.uuid == entry.uuid))
            .filter(|entry| self.filter.accepts(entry))
            .collect();
        let shown: Vec<&Staged> = self
            .staged
            .iter()
            .filter(|s| s.registration().is_none_or(|r| self.filter.accepts(r)))
            .collect();

        if found.entries.is_empty() && self.staged.is_empty() {
            // The primary onboarding surface, and the only screen whose icon
            // takes the accent: it is an invitation rather than a report.
            let empty = widget::Empty {
                icon: widget::icon::DROP,
                inviting: true,
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
        widget::list(ui, |ui| {
            // Pending work first, which is the designer's recommendation and
            // the only ordering under which the list answers "what am I about to
            // do" without scrolling.
            if !shown.is_empty() {
                widget::section(ui, palette, "Pending", palette.accent_text, shown.len());
                for pending in &shown {
                    staged_row(ui, palette, pending);
                }
            }
            widget::section(ui, palette, "Registered", palette.ink_2, registered.len());
            for entry in &registered {
                row(ui, palette, entry);
            }
        });
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
        // operating system's.
        let acceptable = staging::documents_in(&hovered);
        let names: Vec<String> = acceptable
            .iter()
            .map(|path| path.file_name().unwrap_or(path.as_os_str()).to_string_lossy().into_owned())
            .collect();
        let heading = match names.len() {
            0 => "Nothing here can be registered".to_owned(),
            1 => "Drop to stage 1 document".to_owned(),
            many => format!("Drop to stage {many} documents"),
        };
        widget::drop_target(ui, self.palette, &heading, &names);
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
                                RichText::new(&summary)
                                    .font(font::plain(font::CONTROL))
                                    .color(tone.colour(palette)),
                            )
                            .truncate(),
                        );
                        if !note.is_empty() {
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&note)
                                        .font(font::plain(font::NOTE))
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
        if let Some(applying) = &self.applying {
            return match (applying.is_running(), applying.stage, &applying.outcome) {
                (true, Stage::Preparing, _) => {
                    ("Preparing the installation".to_owned(), Tone::Warn, String::new())
                }
                (true, Stage::Registering, _) => {
                    ("Registering".to_owned(), Tone::Warn, String::new())
                }
                (false, _, Some(Ok(_))) => (
                    "Done.".to_owned(),
                    Tone::Neutral,
                    "Restart Bitwig Studio to see your changes.".to_owned(),
                ),
                // The failure itself is drawn where there is room for it. Here
                // it only has to stop reading as success.
                (false, _, _) => (
                    "Nothing was applied.".to_owned(),
                    Tone::Err,
                    "Your installation was not changed.".to_owned(),
                ),
            };
        }

        let mut parts = Vec::new();
        let ready = self.ready().count();
        if ready > 0 {
            parts.push(format!("{ready} to add"));
        }
        let to_fix = self.staged.len() - ready;
        if to_fix > 0 {
            parts.push(format!("{to_fix} to fix"));
        }
        // What the press costs, which is the difference between the two modes
        // and the thing a user is entitled to know before pressing rather than
        // after.
        let (note, tone) = match self.pending(found) {
            Some(Work::PrepareThenEntries) => {
                ("Prepare install . a backup is written first", Tone::Warn)
            }
            Some(Work::Entries) => ("Update entries . Bitwig may stay open", Tone::Neutral),
            None => ("", Tone::Neutral),
        };
        if !parts.is_empty() {
            return (parts.join(", "), tone, note.to_owned());
        }
        // Nothing is staged. What the press is *for* then depends on the mode:
        // preparing puts the entries already on record back into effect, which
        // is the second most common session there is, and saying "nothing
        // pending" beside an enabled button that does something would be wrong.
        let registered = found.entries.entries().len();
        match (self.pending(found), registered) {
            (Some(Work::PrepareThenEntries), 0) => (
                "Nothing staged yet".to_owned(),
                Tone::Neutral,
                "Drop documents onto the window, or use Add files...".to_owned(),
            ),
            (Some(Work::PrepareThenEntries), 1) => {
                ("1 entry to restore".to_owned(), tone, note.to_owned())
            }
            (Some(Work::PrepareThenEntries), many) => {
                (format!("{many} entries to restore"), tone, note.to_owned())
            }
            _ => ("Nothing pending".to_owned(), Tone::Neutral, note.to_owned()),
        }
    }

    /// The one button, and what it would do.
    fn action(&mut self, ui: &mut egui::Ui) {
        let palette = self.palette;
        let found = match &self.session {
            Session::Found(found) => found,
            Session::Unreadable { .. } => {
                let why = "This installation could not be read";
                widget::primary_button(ui, palette, "Apply changes", icon::APPLY, false, why);
                return;
            }
            Session::NoInstallation { .. } => {
                let why = "No installation selected";
                widget::primary_button(ui, palette, "Apply changes", icon::APPLY, false, why);
                return;
            }
        };

        if let Some(applying) = &self.applying {
            if applying.is_running() {
                widget::primary_button(ui, palette, "Applying", icon::APPLY, false, "In progress");
                return;
            }
            if widget::small_button(ui, palette, icon::APPLY, "Done").clicked() {
                self.applying = None;
            }
            return;
        }

        let pending = self.pending(found);
        let label = match pending {
            Some(Work::PrepareThenEntries) => "Prepare installation".to_owned(),
            Some(Work::Entries) => match self.ready().count() {
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
        if work == Work::PrepareThenEntries
            && found.entries.is_empty()
            && self.ready().count() == 0
        {
            widget::primary_button(ui, palette, &label, mark, false, "Nothing to register yet");
            return;
        }
        // Only preparation is blocked by a running Bitwig or an unrecognised
        // guard, and the banner above has already said which. The button states
        // it too, because a disabled control with no reason on it is not a
        // statement.
        if let Some((_, why, _)) = self.blocking() {
            widget::primary_button(ui, palette, &label, mark, false, why);
            return;
        }
        if widget::primary_button(ui, palette, &label, mark, true, "").clicked() {
            self.start(work, ui.ctx());
        }
    }

    /// Hand the pending work to a thread that is not this one.
    fn start(&mut self, work: Work, ctx: &egui::Context) {
        let Session::Found(found) = &self.session else { return };
        let mut update = Update::to(found.entries.clone());
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
        self.absorbed = false;
        self.applying = Some(Applying::start(work, found.to.clone(), update, ctx.clone()));
    }
}

/// The installation's name, the loudest thing in the window.
fn title(ui: &mut egui::Ui, text: &str, ink: egui::Color32) {
    ui.label(RichText::new(text).font(font::emphasis(ui.ctx(), font::INSTALL_TITLE)).color(ink));
}

/// Where it is, truncated, and whole on hover.
fn path(ui: &mut egui::Ui, palette: Palette, root: &str) {
    ui.add(
        egui::Label::new(RichText::new(root).font(font::mono(font::MONO)).color(palette.ink_3))
            .truncate(),
    )
    .on_hover_text(root);
}

/// What state the registry is in, coloured as the design colours it.
fn badge(ui: &mut egui::Ui, palette: Palette, label: &str) {
    ui.label(
        RichText::new(label)
            .font(font::plain(font::CHIP))
            .color(widget::badge_colour(palette, label)),
    );
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

/// A preparation, step by step.
fn progress(ui: &mut egui::Ui, palette: Palette, applying: &Applying) {
    ui.add_space(metric::PAD);
    ui.horizontal(|ui| {
        ui.add_space(metric::PAD);
        ui.vertical(|ui| {
            ui.label(
                RichText::new("Preparing this installation")
                    .font(font::emphasis(ui.ctx(), font::HEADING))
                    .color(palette.ink),
            );
            ui.add_space(metric::TIGHT);
            ui.label(
                RichText::new(
                    "Nothing in the installation changes until the patched archive verifies.",
                )
                .font(font::plain(font::NOTE))
                .color(palette.ink_3),
            );
            ui.add_space(metric::GAP);

            for (step, state) in applying.steps.iter().flatten() {
                widget::step_row(ui, palette, step_label(*step), *state);
            }

            if let Some(Err(why)) = &applying.outcome {
                ui.add_space(metric::GAP);
                // The transaction's promise, said plainly. Nothing was restored,
                // because nothing was touched.
                widget::failure(ui, palette, "Your installation was not changed.");
                ui.add_space(metric::TIGHT);
                ui.label(RichText::new(why).font(font::mono(font::MONO)).color(palette.ink_3));
            }
        });
    });
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

/// The first segment of an identity, which is what a row has room for.
fn short_uuid(registration: &Registration) -> String {
    registration.uuid.to_string().split('-').next().unwrap_or_default().to_owned()
}

/// One registered entry.
fn row(ui: &mut egui::Ui, palette: Palette, entry: &Registration) {
    widget::row(ui, palette, |ui, columns| {
        widget::cell(ui, columns.kind, Align::Min, |ui| {
            widget::kind_label(ui, palette, entry.kind);
        });
        widget::cell(ui, columns.name, Align::Min, |ui| {
            ui.add(
                egui::Label::new(
                    RichText::new(&entry.name)
                        .font(font::emphasis(ui.ctx(), font::ROW_NAME))
                        .color(palette.ink),
                )
                .truncate(),
            )
            .on_hover_text(entry.library_path.as_str());
        });
        widget::cell(ui, columns.uuid, Align::Min, |ui| {
            identity(ui, palette, entry);
        });
        widget::cell(ui, columns.status, Align::Max, |ui| {
            let status = "Registered";
            ui.label(
                RichText::new(status)
                    .font(font::plain(font::CHIP))
                    .color(widget::status_colour(palette, status)),
            );
        });
    });
}

/// An identity, short enough for a column and whole on hover. Clicking copies
/// it, because a UUID is a thing people paste into bug reports and nobody
/// transcribes one by hand.
fn identity(ui: &mut egui::Ui, palette: Palette, entry: &Registration) {
    let full = entry.uuid.to_string();
    let response = ui
        .add(
            egui::Label::new(
                RichText::new(short_uuid(entry)).font(font::mono(font::MONO)).color(palette.ink_3),
            )
            .sense(egui::Sense::click()),
        )
        .on_hover_text(format!("{full}\nClick to copy"));
    if response.clicked() {
        ui.ctx().copy_text(full);
    }
}

/// One dropped document, and what can be done with it.
fn staged_row(ui: &mut egui::Ui, palette: Palette, staged: &Staged) {
    widget::row(ui, palette, |ui, columns| {
        widget::cell(ui, columns.kind, Align::Min, |ui| {
            // A rejected row has no kind, because nothing readable said what it
            // was. Drawing one would be inventing it.
            match staged.registration() {
                Some(registration) => widget::kind_label(ui, palette, registration.kind),
                None => {
                    ui.label(RichText::new("-").font(font::plain(font::CHIP)).color(palette.ink_3));
                }
            }
        });
        widget::cell(ui, columns.name, Align::Min, |ui| {
            ui.label(
                RichText::new(&staged.label)
                    .font(font::emphasis(ui.ctx(), font::ROW_NAME))
                    .color(if staged.is_ready() { palette.ink } else { palette.ink_2 }),
            );
            // The reason sits beside the name, in the colour of the status it
            // explains, so an explanation is never louder than the word it
            // belongs to.
            if let Some(why) = staged.reason() {
                ui.add_space(TIGHT_INSIDE_A_ROW);
                ui.add(
                    egui::Label::new(
                        RichText::new(why)
                            .font(font::plain(font::NOTE))
                            .color(widget::status_colour(palette, staged.status())),
                    )
                    .truncate(),
                );
            }
        });
        widget::cell(ui, columns.uuid, Align::Min, |ui| match staged.registration() {
            Some(registration) => identity(ui, palette, registration),
            None => {
                ui.label(RichText::new("-").font(font::mono(font::MONO)).color(palette.ink_3));
            }
        });
        widget::cell(ui, columns.status, Align::Max, |ui| {
            ui.label(
                RichText::new(staged.status())
                    .font(font::plain(font::CHIP))
                    .color(widget::status_colour(palette, staged.status())),
            );
        });
    });
}

/// Between a name and the reason beside it, which is closer than two separate
/// things but further than one phrase.
const TIGHT_INSIDE_A_ROW: f32 = 3.0;

/// The Catalog view: what ORNG Catalog publishes, once it has been proved.
fn published(ui: &mut egui::Ui, palette: Palette, catalog: &Fetching) {
    match catalog.outcome.as_ref() {
        None => {
            let empty = widget::Empty {
                icon: icon::CATALOG,
                inviting: false,
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
        Some(Ok(index)) => {
            widget::list(ui, |ui| {
                widget::section(ui, palette, "Catalog", palette.ink_2, index.items.len());
                for entry in &index.items {
                    widget::row(ui, palette, |ui, columns| {
                        widget::cell(ui, columns.kind, Align::Min, |ui| {
                            widget::kind_label(ui, palette, entry.kind.into());
                        });
                        widget::cell(ui, columns.name, Align::Min, |ui| {
                            ui.label(
                                RichText::new(&entry.name)
                                    .font(font::emphasis(ui.ctx(), font::ROW_NAME))
                                    .color(palette.ink),
                            );
                            ui.add_space(TIGHT_INSIDE_A_ROW);
                            // A catalog row leads with what the item is and who
                            // made it: the author is the trust signal, because
                            // an item is DSP that Bitwig will run.
                            ui.add(
                                egui::Label::new(
                                    RichText::new(&entry.description)
                                        .font(font::plain(font::NOTE))
                                        .color(palette.ink_3),
                                )
                                .truncate(),
                            );
                        });
                        widget::cell(ui, columns.uuid, Align::Min, |ui| {
                            ui.label(
                                RichText::new(entry.author.to_string())
                                    .font(font::plain(font::CHIP))
                                    .color(palette.ink_2),
                            );
                        });
                        widget::cell(ui, columns.status, Align::Max, |ui| {
                            ui.label(
                                RichText::new(entry.version.to_string())
                                    .font(font::mono(font::MONO))
                                    .color(palette.ink_3),
                            );
                        });
                    });
                }
            });
        }
    }
}
