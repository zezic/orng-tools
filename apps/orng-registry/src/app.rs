// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The window.
//!
//! Draws from a [`Session`] and never from its own memory of what it drew last
//! time. Anything it wants to know about the machine it asks the session for,
//! and the session is re-read after anything that could change the answer.

use eframe::egui::{self, Align, Layout, RichText};
use orng_tools::{OrngHome, Registration, RunState, Step, Strategy, UserLibrary};

use crate::session::{Found, Session};
use crate::theme::{self, Palette, metric, text};
use crate::widget;
use crate::work::Preparing;

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
    /// Set while an installation is being prepared, and kept afterwards so the
    /// result stays on screen until the user does something else.
    preparing: Option<Preparing>,
    /// Whether the machine has been re-read since the preparation ended. It is
    /// read once, not every frame: the answer costs seconds.
    reread: bool,
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
        App { session, view: View::Local, palette, dark: true, preparing: None, reread: false }
    }

    pub fn show_view(&mut self, view: View) {
        self.view = view;
    }

    /// Put a preparation on screen without having started one. Tests only.
    #[cfg(test)]
    pub fn set_preparing(&mut self, preparing: Preparing) {
        self.preparing = Some(preparing);
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
        self.pump(ctx);

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
            .show(ctx, |ui| match &self.session {
                Session::NoInstallation { searched } => widget::empty_state(
                    ui,
                    self.palette,
                    "No Bitwig Studio found",
                    searched,
                ),
                Session::Unreadable { root, why } => {
                    widget::empty_state(ui, self.palette, "This installation cannot be read", why);
                    ui.add_space(metric::TIGHT);
                    ui.label(RichText::new(root).text_style(text::MONO).color(self.palette.ink_3));
                }
                Session::Found(found) => match self.view {
                    View::Local if self.preparing.is_some() => {
                        let preparing = self.preparing.as_ref().expect("just checked");
                        progress(ui, self.palette, preparing);
                    }
                    View::Local => local(ui, self.palette, found),
                    View::Catalog => widget::empty_state(
                        ui,
                        self.palette,
                        "Catalog",
                        "Browsing ORNG Catalog is not built yet.",
                    ),
                },
            });
    }
}

impl eframe::App for App {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.draw(ctx);
    }
}

impl App {
    /// Take whatever the worker has said, and re-read the machine once it is
    /// done, because what was true before a preparation is not true after one.
    fn pump(&mut self, ctx: &egui::Context) {
        let Some(preparing) = &mut self.preparing else { return };
        if preparing.poll() {
            ctx.request_repaint();
        }
        if preparing.is_running() {
            // Nothing has happened this frame, but something will, and no input
            // is coming to wake the window up.
            ctx.request_repaint_after(std::time::Duration::from_millis(100));
        } else if preparing.outcome.as_ref().is_some_and(Result::is_ok) && !self.reread {
            self.reread = true;
            self.session = Session::read();
        }
    }

    /// Whether preparation may even be offered.
    fn blocker(found: &crate::session::Found) -> Option<&'static str> {
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
                    let root = found.install.root().display();
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

    /// The one thing this window can do to an installation, so far.
    fn action(&mut self, ui: &mut egui::Ui) {
        let Session::Found(found) = &self.session else { return };

        if let Some(preparing) = &self.preparing {
            if preparing.is_running() {
                ui.label(widget::toned(self.palette, widget::Tone::Warn, "Preparing"));
                return;
            }
            if ui.button("Done").clicked() {
                self.preparing = None;
            }
            return;
        }

        if found.condition.is_prepared() {
            return;
        }
        match App::blocker(found) {
            // Say why rather than showing a button that refuses. The reason is
            // the useful half; a disabled control with no explanation is not.
            Some(why) => {
                ui.label(widget::toned(self.palette, widget::Tone::Quiet, why));
            }
            None => {
                if ui.button("Prepare").clicked() {
                    self.reread = false;
                    self.preparing = Some(Preparing::start(
                        found.install.clone(),
                        UserLibrary::discover().expect("a library that was found a moment ago"),
                        OrngHome::discover().expect("a home that was found a moment ago"),
                        Strategy::Link,
                    ));
                }
            }
        }
    }
}

/// A preparation, step by step.
fn progress(ui: &mut egui::Ui, palette: Palette, preparing: &Preparing) {
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

    for (step, state) in &preparing.steps {
        widget::step_row(ui, palette, step_label(*step), *state);
    }

    if let Some(Err(why)) = &preparing.outcome {
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

/// The Local view: everything this app has registered.
fn local(ui: &mut egui::Ui, palette: Palette, found: &Found) {
    let entries = found.entries.entries();
    if entries.is_empty() {
        widget::empty_state(
            ui,
            palette,
            "Nothing registered yet",
            "Drop a device, modulator or Grid module here to register it.",
        );
        return;
    }

    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        for (index, entry) in entries.iter().enumerate() {
            row(ui, palette, entry, index);
        }
    });
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
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            ui.add_space(metric::PAD);
            widget::kind_tag(ui, palette, entry.kind);
        });
    });
}
