// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The pieces the interface is built from.
//!
//! Every frame, stripe, tag and heading the design repeats lives here once. A
//! screen composes these and never reaches for a colour, a margin or a corner
//! radius of its own - which is the whole point, because the second place a
//! value is written is the place that stops matching when the first one changes,
//! and there is a whole second palette waiting to prove it.
//!
//! Anything that takes a [`Palette`] takes it as an argument rather than reading
//! a global, so a preview or a test can draw the same widget in either theme.

use eframe::egui::{
    self, Align, CornerRadius, Frame, Layout, Margin, Response, RichText, Sense, Stroke, Ui, vec2,
};
use orng_tools::Kind;

use crate::theme::{Palette, metric, text};

/// The frame behind a toolbar or a status bar: flat fill, one hairline.
pub fn bar(palette: Palette) -> Frame {
    Frame::new()
        .fill(palette.panel)
        .stroke(Stroke::new(metric::HAIRLINE, palette.line))
        .inner_margin(Margin::symmetric(0, 0))
}

/// The frame behind a view's content.
pub fn page(palette: Palette) -> Frame {
    Frame::new().fill(palette.bg).inner_margin(Margin::same(metric::PAD as i8))
}

/// A top-level view selector.
///
/// Not a radio button: the design draws these as text that gains the accent when
/// it is the current view, with no control chrome at all.
pub fn tab(ui: &mut Ui, palette: Palette, label: &str, current: bool) -> Response {
    let colour = if current { palette.accent_text } else { palette.ink_3 };
    let text = RichText::new(label).text_style(text::BODY).color(colour);
    let response = ui.add(egui::Button::new(text).frame(false).min_size(vec2(0.0, 26.0)));

    // The current view is underlined in the accent, which is what carries the
    // selection once the label colour alone stops being enough.
    if current {
        let rect = response.rect;
        let y = rect.bottom() + metric::TIGHT / 2.0;
        ui.painter().hline(rect.x_range(), y, Stroke::new(metric::UNDERLINE, palette.accent));
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One row of a list, striped and highlighting under the pointer.
///
/// The stripe is by index rather than by any property of the content, so a
/// filtered list stays readable instead of showing runs of one shade.
pub fn row(ui: &mut Ui, palette: Palette, index: usize, contents: impl FnOnce(&mut Ui)) {
    let fill = if index.is_multiple_of(2) { palette.row } else { palette.row_alt };
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), metric::ROW_HEIGHT), Sense::click());

    let fill = if response.hovered() { palette.row_hover } else { fill };
    ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    let hairline = Stroke::new(metric::HAIRLINE, palette.line_soft);
    ui.painter().hline(rect.x_range(), rect.bottom(), hairline);

    let mut content = ui.new_child(
        egui::UiBuilder::new().max_rect(rect.shrink2(vec2(metric::PAD, 0.0))).layout(
            Layout::left_to_right(Align::Center),
        ),
    );
    contents(&mut content);
}

/// The kind of a document, as a quiet tag rather than a coloured badge.
///
/// The design deliberately gives all three the same colour: kind is a fact about
/// the document, not a status, and colouring it would compete with the states
/// that do need attention.
pub fn kind_tag(ui: &mut Ui, palette: Palette, kind: Kind) {
    let label = match kind {
        Kind::Device => "Device",
        Kind::Modulator => "Modulator",
        Kind::Module => "Grid module",
    };
    ui.label(RichText::new(label).text_style(text::SMALL).color(palette.info));
}

/// A view with nothing in it, or nothing that can be shown.
///
/// Centred, quiet, and always two lines: what the situation is, and what to do
/// about it. A heading with no second line reads as an error even when it is
/// only an empty list.
pub fn empty_state(ui: &mut Ui, palette: Palette, heading: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.label(RichText::new(heading).text_style(text::HEADING).color(palette.ink));
        ui.add_space(metric::TIGHT);
        ui.label(RichText::new(detail).text_style(text::BODY).color(palette.ink_3));
    });
}

/// How much attention a piece of text is asking for.
///
/// The mapping from state to colour lives here so two screens reporting the
/// same condition cannot disagree about how alarming it is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Warn,
    Quiet,
}

pub fn toned(palette: Palette, tone: Tone, value: &str) -> RichText {
    let colour = match tone {
        Tone::Warn => palette.warn,
        Tone::Quiet => palette.ink_3,
    };
    RichText::new(value).text_style(text::BODY).color(colour)
}

