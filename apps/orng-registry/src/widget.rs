// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! The pieces the interface is built from.
//!
//! Every frame, chip, row and heading the design repeats lives here once. A
//! screen composes these and never reaches for a colour, a margin or a corner
//! radius of its own - which is the whole point, because the second place a
//! value is written is the place that stops matching when the first one changes,
//! and there is a whole second palette waiting to prove it.
//!
//! These are the design bundle's own components, roughly one function per
//! component: `InstallBar`, `ListToolbar`, `EntryRow`, `ActionBar`. Keeping the
//! boundary where the design put it is what makes a revision of the bundle
//! reviewable against this file rather than against the whole window.
//!
//! Anything that takes a [`Palette`] takes it as an argument rather than reading
//! a global, so a preview or a test can draw the same widget in either theme.

use eframe::egui::{
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Rect, Response, RichText, Sense,
    Stroke, Ui, vec2,
};
use orng_tools::Kind;

use crate::theme::{Palette, font, metric};

/// The frame behind the install bar and the action bar.
pub fn bar(palette: Palette) -> Frame {
    Frame::new().fill(palette.panel).inner_margin(Margin::symmetric(metric::PAD as i8, 0))
}

/// The frame behind the list toolbar, which sits on the page rather than on a
/// panel: the design separates the bars from the working area by colour and not
/// by a line.
pub fn toolbar(palette: Palette) -> Frame {
    Frame::new().fill(palette.bg).inner_margin(Margin::symmetric(metric::PAD as i8, 0))
}

/// The frame behind the list itself.
pub fn page(palette: Palette) -> Frame {
    Frame::new().fill(palette.row).inner_margin(Margin::ZERO)
}

/// One of the two top-level views.
///
/// Drawn as a chip that fills when it is the current view, which is what the
/// design does. Not an underline and not a radio button: this is a switch
/// between two activities, and it has to read as one of them being held.
pub fn view_tab(ui: &mut Ui, palette: Palette, label: &str, current: bool) -> Response {
    let text = RichText::new(label)
        .font(if current {
            font::emphasis(ui.ctx(), font::CONTROL)
        } else {
            font::plain(font::CONTROL)
        })
        .color(if current { palette.ink } else { palette.ink_3 });

    ui.add(
        egui::Button::new(text)
            .fill(if current { palette.btn } else { Color32::TRANSPARENT })
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(metric::RADIUS))
            .min_size(vec2(0.0, metric::CONTROL)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A filter that is either on or off, with the number it would show.
///
/// The count is inside the control rather than beside it: the question a kind
/// filter answers is "how many of these are there", and a toggle that hides the
/// answer until it is pressed makes the user press it to find out.
pub fn filter_chip(ui: &mut Ui, palette: Palette, label: &str, count: usize, on: bool) -> Response {
    let ink = if on { palette.ink } else { palette.ink_3 };
    let mut text = egui::text::LayoutJob::default();
    text.append(
        label,
        0.0,
        egui::TextFormat { font_id: font::plain(font::CHIP), color: ink, ..Default::default() },
    );
    text.append(
        &count.to_string(),
        metric::TIGHT,
        egui::TextFormat {
            font_id: font::mono(font::MONO_TIGHT),
            color: palette.ink_3,
            ..Default::default()
        },
    );

    ui.add(
        egui::Button::new(text)
            .fill(if on { palette.btn } else { Color32::TRANSPARENT })
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(metric::RADIUS))
            .min_size(vec2(0.0, metric::CONTROL)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A quiet control in a bar: `Change install`, `Add files...`.
pub fn small_button(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).font(font::plain(font::CHIP)).color(palette.ink))
            .fill(palette.btn)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(metric::RADIUS))
            .min_size(vec2(0.0, metric::CONTROL)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A control that is on or off, and says which by a mark rather than a fill.
pub fn check(ui: &mut Ui, palette: Palette, label: &str, on: bool) -> Response {
    let mark = if on { "[x]" } else { "[ ]" };
    let mut text = egui::text::LayoutJob::default();
    text.append(
        mark,
        0.0,
        egui::TextFormat {
            font_id: font::mono(font::MONO),
            color: if on { palette.accent } else { palette.ink_3 },
            ..Default::default()
        },
    );
    text.append(
        label,
        metric::TIGHT,
        egui::TextFormat {
            font_id: font::plain(font::CHIP),
            color: if on { palette.ink_2 } else { palette.ink_3 },
            ..Default::default()
        },
    );
    ui.add(
        egui::Button::new(text)
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(vec2(0.0, metric::CONTROL)),
    )
    .on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The one thing the window does, drawn as the one thing the window does.
///
/// Filled with the accent when it can be pressed and with the button fill when
/// it cannot, which is the design's way of saying that a disabled primary is
/// still the primary. The reason it cannot be pressed is carried on it rather
/// than left to be guessed at.
pub fn primary_button(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    enabled: bool,
    reason: &str,
) -> Response {
    let (fill, ink) =
        if enabled { (palette.accent, palette.accent_ink) } else { (palette.btn, palette.ink_3) };
    let text = RichText::new(label).font(font::emphasis(ui.ctx(), font::ACTION)).color(ink);
    let button = egui::Button::new(text)
        .fill(fill)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(vec2(0.0, metric::ACTION));

    let response = ui.add_enabled(enabled, button);
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response.on_disabled_hover_text(reason)
    }
}

/// The three dots of the overflow control. Small enough to read as punctuation
/// rather than as content, which is what the control is.
const DOT_RADIUS: f32 = 1.3;
const DOT_SPACING: f32 = 4.0;

/// The overflow control, and the menu behind it.
///
/// Three dots painted rather than set in an icon font: the design's icon set is
/// not redistributable with this application, and a glyph borrowed from
/// somewhere else would be the one thing on screen that is not the design's.
pub fn overflow(ui: &mut Ui, palette: Palette, menu: impl FnOnce(&mut Ui)) {
    let response = ui.add(
        egui::Button::new("")
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(metric::RADIUS))
            .min_size(vec2(metric::CONTROL, metric::CONTROL)),
    );
    let colour = if response.hovered() { palette.ink } else { palette.ink_2 };
    let centre = response.rect.center();
    for step in [-1.0f32, 0.0, 1.0] {
        ui.painter().circle_filled(
            egui::pos2(centre.x, centre.y + step * DOT_SPACING),
            DOT_RADIUS,
            colour,
        );
    }
    response.on_hover_cursor(egui::CursorIcon::PointingHand).context_menu(menu);
}

/// Wide enough for the longest item the menu carries, so the menu does not
/// change width with what is in it.
const MENU_WIDTH: f32 = 176.0;

/// One line of the overflow menu.
pub fn menu_item(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    ui.add(
        egui::Button::new(RichText::new(label).font(font::plain(font::CONTROL)).color(palette.ink))
            .fill(Color32::TRANSPARENT)
            .stroke(Stroke::NONE)
            .min_size(vec2(MENU_WIDTH, 0.0)),
    )
}

/// A heading dividing the list into what is pending, what is registered and
/// what Bitwig shipped.
pub fn section(ui: &mut Ui, palette: Palette, title: &str, tone: Color32, count: usize) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), metric::SECTION), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.row_alt);

    let mut line = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(metric::PAD, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    line.label(RichText::new(title).font(font::emphasis(ui.ctx(), font::CHIP)).color(tone));
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(
            RichText::new(count.to_string())
                .font(font::mono(font::MONO_TIGHT))
                .color(palette.ink_3),
        );
    });
}

