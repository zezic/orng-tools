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

/// An icon and a label as one run of text.
///
/// The icon font is a fallback inside the proportional family, so an icon is a
/// character; what it is not is the same size as the words beside it, which is
/// why this is a job and not a string.
fn with_icon(
    ui: &Ui,
    icon: &str,
    label: &str,
    size: f32,
    ink: Color32,
    icon_ink: Color32,
) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    job.append(
        icon,
        0.0,
        egui::TextFormat {
            font_id: font::icon(ui.ctx(), font::ICON),
            color: icon_ink,
            valign: Align::Center,
            ..Default::default()
        },
    );
    if !label.is_empty() {
        job.append(
            label,
            metric::TIGHT,
            egui::TextFormat {
                font_id: font::plain(size),
                color: ink,
                valign: Align::Center,
                ..Default::default()
            },
        );
    }
    job.into()
}

/// Add a button whose fill follows the pointer.
///
/// The fill has to come from the style rather than from `Button::fill`, because
/// an explicit fill wins over every state and the control then never responds to
/// being hovered at all. Set on a scope so that each control can name its own
/// pair without the two sharing one entry in the theme.
fn filled_button(
    ui: &mut Ui,
    base: Color32,
    hover: Color32,
    button: egui::Button<'_>,
) -> Response {
    ui.scope(|ui| {
        let states = &mut ui.style_mut().visuals.widgets;
        states.inactive.weak_bg_fill = base;
        states.hovered.weak_bg_fill = hover;
        states.active.weak_bg_fill = hover;
        ui.add(button)
    })
    .inner
}

/// A quiet control in a bar: `Change install`, `Add files...`.
pub fn small_button(ui: &mut Ui, palette: Palette, icon: &str, label: &str) -> Response {
    let button = egui::Button::new(with_icon(ui, icon, label, font::CHIP, palette.ink, palette.ink_2))
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(vec2(0.0, metric::CONTROL));
    filled_button(ui, palette.btn, palette.btn_hover, button)
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
    icon: &str,
    enabled: bool,
    reason: &str,
) -> Response {
    let (fill, ink) =
        if enabled { (palette.accent, palette.accent_ink) } else { (palette.btn, palette.ink_3) };
    // The label first and the icon after it, as the design has it: the words
    // say what will happen and the arrow says only that something will.
    let mut text = egui::text::LayoutJob::default();
    text.append(
        label,
        0.0,
        egui::TextFormat {
            font_id: font::emphasis(ui.ctx(), font::ACTION),
            color: ink,
            valign: Align::Center,
            ..Default::default()
        },
    );
    if !icon.is_empty() {
        text.append(
            icon,
            metric::TOOL_GAP,
            egui::TextFormat {
                font_id: font::icon(ui.ctx(), font::ICON),
                color: ink,
                valign: Align::Center,
                ..Default::default()
            },
        );
    }
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

/// The overflow control, and the menu behind it.
pub fn overflow(ui: &mut Ui, palette: Palette, menu: impl FnOnce(&mut Ui)) {
    let button = egui::Button::new(
        RichText::new(icon::OVERFLOW).font(font::icon(ui.ctx(), font::ICON)).color(palette.ink_2),
    )
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::RADIUS))
    .min_size(vec2(metric::CONTROL, metric::CONTROL));
    filled_button(ui, Color32::TRANSPARENT, palette.btn_hover, button)
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .context_menu(menu);
}

/// The icons the design names, by the job each does here rather than by the
/// shape it happens to be. One list, so a screen never reaches into the icon
/// crate and picks a different glyph for the same idea.
pub mod icon {
    use egui_phosphor::light;

    pub const OVERFLOW: &str = light::DOTS_THREE_OUTLINE_VERTICAL;
    pub const CHANGE_INSTALL: &str = light::FOLDER_OPEN;
    pub const ADD_FILES: &str = light::FILE_PLUS;
    pub const SEARCH: &str = light::MAGNIFYING_GLASS;
    pub const SETTINGS: &str = light::GEAR_SIX;
    pub const RESTORE: &str = light::CLOCK_COUNTER_CLOCKWISE;
    pub const ABOUT: &str = light::INFO;
    pub const COPY: &str = light::COPY;
    /// On the primary action: an arrow when the press leads somewhere, a tick
    /// when it simply does the thing.
    pub const PREPARE: &str = light::ARROW_RIGHT;
    pub const APPLY: &str = light::CHECK;
    /// The one icon an empty state is built around.
    pub const DROP: &str = light::TRAY_ARROW_DOWN;
    pub const NO_INSTALL: &str = light::FOLDER_DASHED;
    pub const UNREADABLE: &str = light::QUESTION;
    pub const NO_MATCH: &str = light::FUNNEL_X;
    pub const CATALOG: &str = light::PACKAGE;
}

/// Wide enough for the longest item the menu carries, so the menu does not
/// change width with what is in it.
const MENU_WIDTH: f32 = 176.0;

