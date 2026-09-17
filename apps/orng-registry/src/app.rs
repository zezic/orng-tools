// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The window.
//!
//! Draws from a [`Session`] and never from its own memory of what it drew last
//! time. Anything it wants to know about the machine it asks the session for,
//! and the session is re-read after anything that could change the answer.

use eframe::egui::{self, Align, Layout, RichText};
use orng_tools::{Registration, RunState};

use crate::session::{Found, Session};
use crate::theme::{self, Palette, metric, text};
use crate::widget;

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
        theme::apply(ctx, palette);
        App { session, view: View::Local, palette, dark: true }
    }

    pub fn show_view(&mut self, view: View) {
        self.view = view;
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
            let (line, tone) = match &self.session {
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
            ui.label(RichText::new(line).text_style(text::MONO).color(tone));
        });
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