/// How loud a row's status is, as the design's own map from status to token.
///
/// Transcribed here so that a status added to one screen cannot be drawn a
/// different shade on another, and so the reason under a row is the same colour
/// as the word it explains. A status this does not know is drawn as quietly as
/// `Registered`, which is the design's own fallback: an unfamiliar word should
/// not shout.
pub fn status_colour(palette: Palette, status: &str) -> egui::Color32 {
    match status {
        "Staged" | "Pending restart" => palette.ink_2,
        "Changed" | "Update available" => palette.accent_text,
        "Missing file" | "Conflict" => palette.err_text,
        _ => palette.ink_3,
    }
}

/// What state a row is in, as a word on the right of it.
pub fn status_chip(ui: &mut Ui, palette: Palette, status: &str) {
    let colour = status_colour(palette, status);
    ui.label(RichText::new(status).text_style(text::SMALL).color(colour));
}

/// The whole window as a drop target, while something is over it.
///
/// Painted on the foreground layer rather than composed into a panel, because
/// the design covers everything - the toolbar and the status bar included - and
/// a drop is not aimed at any particular region of the window.
pub fn drop_target(ui: &mut Ui, palette: Palette, heading: &str, files: &[String]) {
    // The viewport rather than the content area: the whole window is the
    // target, bars included, and the scrim has to reach the edges of what the
    // user is dragging over.
    let window = ui.ctx().viewport_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-target"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(window, CornerRadius::ZERO, palette.scrim);
    painter.rect_stroke(
        window.shrink(metric::GAP),
        CornerRadius::ZERO,
        Stroke::new(metric::HAIRLINE, palette.accent),
        egui::StrokeKind::Inside,
    );

    let mut overlay = Ui::new(
        ui.ctx().clone(),
        egui::Id::new("drop-target-contents"),
        egui::UiBuilder::new().layer_id(layer).max_rect(window),
    );
    overlay.vertical_centered(|ui| {
        ui.add_space(window.height() * 0.3);
        ui.label(RichText::new(heading).text_style(text::HEADING).color(palette.ink));
        ui.add_space(metric::GAP);
        // What is being dropped, by name. A count alone cannot be checked
        // against what the pointer is carrying, and a drop is a decision made
        // before it lands.
        for (at, file) in files.iter().enumerate() {
            // A fixed width, so the block is centred as a block and the names
            // line up under one another. A row laid out left to right would
            // take the full width and start at the edge instead.
            ui.allocate_ui_with_layout(
                vec2(LISTING_WIDTH, ui.spacing().interact_size.y),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(format!("{:>3}", at + 1))
                            .text_style(text::MONO)
                            .color(palette.accent_text),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(file).text_style(text::MONO).color(palette.ink),
                        )
                        .truncate(),
                    );
                },
            );
        }
    });
}

/// How wide the list of files being dropped is. Fixed, because it is what
/// centres the block; a name longer than this is truncated rather than allowed
/// to move the whole listing sideways.
const LISTING_WIDTH: f32 = 340.0;

/// One step of a preparation, as a row of the progress list.
///
/// Every step is drawn whether or not this plan runs it. A step that vanished
/// would change the count under a reader who is watching it move, and "not run"
/// is a thing they need to be able to see afterwards.
pub fn step_row(ui: &mut Ui, palette: Palette, label: &str, state: crate::work::State) {
    use crate::work::State;
    let (mark, colour) = match state {
        State::Waiting => ("   ", palette.ink_3),
        State::NotRun => ("  -", palette.ink_3),
        State::Running => ("  >", palette.accent_text),
        State::Done => ("  +", palette.ink_2),
        State::Failed => ("  x", palette.err_text),
    };
    let suffix = match state {
        State::NotRun => "   not run",
        _ => "",
    };
    ui.horizontal(|ui| {
        ui.label(RichText::new(mark).text_style(text::MONO).color(colour));
        ui.label(RichText::new(label).text_style(text::BODY).color(colour));
        if !suffix.is_empty() {
            ui.label(RichText::new(suffix).text_style(text::SMALL).color(palette.ink_3));
        }
    });
}

/// A block of text explaining a failure, in the tone a failure calls for.
pub fn failure(ui: &mut Ui, palette: Palette, why: &str) {
    ui.label(RichText::new(why).text_style(text::BODY).color(palette.err_text));
}