/// One line of the overflow menu.
pub fn menu_item(ui: &mut Ui, palette: Palette, icon: &str, label: &str) -> Response {
    ui.add(
        egui::Button::new(with_icon(ui, icon, label, font::CONTROL, palette.ink, palette.ink_3))
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

/// A screen with nothing on it, as the design writes one.
///
/// Every field here is a field the design's `EmptyState` has, and every string
/// that fills one is the design's own. The copy is the part of an empty state
/// that does the work - it is the only thing on screen - and it had already been
/// written down.
pub struct Empty<'a> {
    pub icon: &'a str,
    /// The accent is for the state that is an invitation. Everything else is
    /// quiet, because nothing here is wrong.
    pub inviting: bool,
    pub title: &'a str,
    pub body: &'a str,
    /// The three extensions a drop accepts, in monospace under the body.
    pub extensions: bool,
    pub aside: Option<&'a str>,
    pub action: Option<&'a str>,
    /// Whether the action is the accent-filled one. A filter matching nothing
    /// offers a way out, not a thing to do, and the design draws it quietly.
    pub action_is_primary: bool,
    pub alt: Option<&'a str>,
    pub foot: Option<&'a str>,
    /// A filter matching nothing is a smaller event than an installation that
    /// cannot be read, and the design draws it smaller.
    pub minor: bool,
}

/// Which of an empty state's two controls was pressed, if either was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pressed {
    Nothing,
    Action,
    Alt,
}

/// How far the corner marks sit in, and how long their arms are.
const MARK_INSET: f32 = 16.0;
const MARK_ARM: f32 = 11.0;
/// How wide the body is allowed to run before it wraps. Prose set the full
/// width of a window is prose nobody finishes.
const BODY_WIDTH: f32 = 430.0;

pub fn empty_state(ui: &mut Ui, palette: Palette, empty: &Empty<'_>) -> Pressed {
    let region = ui.available_rect_before_wrap();
    if !empty.minor {
        corner_marks(ui, palette, region);
    }

    let mut pressed = Pressed::Nothing;
    let title_size = if empty.minor { font::SUBHEADING } else { font::HEADING };
    let icon_size = if empty.minor { font::ICON_SMALL } else { font::ICON_LARGE };
    let icon_ink = if empty.inviting { palette.accent } else { palette.ink_3 };

    ui.vertical_centered(|ui| {
        ui.add_space(region.height() * 0.24);
        ui.label(RichText::new(empty.icon).font(font::icon(ui.ctx(), icon_size)).color(icon_ink));
        ui.add_space(metric::GAP);
        ui.label(
            RichText::new(empty.title)
                .font(font::emphasis(ui.ctx(), title_size))
                .color(palette.ink),
        );
        ui.add_space(metric::TIGHT);
        ui.allocate_ui_with_layout(
            vec2(BODY_WIDTH, 0.0),
            Layout::top_down(Align::Center),
            |ui| {
                ui.label(
                    RichText::new(empty.body).font(font::plain(font::CONTROL)).color(palette.ink_2),
                );
                if empty.extensions {
                    ui.add_space(metric::GAP);
                    ui.label(
                        RichText::new(".bwdevice    .bwmodulator    .bwmodule")
                            .font(font::mono(font::MONO))
                            .color(palette.ink_3),
                    );
                }
                if let Some(aside) = empty.aside {
                    ui.add_space(metric::GAP);
                    ui.label(
                        RichText::new(aside).font(font::plain(font::CHIP)).color(palette.ink_3),
                    );
                }
            },
        );

        if empty.action.is_some() || empty.alt.is_some() {
            ui.add_space(metric::GAP);
            ui.horizontal(|ui| {
                // Centred as a block. A row laid out left to right inside a
                // centred column still starts at the left edge of it.
                let controls = empty.action.iter().chain(empty.alt.iter());
                let width: f32 = controls
                    .map(|label| label.len() as f32 * BUTTON_WIDTH_PER_CHAR + BUTTON_PADDING)
                    .sum::<f32>()
                    + metric::TOOL_GAP;
                ui.add_space((ui.available_width() - width) / 2.0);
                if let Some(action) = empty.action {
                    let hit = if empty.action_is_primary {
                        primary_button(ui, palette, action, "", true, "").clicked()
                    } else {
                        small_button(ui, palette, "", action).clicked()
                    };
                    if hit {
                        pressed = Pressed::Action;
                    }
                }
                if let Some(alt) = empty.alt
                    && small_button(ui, palette, "", alt).clicked()
                {
                    pressed = Pressed::Alt;
                }
            });
        }
        if let Some(foot) = empty.foot {
            ui.add_space(metric::GAP);
            ui.label(RichText::new(foot).font(font::plain(font::NOTE)).color(palette.ink_3));
        }
    });
    pressed
}

/// Roughly how wide a character of button text is, for centring a pair of them
/// before either has been laid out. An estimate, and only ever used to centre:
/// being a few pixels out moves the block, it does not break it.
const BUTTON_WIDTH_PER_CHAR: f32 = 6.2;
const BUTTON_PADDING: f32 = 40.0;

/// The four corner marks the design puts around a full-region empty state.
fn corner_marks(ui: &Ui, palette: Palette, region: Rect) {
    let stroke = Stroke::new(metric::HAIRLINE, palette.line);
    let inner = region.shrink(MARK_INSET);
    let painter = ui.painter();
    for (x, dx) in [(inner.left(), 1.0), (inner.right(), -1.0)] {
        for (y, dy) in [(inner.top(), 1.0f32), (inner.bottom(), -1.0)] {
            let corner = egui::pos2(x, y);
            painter.line_segment([corner, egui::pos2(x + dx * MARK_ARM, y)], stroke);
            painter.line_segment([corner, egui::pos2(x, y + dy * MARK_ARM)], stroke);
        }
    }
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
