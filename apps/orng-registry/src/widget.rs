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