/// The columns of an entry row, as the design's grid has them.
///
/// Computed together because they depend on each other: the name column is
/// whatever the four fixed ones leave, and working that out twice is how two
/// rows come to disagree about where a column starts.
pub struct Columns {
    pub kind: Rect,
    pub name: Rect,
    pub uuid: Rect,
    pub status: Rect,
}

impl Columns {
    fn across(row: Rect) -> Columns {
        let inner = row.shrink2(vec2(metric::PAD, 0.0));
        let fixed = metric::KIND_COLUMN + metric::UUID_COLUMN + metric::STATUS_COLUMN;
        let name = (inner.width() - fixed - 3.0 * metric::GAP).max(0.0);

        let mut x = inner.left();
        let mut take = |width: f32| {
            let cell = Rect::from_min_size(egui::pos2(x, inner.top()), vec2(width, inner.height()));
            x += width + metric::GAP;
            cell
        };
        Columns {
            kind: take(metric::KIND_COLUMN),
            name: take(name),
            uuid: take(metric::UUID_COLUMN),
            status: take(metric::STATUS_COLUMN),
        }
    }
}

/// One row of the list: a fixed height, a hover highlight, and five columns.
///
/// The columns are the point. A row laid out as a flow puts every identity and
/// every status at a different x, so a list of them cannot be read down; the
/// design lays a row out as a grid, and so does this.
pub fn row(ui: &mut Ui, palette: Palette, contents: impl FnOnce(&mut Ui, &Columns)) -> Response {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), metric::ROW), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.row_hover);
    }
    let columns = Columns::across(rect);
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    contents(&mut content, &columns);
    response
}

/// Put one piece of a row in its column.
pub fn cell(ui: &mut Ui, at: Rect, align: Align, contents: impl FnOnce(&mut Ui)) {
    let layout = match align {
        Align::Max => Layout::right_to_left(Align::Center),
        _ => Layout::left_to_right(Align::Center),
    };
    let mut column = ui.new_child(egui::UiBuilder::new().max_rect(at).layout(layout));
    contents(&mut column);
}

/// The kind of a document, as a quiet word rather than a coloured badge.
///
/// The design deliberately gives all three the same colour: kind is a fact about
/// the document, not a status, and colouring it would compete with the states
/// that do need attention.
pub fn kind_label(ui: &mut Ui, palette: Palette, kind: Kind) {
    let label = match kind {
        Kind::Device => "Device",
        Kind::Modulator => "Modulator",
        Kind::Module => "Grid module",
    };
    ui.label(RichText::new(label).font(font::plain(font::CHIP)).color(palette.ink_3));
}

