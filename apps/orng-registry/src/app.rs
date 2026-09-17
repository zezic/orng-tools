// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The window.
//!
//! Draws from a [`Session`] and never from its own memory of what it drew last
//! time. Anything it wants to know about the machine it asks the session for,
//! and the session is re-read after anything that could change the answer.
//!
//! The one thing it does hold of its own is what has been dropped and not yet
//! written. That is pending work rather than a fact about the machine, so it
//! cannot be re-read from anywhere and has to live here until it is applied.

use std::path::PathBuf;

use eframe::egui::{self, Align, Layout, RichText};
use orng_tools::{Registration, RunState, Step, Update};

use crate::catalog::Fetching;
use crate::session::{Found, Session};
use crate::staging::{self, Reading, Staged};
use crate::theme::{self, Palette, metric, text};
use crate::widget;
use crate::work::{Applying, Stage, Work};

/// Which top-level view is showing. Two, as the design has it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    /// What is registered on this machine.
    Local,
    /// What ORNG Catalog publishes.
    Catalog,
}

pub struct App {
    session: Session,
    view: View,
    palette: Palette,
    dark: bool,
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
    pub fn draw(&mut self, ctx: &egui::Context) {
        self.pump();
        self.take_drop(ctx);

        egui::TopBottomPanel::top("views")
            .exact_height(metric::BAR_HEIGHT)
            .frame(widget::bar(self.palette))
            .show(ctx, |ui| self.views(ui));

        egui::TopBottomPanel::bottom("status")
            .exact_height(metric::BAR_HEIGHT)
            .frame(widget::bar(self.palette))
            .show(ctx, |ui| self.status(ui));

        egui::CentralPanel::default()
            .frame(widget::page(self.palette))
            .show(ctx, |ui| self.page(ui));
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
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
    ///
    /// egui collects the drop for us; what is left is deciding which of the
    /// files are ours and reading them somewhere other than here.
    fn take_drop(&mut self, ctx: &egui::Context) {
        let dropped: Vec<PathBuf> = ctx.input(|input| {
            input.raw.dropped_files.iter().filter_map(|file| file.path.clone()).collect()
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

    /// Whether preparation may even be offered.
    fn blocker(found: &Found) -> Option<&'static str> {
        use orng_tools::GuardState;
        match &found.running {
            RunState::Running(_) => Some("Quit Bitwig Studio first"),
            RunState::Clear => match found.condition.guard {
                // Section 4.3: an unrecognised guard is refused, never edited
                // blind. Saying so here is better than a refusal after a click.
                GuardState::Unknown => Some("This build's tamper guard is not recognised"),
                _ => None,
            },
        }
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
}

impl App {
    fn views(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(metric::PAD - metric::TIGHT);
            for (view, label) in [(View::Local, "Local"), (View::Catalog, "Catalog")] {
                if widget::tab(ui, self.palette, label, self.view == view).clicked() {
                    self.view = view;
                }
                ui.add_space(metric::GAP);
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(metric::PAD - metric::TIGHT);
                if ui.button("Rescan").clicked() {
                    self.session = Session::read();
                }
                // Both themes are drawn, so both are reachable. It is also the
                // only way to see that nothing has quietly hard-coded a colour.
                let other = if self.dark { "Light" } else { "Dark" };
                if ui.button(other).clicked() {
                    let dark = !self.dark;
                    self.set_theme(dark, ui.ctx());
                }
            });
        });
    }

    fn page(&mut self, ui: &mut egui::Ui) {
        match &self.session {
            Session::NoInstallation { searched } => {
                widget::empty_state(ui, self.palette, "No Bitwig Studio found", searched);
            }
            Session::Unreadable { root, why } => {
                widget::empty_state(ui, self.palette, "This installation cannot be read", why);
                ui.add_space(metric::TIGHT);
                ui.label(RichText::new(root).text_style(text::MONO).color(self.palette.ink_3));
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
        self.toolbar(ui);
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
            .collect();

        if self.staged.is_empty() && registered.is_empty() {
            // The primary onboarding surface. It names the extensions and says
            // that preparing needs Bitwig closed, because that is the one thing
            // about the first run that is not obvious.
            widget::empty_state(
                ui,
                self.palette,
                "Nothing registered yet",
                &format!(
                    "Drop a device, modulator or Grid module here to register it: {}.",
                    staging::ACCEPTED.map(|e| format!(".{e}")).join("  ")
                ),
            );
            ui.add_space(metric::TIGHT);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new("Bitwig Studio must be closed the first time.")
                        .text_style(text::SMALL)
                        .color(self.palette.ink_3),
                );
            });
            return;
        }

        let palette = self.palette;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            // Pending work first, which is the designer's recommendation and
            // the only ordering under which the list answers "what am I about to
            // do" without scrolling.
            for (at, pending) in self.staged.iter().enumerate() {
                staged_row(ui, palette, pending, at);
            }
            for (at, entry) in registered.iter().enumerate() {
                row(ui, palette, entry, at + self.staged.len());
            }
        });
    }

    /// The start of the list toolbar. Search and the kind filters are the rest
    /// of it and are not built; this is the control that must exist whatever
    /// else does, because drag and drop may not be the only way in.
    fn toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                if ui.button("Add files...").clicked() {
                    let chosen = rfd::FileDialog::new()
                        .set_title("Add documents to register")
                        .add_filter("Bitwig documents", &staging::ACCEPTED)
                        .pick_files()
                        .unwrap_or_default();
                    self.read(staging::documents_in(&chosen), ui.ctx());
                }
            });
        });
        ui.add_space(metric::TIGHT);
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

    fn status(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            ui.add_space(metric::PAD - metric::TIGHT);
            let (line, tone): (String, egui::Color32) = match &self.session {
                Session::NoInstallation { .. } => {
                    ("no installation".to_owned(), self.palette.ink_3)
                }
                Session::Unreadable { .. } => ("unreadable".to_owned(), self.palette.err_text),
                Session::Found(found) => {
                    let build = match &found.condition.build {
                        Some(build) => build.to_string(),
                        None => "unknown build".to_owned(),
                    };
                    // Bitwig being open is what stops a preparation, so it
                    // belongs beside the state rather than behind a dialog.
                    let running = match &found.running {
                        RunState::Running(_) => "  .  Bitwig is running",
                        RunState::Clear => "",
                    };
                    let tone = if found.condition.is_prepared() {
                        self.palette.ink_3
                    } else {
                        self.palette.accent_text
                    };
                    let root = found.to.install.root().display();
                    (format!("{root}  .  {build}  .  {}{running}", found.guard_summary()), tone)
                }
            };
            // The action is placed first, from the right. Laying the line out
            // first leaves the button whatever width is left over, and an
            // installation path is long enough that there is none: the button
            // then hangs off the edge of the window.
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.add_space(metric::PAD - metric::TIGHT);
                self.action(ui);
                ui.add_space(metric::GAP);
                self.summary(ui);
                ui.add_space(metric::GAP);
                // Whatever is left is the line's, and it gives way rather than
                // pushing anything: a path is the least important thing here.
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.add_space(metric::PAD - metric::TIGHT);
                    ui.add(
                        egui::Label::new(
                            RichText::new(line).text_style(text::MONO).color(tone),
                        )
                        .truncate(),
                    );
                });
            });
        });
    }

    /// What one press would do, in words, beside the button that would do it.
    fn summary(&mut self, ui: &mut egui::Ui) {
        let Session::Found(found) = &self.session else { return };
        if let Some(reading) = &self.reading {
            let line = format!("Reading {} documents", reading.count);
            ui.label(widget::toned(self.palette, widget::Tone::Quiet, &line));
            return;
        }
        if let Some(applying) = &self.applying {
            let line = match (applying.is_running(), applying.stage, &applying.outcome) {
                (true, Stage::Preparing, _) => "Preparing the installation",
                (true, Stage::Registering, _) => "Registering",
                (false, _, Some(Ok(_))) => "Restart Bitwig Studio to see your changes.",
                // The failure itself is drawn where there is room for it. Here
                // it only has to stop reading as success.
                (false, _, _) => "Nothing was applied.",
            };
            ui.label(widget::toned(self.palette, widget::Tone::Quiet, line));
            return;
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
        // What the press costs, which is the difference the two modes have and
        // the thing the user is entitled to know before pressing rather than
        // after.
        let note = match self.pending(found) {
            Some(Work::PrepareThenEntries) => Some("Prepare install . a backup is written first"),
            Some(Work::Entries) => Some("Update entries . Bitwig may stay open"),
            None => None,
        };
        let summary = match (parts.is_empty(), note) {
            (true, None) => "Nothing pending".to_owned(),
            (true, Some(note)) => note.to_owned(),
            (false, None) => parts.join(", "),
            (false, Some(note)) => format!("{}  .  {note}", parts.join(", ")),
        };
        ui.add(
            egui::Label::new(widget::toned(self.palette, widget::Tone::Quiet, &summary)).truncate(),
        );
    }

    /// The one button, and what it would do.
    fn action(&mut self, ui: &mut egui::Ui) {
        let Session::Found(found) = &self.session else { return };

        if let Some(applying) = &self.applying {
            if applying.is_running() {
                ui.add_enabled(false, egui::Button::new("Applying"));
                return;
            }
            if ui.button("Done").clicked() {
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

        let Some(work) = pending else {
            ui.add_enabled(false, egui::Button::new(label))
                .on_disabled_hover_text("Nothing to apply");
            return;
        };
        // Only preparation is blocked by a running Bitwig or an unrecognised
        // guard. Registering entries writes no part of the installation's
        // archive and is never held up by either.
        if let (Work::PrepareThenEntries, Some(why)) = (work, App::blocker(found)) {
            // Say why rather than showing a button that refuses. The reason is
            // the useful half; a disabled control with no explanation is not.
            ui.add_enabled(false, egui::Button::new(label)).on_disabled_hover_text(why);
            return;
        }
        if ui.button(label).clicked() {
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

/// A preparation, step by step.
fn progress(ui: &mut egui::Ui, palette: Palette, applying: &Applying) {
    ui.add_space(metric::PAD);
    ui.label(RichText::new("Preparing this installation").text_style(text::HEADING));
    ui.add_space(metric::TIGHT);
    ui.label(
        RichText::new(
            "Nothing in the installation changes until the patched archive verifies.",
        )
        .text_style(text::SMALL)
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
        ui.label(RichText::new(why).text_style(text::MONO).color(palette.ink_3));
    }
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

/// One registered entry.
fn row(ui: &mut egui::Ui, palette: Palette, entry: &Registration, index: usize) {
    widget::row(ui, palette, index, |ui| {
        ui.vertical(|ui| {
            ui.add_space(metric::TIGHT);
            ui.label(RichText::new(&entry.name).text_style(text::BODY).color(palette.ink));
            ui.label(
                RichText::new(entry.library_path.as_str())
                    .text_style(text::MONO)
                    .color(palette.ink_3),
            );
        });
        // Right to left, so the status is the rightmost column as the design
        // has it and the kind sits inside it.
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(metric::PAD);
            widget::status_chip(ui, palette, "Registered");
            ui.add_space(metric::GAP);
            widget::kind_tag(ui, palette, entry.kind);
        });
    });
}

/// One dropped document, and what can be done with it.
fn staged_row(ui: &mut egui::Ui, palette: Palette, staged: &Staged, index: usize) {
    widget::row(ui, palette, index, |ui| {
        ui.vertical(|ui| {
            ui.add_space(metric::TIGHT);
            ui.label(RichText::new(&staged.label).text_style(text::BODY).color(palette.ink));
            // The reason, when there is one, takes the line the library path
            // would have had. A row that cannot be written has nothing useful
            // to say about where it would have gone.
            //
            // Drawn in its status's own colour, so an explanation cannot be
            // louder than the word it explains: a rejected file is a quiet
            // state, and a conflict is not.
            let (detail, tone) = match (staged.reason(), staged.registration()) {
                (Some(why), _) => (why, widget::status_colour(palette, staged.status())),
                (None, Some(registration)) => (registration.library_path.as_str(), palette.ink_3),
                (None, None) => ("", palette.ink_3),
            };
            ui.label(RichText::new(detail).text_style(text::MONO).color(tone));
        });
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(metric::PAD);
            widget::status_chip(ui, palette, staged.status());
            ui.add_space(metric::GAP);
            // A rejected row has no kind, because nothing readable said what it
            // was. Drawing one would be inventing it.
            if let Some(registration) = staged.registration() {
                widget::kind_tag(ui, palette, registration.kind);
            }
        });
    });
}

/// The Catalog view: what ORNG Catalog publishes, once it has been proved.
fn published(ui: &mut egui::Ui, palette: Palette, catalog: &Fetching) {
    match catalog.outcome.as_ref() {
        None => widget::empty_state(ui, palette, "Fetching the catalog", "Checking its signature."),
        Some(Err(why)) => {
            widget::empty_state(
                ui,
                palette,
                "The catalog could not be read",
                "Nothing is installed from an index that does not verify.",
            );
            ui.add_space(metric::TIGHT);
            ui.vertical_centered(|ui| {
                ui.label(RichText::new(why).text_style(text::MONO).color(palette.ink_3));
            });
        }
        Some(Ok(index)) if index.items.is_empty() => {
            widget::empty_state(ui, palette, "The catalog is empty", "Nothing is published yet.")
        }
        Some(Ok(index)) => {
            egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
                for (at, entry) in index.items.iter().enumerate() {
                    widget::row(ui, palette, at, |ui| {
                        ui.vertical(|ui| {
                            ui.add_space(metric::TIGHT);
                            ui.label(
                                RichText::new(&entry.name).text_style(text::BODY).color(palette.ink),
                            );
                            ui.label(
                                RichText::new(format!("{}  {}", entry.author, entry.version))
                                    .text_style(text::MONO)
                                    .color(palette.ink_3),
                            );
                        });
                        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                            ui.add_space(metric::PAD);
                            widget::kind_tag(ui, palette, entry.kind.into());
                        });
                    });
                }
            });
        }
    }
}