/// How loud a row's status is, as the design's own map from status to token.
///
/// Transcribed here so that a status added to one screen cannot be drawn a
/// different shade on another, and so the reason beside a row is the same colour
/// as the word it explains. A status this does not know is drawn as quietly as
/// `Registered`, which is the design's own fallback: an unfamiliar word should
/// not shout.
pub fn status_colour(palette: Palette, status: &str) -> Color32 {
    match status {
        "Staged" | "Pending restart" => palette.ink_2,
        "Changed" | "Update available" => palette.accent_text,
        "Missing file" | "Conflict" => palette.err_text,
        _ => palette.ink_3,
    }
}

/// What state the installation's registry is in, as the design colours it.
pub fn badge_colour(palette: Palette, badge: &str) -> Color32 {
    match badge {
        "Needs re-apply" => palette.accent_text,
        "Modified elsewhere" => palette.err_text,
        _ => palette.ink_2,
    }
}

/// Text that has to carry a tone as well as a word.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Neutral,
    Warn,
    Err,
}

impl Tone {
    pub fn colour(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.ink_2,
            Tone::Warn => palette.accent_text,
            Tone::Err => palette.err_text,
        }
    }

    fn wash(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.info_bg,
            Tone::Warn => palette.warn_bg,
            Tone::Err => palette.err_bg,
        }
    }
}

/// A banner across the working area: something is in the way, and here is what.
///
/// Two lines, because one is never enough for a condition the user has to act
/// on: what is true, and what it means for the thing they were about to do.
pub fn banner(ui: &mut Ui, palette: Palette, tone: Tone, title: &str, body: &str) {
    Frame::new()
        .fill(tone.wash(palette))
        .inner_margin(Margin::symmetric(metric::PAD as i8, metric::TIGHT as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.label(
                RichText::new(title)
                    .font(font::emphasis(ui.ctx(), font::CONTROL))
                    .color(tone.colour(palette)),
            );
            ui.label(RichText::new(body).font(font::plain(font::NOTE)).color(palette.ink_3));
        });
}

/// A view with nothing in it, or nothing that can be shown.
///
/// Centred, quiet, and always two lines: what the situation is, and what to do
/// about it. A heading with no second line reads as an error even when it is
/// only an empty list.
pub fn empty_state(ui: &mut Ui, palette: Palette, heading: &str, detail: &str) {
    ui.vertical_centered(|ui| {
        ui.add_space(ui.available_height() * 0.3);
        ui.label(
            RichText::new(heading).font(font::emphasis(ui.ctx(), font::HEADING)).color(palette.ink),
        );
        ui.add_space(metric::TIGHT);
        ui.label(RichText::new(detail).font(font::plain(font::CONTROL)).color(palette.ink_3));
    });
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
    ui.horizontal(|ui| {
        ui.label(RichText::new(mark).font(font::mono(font::MONO)).color(colour));
        ui.label(RichText::new(label).font(font::plain(font::CONTROL)).color(colour));
        if state == State::NotRun {
            ui.label(RichText::new("not run").font(font::plain(font::NOTE)).color(palette.ink_3));
        }
    });
}

/// A block of text explaining a failure, in the tone a failure calls for.
pub fn failure(ui: &mut Ui, palette: Palette, why: &str) {
    ui.label(
        RichText::new(why).font(font::emphasis(ui.ctx(), font::CONTROL)).color(palette.err_text),
    );
}

/// How wide the list of files being dropped is. Fixed, because it is what
/// centres the block; a name longer than this is truncated rather than allowed
/// to move the whole listing sideways.
const LISTING_WIDTH: f32 = 340.0;

/// The whole window as a drop target, while something is over it.
///
/// Painted on the foreground layer rather than composed into a panel, because
/// the design covers everything - both bars included - and a drop is not aimed
/// at any particular region of the window.
pub fn drop_target(ui: &mut Ui, palette: Palette, heading: &str, files: &[String]) {
    // The viewport rather than the content area: the whole window is the
    // target, bars included, and the scrim has to reach the edges of what the
    // user is dragging over.
    let window = ui.ctx().viewport_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-target"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(window, CornerRadius::ZERO, palette.scrim);
    painter.rect_filled(window.shrink(metric::GAP), CornerRadius::ZERO, palette.accent_soft);
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
        ui.label(
            RichText::new(heading).font(font::emphasis(ui.ctx(), font::HEADING)).color(palette.ink),
        );
        ui.add_space(metric::GAP);
        // What is being dropped, by name. A count alone cannot be checked
        // against what the pointer is carrying, and a drop is a decision made
        // before it lands.
        for (at, file) in files.iter().enumerate() {
            // A fixed width, so the block is centred as a block and the names
            // line up under one another. A row laid out left to right would
            // take the full width and start at the edge instead.
            ui.allocate_ui_with_layout(
                vec2(LISTING_WIDTH, metric::CONTROL),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(format!("{:>3}", at + 1))
                            .font(font::mono(font::MONO))
                            .color(palette.accent_text),
                    );
                    ui.add(
                        egui::Label::new(
                            RichText::new(file).font(font::mono(font::MONO)).color(palette.ink),
                        )
                        .truncate(),
                    );
                },
            );
        }
    });
}
