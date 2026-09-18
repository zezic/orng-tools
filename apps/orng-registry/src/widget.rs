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
            .min_size(vec2(0.0, metric::TAB)),
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
    labelled_icon(ui, icon, label, size, ink, icon_ink, metric::TIGHT)
}

/// The same, where the design states a gap of its own rather than a bar's.
fn labelled_icon(
    ui: &Ui,
    icon: &str,
    label: &str,
    size: f32,
    ink: Color32,
    icon_ink: Color32,
    gap: f32,
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
            gap,
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

/// The pair of controls an empty state offers, which are not a bar's.
///
/// `small_button` is a bar control: `metric::CONTROL` (26) tall, in
/// `font::CHIP`, with the label in `ink`. The design draws both empty-state
/// controls at the primary's height with the label in `ink_2`, so borrowing
/// the toolbar's button put a 26-tall control beside a 32-tall one - which is
/// the part of this that was visible without measuring anything.
///
/// Measured off the bundle with the empty state rendered at its own preview
/// size: the action 14 in from each edge in `font::ACTION`, the alternative 12
/// in `font::CONTROL`, both 32 tall, 8 apart.
fn empty_button(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    size: f32,
    pad: f32,
) -> Response {
    let button = egui::Button::new(RichText::new(label).font(font::plain(size)).color(palette.ink_2))
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(vec2(0.0, metric::EMPTY_CONTROL));
    ui.scope(|ui| {
        // The design states the padding across the box, so the height comes
        // from `min_size` alone and the vertical padding must not add to it.
        ui.spacing_mut().button_padding = vec2(pad, 0.0);
        filled_button(ui, palette.btn, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// The empty state's action, where it is not the primary one.
pub fn empty_action(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    empty_button(ui, palette, label, font::ACTION, metric::EMPTY_ACTION_PAD)
}

/// The quieter of the pair, beside the action.
pub fn empty_alt(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    empty_button(ui, palette, label, font::CONTROL, metric::EMPTY_ALT_PAD)
}

/// How wide a run of text is once it has been laid out.
///
/// Asking the font rather than counting characters. A proportional face has no
/// per-character width to multiply by, so an estimate is wrong by an amount
/// that depends on which letters the label happens to contain.
fn text_width(ui: &Ui, label: &str, font: egui::FontId) -> f32 {
    ui.painter()
        .layout_no_wrap(label.to_owned(), font, Color32::PLACEHOLDER)
        .rect
        .width()
}

/// How wide the empty state's controls will be, before either is drawn.
///
/// Needed because the pair is centred, and egui cannot centre a row it has not
/// laid out yet: a horizontal inside a centring column still starts at the
/// column's left edge. So the width is worked out first and the row is pushed
/// half the remainder.
///
/// This replaced an estimate of six-and-a-bit pixels a character plus a
/// constant. On the welcome screen that over-stated `Browse the catalog` and
/// `Add files...` together by 69 pixels, and the pair drew 35 left of centre.
pub fn empty_controls_width(
    ui: &Ui,
    action: Option<&str>,
    action_is_primary: bool,
    alt: Option<&str>,
) -> f32 {
    let mut width = 0.0;
    let mut items = 0;
    if let Some(action) = action {
        width += if action_is_primary {
            text_width(ui, action, font::emphasis(ui.ctx(), font::ACTION))
                + 2.0 * ui.spacing().button_padding.x
        } else {
            text_width(ui, action, font::plain(font::ACTION))
                + 2.0 * metric::EMPTY_ACTION_PAD
        };
        items += 1;
    }
    if let Some(alt) = alt {
        width += text_width(ui, alt, font::plain(font::CONTROL))
            + 2.0 * metric::EMPTY_ALT_PAD;
        items += 1;
    }
    width + metric::TOOL_GAP * (items.max(1) - 1) as f32
}

/// A stack of lines, centred in the bar it sits in.
///
/// egui places a child `Ui` at the top of what is available, because when it is
/// positioned nobody knows yet how tall it will be - so a two-line block inside
/// a centring layout sits against the top edge instead of in the middle. The
/// block is therefore measured first, in a detached sizing pass, and then
/// allocated at that height so the parent's alignment has something to centre.
///
/// The same two-pass shape as [`empty_state`], and for the same reason.
pub fn centred_block(ui: &mut Ui, contents: impl Fn(&mut Ui)) {
    let mut probe = Ui::new(
        ui.ctx().clone(),
        ui.id().with("centred-measure"),
        egui::UiBuilder::new()
            .sizing_pass()
            .invisible()
            .max_rect(ui.available_rect_before_wrap()),
    );
    contents(&mut probe);
    let height = probe.min_rect().height();
    ui.allocate_ui_with_layout(
        vec2(ui.available_width(), height),
        Layout::top_down(Align::Min),
        |ui| contents(ui),
    );
}

/// The search field: one filled, rounded box holding the glyph and the text.
///
/// The icon is *inside* the field in the bundle, not sitting on the bar beside
/// it. That is the difference between a field with an affordance in it and an
/// icon that happens to be next to a box.
pub fn search_field(ui: &mut Ui, palette: Palette, query: &mut String, hint: &str) {
    Frame::new()
        .fill(palette.field)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .inner_margin(Margin::symmetric(metric::TOOL_GAP as i8, 0))
        .show(ui, |ui| {
            ui.set_height(metric::CONTROL);
            ui.horizontal_centered(|ui| {
                ui.label(
                    RichText::new(icon::SEARCH)
                        .font(font::icon(ui.ctx(), font::ICON))
                        .color(palette.ink_3),
                );
                // Stated, not inherited: a bar zeroes egui's own item spacing so
                // that the design's gaps are the only ones, and this is one of
                // the design's gaps.
                ui.add_space(metric::TIGHT);
                // What is left of the field once the glyph, the gap and the two
                // margins have taken theirs. The constant is the whole box, so
                // the box is what matches the bundle rather than the text area
                // inside it.
                let text = metric::SEARCH_FIELD
                    - font::ICON
                    - metric::TIGHT
                    - 2.0 * metric::TOOL_GAP;
                ui.add(
                    egui::TextEdit::singleline(query)
                        .hint_text(hint)
                        .desired_width(text)
                        .font(font::plain(font::CONTROL))
                        // The frame is the one drawn above; a second one inside
                        // it is a box in a box.
                        .frame(Frame::NONE),
                );
            });
        });
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
///
/// The design gives this one both of its measurements, and they are not the
/// same: 26 by 24. `min_size` alone would not produce them - egui lays a button
/// out as its content plus `button_padding`, and takes the larger of the two -
/// so the padding is zeroed here and the size is the whole of what is asked
/// for.
///
/// **The menu is a `Popup` and not a `Response::context_menu`.** That call ends
/// in `Popup::context_menu`, which opens on a *secondary* click and closes
/// explicitly on a primary one, so the three dots did nothing at all when they
/// were clicked and the menu had never been seen. `Popup::menu` toggles on the
/// primary click, which is the press the design draws.
pub fn overflow(ui: &mut Ui, palette: Palette, menu: impl FnOnce(&mut Ui)) {
    let button = egui::Button::new(
        RichText::new(icon::OVERFLOW).font(font::icon(ui.ctx(), font::ICON)).color(palette.ink_2),
    )
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::RADIUS))
    .min_size(vec2(metric::OVERFLOW, metric::TAB));
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = egui::Vec2::ZERO;
            filled_button(ui, Color32::TRANSPARENT, palette.btn_hover, button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        })
        .inner;
    egui::Popup::menu(&response)
        .align(egui::RectAlign::BOTTOM_END)
        // Anchored to a rect that is the control shifted right, because the
        // design hangs the menu off the bar and not off the control: the menu
        // clears the window's edge by eight and the control by twelve.
        .anchor(response.rect.translate(vec2(metric::MENU_OVERHANG, 0.0)))
        .gap(metric::MENU_DROP)
        .frame(menu_frame(palette))
        .show(|ui| {
            // The design draws the items touching, and the pitch is the item's
            // own height. egui's list spacing would add six between each.
            ui.spacing_mut().item_spacing.y = 0.0;
            menu(ui);
        });
}

/// The surface the overflow menu sits on: a panel, a line around it, and the
/// one corner in the design rounded by six rather than by three.
fn menu_frame(palette: Palette) -> egui::Frame {
    egui::Frame::new()
        .fill(palette.panel_2)
        .stroke(Stroke::new(metric::HAIRLINE, palette.line))
        .corner_radius(CornerRadius::same(metric::MENU_RADIUS))
        .inner_margin(egui::Margin::same(metric::MENU_MARGIN))
        .shadow(egui::epaint::Shadow {
            offset: [0, metric::MENU_SHADOW_DROP],
            blur: metric::MENU_SHADOW_BLUR,
            spread: 0,
            color: palette.menu_shadow,
        })
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
    /// Put away something that has already happened.
    pub const DISMISS: &str = light::X;
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

/// One line of the overflow menu.
///
/// The width is the design's `min-width` and is set here rather than on the
/// menu, so the menu is as wide as its widest item asks for and never narrower
/// than the design draws it. The gap from icon to label is the menu's own,
/// which is wider than a bar's.
///
/// The fill goes through [`filled_button`] for the reason written there: a
/// `Button::fill` of transparent wins over every state, and the item would
/// never light up under the pointer.
pub fn menu_item(ui: &mut Ui, palette: Palette, icon: &str, label: &str) -> Response {
    let button = egui::Button::new(labelled_icon(
        ui,
        icon,
        label,
        font::CONTROL,
        palette.ink,
        palette.ink_3,
        metric::MENU_GAP,
    ))
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::RADIUS))
    .min_size(vec2(metric::MENU, metric::MENU_ITEM));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(metric::MENU_PAD_X, metric::MENU_PAD_Y);
        filled_button(ui, Color32::TRANSPARENT, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// The rule between two groups of menu items: a hairline, inset from the
/// menu's padding, with equal air above and below.
pub fn menu_rule(ui: &mut Ui, palette: Palette) {
    ui.add_space(metric::MENU_RULE_GAP);
    let (rect, _) = ui.allocate_exact_size(
        vec2(metric::MENU - 2.0 * metric::MENU_RULE_INSET, metric::HAIRLINE),
        egui::Sense::hover(),
    );
    // Allocated at the item width and then inset, rather than laid out inside a
    // margin: the menu's width comes from its widest child, and a rule that
    // asked for the full width would be what decided it.
    ui.painter().rect_filled(
        rect.translate(vec2(metric::MENU_RULE_INSET, 0.0)),
        CornerRadius::ZERO,
        palette.line,
    );
    ui.add_space(metric::MENU_RULE_GAP);
}

/// The scrolling list, with the rows stacked and nothing between them.
///
/// egui puts `item_spacing` between every allocated widget, and this theme sets
/// six pixels of it - which is right between a label and the thing it labels,
/// and wrong between two rows. The design's rows are thirty-six apart because
/// they are thirty-six tall and touch; with the gap they came out forty-two,
/// and a list of eight was a row and a half taller than the bundle's.
pub fn list(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        contents(ui);
    });
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

/// One column of a row's grid.
#[derive(Debug, Clone, Copy)]
enum Column {
    /// A width the design states.
    Fixed(f32),
    /// The one column that takes whatever the others leave.
    Rest,
}

/// Divide a row into the design's columns.
///
/// The design lays a row out as a grid rather than as a flow, so every identity
/// and every status sits at the same x down the whole list. Written once and
/// given the whole specification at a time, because the flexible column is
/// defined by the fixed ones: working that out per row is how two rows come to
/// disagree about where a column starts.
fn grid<const N: usize>(row: Rect, columns: [Column; N]) -> [Rect; N] {
    let inner = row.shrink2(vec2(metric::PAD, 0.0));
    let fixed: f32 = columns
        .iter()
        .map(|column| match column {
            Column::Fixed(width) => *width,
            Column::Rest => 0.0,
        })
        .sum();
    let rest = (inner.width() - fixed - (N - 1) as f32 * metric::GAP).max(0.0);

    let mut x = inner.left();
    columns.map(|column| {
        let width = match column {
            Column::Fixed(width) => width,
            Column::Rest => rest,
        };
        let cell = Rect::from_min_size(egui::pos2(x, inner.top()), vec2(width, inner.height()));
        x += width + metric::GAP;
        cell
    })
}

/// The columns of an entry row, as the design's grid has them.
pub struct Columns {
    pub kind: Rect,
    pub name: Rect,
    pub uuid: Rect,
    pub status: Rect,
    /// What the row itself can do. Reserved even while nothing is drawn in it:
    /// the design hides these controls off hover rather than removing them, so
    /// the four columns before it do not move when the pointer arrives.
    pub actions: Rect,
}

impl Columns {
    fn across(row: Rect) -> Columns {
        let [kind, name, uuid, status, actions] = grid(
            row,
            [
                Column::Fixed(metric::KIND_COLUMN),
                Column::Rest,
                Column::Fixed(metric::UUID_COLUMN),
                Column::Fixed(metric::STATUS_COLUMN),
                Column::Fixed(metric::ACTIONS_COLUMN),
            ],
        );
        Columns { kind, name, uuid, status, actions }
    }
}

/// The columns of a catalog row. A different grid, because a catalog row
/// answers a different question: not "which of mine is this" but "what is this
/// and who made it", so the identity gives way to the author and the version.
pub struct CatalogColumns {
    pub kind: Rect,
    pub name: Rect,
    pub author: Rect,
    pub version: Rect,
    pub status: Rect,
    pub actions: Rect,
}

impl CatalogColumns {
    fn across(row: Rect) -> CatalogColumns {
        let [kind, name, author, version, status, actions] = grid(
            row,
            [
                Column::Fixed(metric::KIND_COLUMN),
                Column::Rest,
                Column::Fixed(metric::AUTHOR_COLUMN),
                Column::Fixed(metric::VERSION_COLUMN),
                Column::Fixed(metric::CATALOG_STATUS_COLUMN),
                Column::Fixed(metric::CATALOG_ACTIONS_COLUMN),
            ],
        );
        CatalogColumns { kind, name, author, version, status, actions }
    }
}

/// One row of the list: a fixed height, a hover highlight, and five columns.
pub fn row(ui: &mut Ui, palette: Palette, contents: impl FnOnce(&mut Ui, &Columns)) -> Response {
    let (rect, response) = row_frame(ui, palette, metric::ROW);
    let columns = Columns::across(rect);
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    contents(&mut content, &columns);
    response
}

/// One row of the catalog, which is taller: the description sits under the name.
pub fn catalog_row(
    ui: &mut Ui,
    palette: Palette,
    contents: impl FnOnce(&mut Ui, &CatalogColumns),
) -> Response {
    let (rect, response) = row_frame(ui, palette, metric::CATALOG_ROW);
    let columns = CatalogColumns::across(rect);
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    contents(&mut content, &columns);
    response
}

/// The part both rows share: the space, and the fill that follows the pointer.
fn row_frame(ui: &mut Ui, palette: Palette, height: f32) -> (Rect, Response) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.row_hover);
    }
    (rect, response)
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

/// Put a two-line piece of a row in its column, centred as a block.
///
/// The same problem [`centred_block`] solves, in a cell: a child `Ui` is placed
/// before its height is known, so a stack inside a row sits against the top of
/// it. Measured first, then allocated at the height that was measured.
pub fn stacked_cell(ui: &mut Ui, at: Rect, contents: impl Fn(&mut Ui)) {
    let mut probe = Ui::new(
        ui.ctx().clone(),
        ui.id().with("stacked-measure"),
        egui::UiBuilder::new().sizing_pass().invisible().max_rect(at),
    );
    probe.spacing_mut().item_spacing.y = 0.0;
    contents(&mut probe);
    let height = probe.min_rect().height();

    let top = at.center().y - height / 2.0;
    let mut column = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(Rect::from_min_size(egui::pos2(at.left(), top), vec2(at.width(), height)))
            .layout(Layout::top_down(Align::Min)),
    );
    column.spacing_mut().item_spacing.y = 0.0;
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
    /// Something went as it should. Louder than neutral and quieter than a
    /// warning: the design says so in the ink rather than in a colour, because
    /// nothing here needs attention.
    Ok,
    Warn,
    Err,
}

impl Tone {
    pub fn colour(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.ink_2,
            Tone::Ok => palette.ok,
            Tone::Warn => palette.accent_text,
            Tone::Err => palette.err_text,
        }
    }

    /// The supporting line under a toned headline, which the design tints with
    /// it rather than leaving grey: a warm grey under the accent, a red-grey
    /// under an error.
    fn supporting(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral | Tone::Ok => palette.ink_2,
            Tone::Warn => palette.ink_2_warm,
            Tone::Err => palette.ink_2_err,
        }
    }

    fn wash(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.info_bg,
            Tone::Ok => palette.ok_bg,
            Tone::Warn => palette.warn_bg,
            Tone::Err => palette.err_bg,
        }
    }
}

/// A banner across the working area: something is in the way, and here is what.
///
/// Two lines, because one is never enough for a condition the user has to act
/// on: what is true, and what it means for the thing they were about to do.
/// Where something can be done about it, the one thing sits at the right end.
pub struct Banner<'a> {
    pub tone: Tone,
    pub title: &'a str,
    pub body: &'a str,
    pub action: Option<&'a str>,
    /// Whether it can be put away. A condition cannot: it goes when it stops
    /// being true. What already happened can, and has to be, because nothing
    /// else will stop being true to take it off the screen.
    pub dismissible: bool,
}

/// What was pressed on a banner, if anything was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answered {
    Nothing,
    Action,
    Dismissed,
}

/// Draw a banner, and answer what was pressed on it.
pub fn banner(ui: &mut Ui, palette: Palette, banner: &Banner<'_>) -> Answered {
    let mut pressed = Answered::Nothing;
    Frame::new().fill(banner.tone.wash(palette)).inner_margin(Margin::same(metric::PAD as i8)).show(
        ui,
        |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                dot(ui, banner.tone.colour(palette));
                ui.add_space(metric::GAP);
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = BETWEEN_THE_LINES;
                    ui.label(
                        RichText::new(banner.title)
                            .font(font::emphasis(ui.ctx(), font::CONTROL))
                            .color(banner.tone.colour(palette)),
                    );
                    ui.label(
                        RichText::new(banner.body)
                            .font(font::plain(font::NOTE))
                            .color(banner.tone.supporting(palette)),
                    );
                });
                if banner.action.is_some() || banner.dismissible {
                    ui.with_layout(Layout::right_to_left(Align::Min), |ui| {
                        if banner.dismissible {
                            if dismiss(ui, palette).clicked() {
                                pressed = Answered::Dismissed;
                            }
                            // Only between the two of them. Laid out from the
                            // right, a gap with nothing after it is a gap that
                            // moves whatever is before it off the edge.
                            ui.add_space(metric::TOOL_GAP);
                        }
                        if let Some(action) = banner.action
                            && outlined_button(ui, banner.tone.colour(palette), action).clicked()
                        {
                            pressed = Answered::Action;
                        }
                    });
                }
            });
        },
    );
    pressed
}

/// A banner's mark: the tone as a shape, so the tone is not carried by colour
/// alone.
fn dot(ui: &mut Ui, colour: Color32) {
    let (rect, _) = ui.allocate_exact_size(vec2(metric::DOT, DOT_BASELINE), Sense::hover());
    ui.painter().circle_filled(
        egui::pos2(rect.center().x, rect.bottom() - metric::DOT / 2.0),
        metric::DOT / 2.0,
        colour,
    );
}

/// How far down the dot sits, which is the middle of the headline beside it
/// rather than the top of the block.
const DOT_BASELINE: f32 = 11.0;
/// Between a banner's headline and the line explaining it.
const BETWEEN_THE_LINES: f32 = 4.0;

/// Put a banner away. A square control at its right end, as the design has it.
fn dismiss(ui: &mut Ui, palette: Palette) -> Response {
    let button = egui::Button::new(
        RichText::new(icon::DISMISS).font(font::icon(ui.ctx(), font::ICON)).color(palette.ink_3),
    )
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::RADIUS))
    .min_size(vec2(metric::TAB, metric::TAB));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::Vec2::ZERO;
        filled_button(ui, Color32::TRANSPARENT, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// A control that carries a tone: an outline and its text in one colour, over
/// the wash it sits on rather than over a fill of its own.
fn outlined_button(ui: &mut Ui, colour: Color32, label: &str) -> Response {
    let button = egui::Button::new(
        RichText::new(label).font(font::emphasis(ui.ctx(), font::CONTROL)).color(colour),
    )
    .fill(Color32::TRANSPARENT)
    .stroke(Stroke::new(metric::HAIRLINE, colour))
    .corner_radius(CornerRadius::same(metric::RADIUS))
    .min_size(vec2(0.0, metric::OUTLINED));
    ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand)
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
/// How wide each block of prose is allowed to run before it wraps, narrowing as
/// it gets quieter. Prose set the full width of a window is prose nobody
/// finishes, and the design gives each of the three its own measure.
const BODY_WIDTH: f32 = 430.0;
const ASIDE_WIDTH: f32 = 410.0;
const FOOT_WIDTH: f32 = 400.0;

/// The gaps down an empty state, which the design states one at a time rather
/// than repeating one value. They are not the same: what follows the headline
/// belongs to it, and what follows the body is a separate thought.
mod stack {
    pub const AFTER_ICON: f32 = 16.0;
    pub const AFTER_TITLE: f32 = 9.0;
    pub const AFTER_BODY: f32 = 15.0;
    pub const BEFORE_ASIDE: f32 = 16.0;
    pub const BEFORE_CONTROLS: f32 = 18.0;
    pub const BEFORE_FOOT: f32 = 18.0;
}

pub fn empty_state(ui: &mut Ui, palette: Palette, empty: &Empty<'_>) -> Pressed {
    let region = ui.available_rect_before_wrap();
    if !empty.minor {
        corner_marks(ui, palette, region);
    }
    // Written to from both passes. The measuring one is a sizing pass, where
    // nothing is interacted with, so what it writes is always `Nothing`.
    let pressed = std::cell::Cell::new(Pressed::Nothing);
    centred_vertically(ui, region, |ui| pressed.set(block(ui, palette, empty)));
    pressed.get()
}

/// Draw a block in the middle of a region, having measured it first.
///
/// The design centres these; immediate mode cannot know how tall one is until
/// it has drawn it, so it is drawn twice: once in a sizing pass that produces
/// no geometry, to be measured, and then for real with the leftover halved
/// above it. A fraction of the region guessed instead - which is what both of
/// these once did - puts the block wherever the region happens to be tall.
fn centred_vertically(ui: &mut Ui, region: Rect, contents: impl Fn(&mut Ui)) {
    let mut probe = Ui::new(
        ui.ctx().clone(),
        ui.id().with("centre-measure"),
        egui::UiBuilder::new().sizing_pass().invisible().max_rect(region),
    );
    contents(&mut probe);
    let measured = probe.min_rect().height();
    ui.add_space(((region.height() - measured) / 2.0).max(0.0));
    contents(ui);
}

fn block(ui: &mut Ui, palette: Palette, empty: &Empty<'_>) -> Pressed {
    let mut pressed = Pressed::Nothing;
    let title_size = if empty.minor { font::SUBHEADING } else { font::HEADING };
    let icon_size = if empty.minor { font::ICON_SMALL } else { font::ICON_LARGE };
    let icon_ink = if empty.inviting { palette.accent } else { palette.ink_3 };

    ui.vertical_centered(|ui| {
        // Stated gap by gap below, so egui's own spacing does not land between
        // the lines on top of the design's.
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.label(RichText::new(empty.icon).font(font::icon(ui.ctx(), icon_size)).color(icon_ink));
        ui.add_space(stack::AFTER_ICON);
        ui.label(
            RichText::new(empty.title)
                .font(font::emphasis(ui.ctx(), title_size))
                .color(palette.ink),
        );
        ui.add_space(stack::AFTER_TITLE);
        ui.allocate_ui_with_layout(
            vec2(BODY_WIDTH, 0.0),
            Layout::top_down(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.label(
                    RichText::new(empty.body).font(font::plain(font::CONTROL)).color(palette.ink_2),
                );
                if empty.extensions {
                    ui.add_space(stack::AFTER_BODY);
                    extensions(ui, palette);
                }
                if let Some(aside) = empty.aside {
                    ui.add_space(stack::BEFORE_ASIDE);
                    ui.allocate_ui_with_layout(
                        vec2(ASIDE_WIDTH, 0.0),
                        Layout::top_down(Align::Center),
                        |ui| {
                            ui.label(
                                RichText::new(aside)
                                    .font(font::plain(font::CHIP))
                                    .color(palette.ink_3),
                            );
                        },
                    );
                }
            },
        );

        if empty.action.is_some() || empty.alt.is_some() {
            ui.add_space(stack::BEFORE_CONTROLS);
            ui.horizontal(|ui| {
                // The design's gap. The style's own is `TIGHT`, so the pair
                // drew six apart while the block was centred as though it were
                // eight - both numbers wrong until the other was.
                ui.spacing_mut().item_spacing.x = metric::TOOL_GAP;
                // Centred as a block. A row laid out left to right inside a
                // centred column still starts at the left edge of it, so the
                // width has to be known before anything is drawn.
                let width = empty_controls_width(
                    ui,
                    empty.action,
                    empty.action_is_primary,
                    empty.alt,
                );
                ui.add_space((ui.available_width() - width) / 2.0);
                if let Some(action) = empty.action {
                    let hit = if empty.action_is_primary {
                        primary_button(ui, palette, action, "", true, "").clicked()
                    } else {
                        empty_action(ui, palette, action).clicked()
                    };
                    if hit {
                        pressed = Pressed::Action;
                    }
                }
                if let Some(alt) = empty.alt
                    && empty_alt(ui, palette, alt).clicked()
                {
                    pressed = Pressed::Alt;
                }
            });
        }
        if let Some(foot) = empty.foot {
            ui.add_space(stack::BEFORE_FOOT);
            ui.allocate_ui_with_layout(
                vec2(FOOT_WIDTH, 0.0),
                Layout::top_down(Align::Center),
                |ui| {
                    ui.label(
                        RichText::new(foot).font(font::plain(font::NOTE)).color(palette.ink_3),
                    );
                },
            );
        }
    });
    pressed
}

/// The extensions a drop accepts, as the design sets them: separated by a mark
/// rather than by whitespace, so the three read as a list rather than as one
/// line of three words.
fn extensions(ui: &mut Ui, palette: Palette) {
    let mut line = egui::text::LayoutJob::default();
    let mono = || egui::TextFormat {
        font_id: font::mono(font::MONO),
        color: palette.ink_3,
        ..Default::default()
    };
    // The one list, from the module that decides what a drop takes. A second
    // copy of it here is a second thing to remember when a fourth extension
    // arrives.
    for (at, extension) in crate::staging::ACCEPTED.iter().enumerate() {
        let gap = if at > 0 { EITHER_SIDE_OF_A_SEPARATOR } else { 0.0 };
        if at > 0 {
            line.append(SEPARATOR, gap, mono());
        }
        line.append(&format!(".{extension}"), gap, mono());
    }
    ui.label(line);
}

const EITHER_SIDE_OF_A_SEPARATOR: f32 = 10.0;

/// The mark the design separates items of one line with.
///
/// Escaped rather than typed: the house rule keeps the source ASCII, and both
/// faces here carry this codepoint - which was checked, because a glyph the
/// font does not have is drawn as a box in the one place a user is reading.
pub const SEPARATOR: &str = "\u{b7}";

/// The four corner marks the design puts around a full-region empty state.
fn corner_marks(ui: &Ui, palette: Palette, region: Rect) {
    let stroke = Stroke::new(metric::HAIRLINE, palette.line);
    corner_marks_of(ui.painter(), stroke, region.shrink(MARK_INSET), MARK_ARM);
}

/// The marks themselves, given where their corners are. Shared with the drop
/// target, which draws the same figure in the accent and further in.
fn corner_marks_of(painter: &egui::Painter, stroke: Stroke, inner: Rect, arm: f32) {
    for (x, dx) in [(inner.left(), 1.0), (inner.right(), -1.0)] {
        for (y, dy) in [(inner.top(), 1.0f32), (inner.bottom(), -1.0)] {
            let corner = egui::pos2(x, y);
            painter.line_segment([corner, egui::pos2(x + dx * arm, y)], stroke);
            painter.line_segment([corner, egui::pos2(x, y + dy * arm)], stroke);
        }
    }
}

/// One step of a preparation, as the dialog lists it.
pub struct StepLine<'a> {
    pub label: &'a str,
    pub state: crate::work::State,
}

/// What a preparation looks like while it is happening.
///
/// The design draws this as a dialog over the window rather than as a screen of
/// its own: the list is still there, the work is not somewhere else, and what
/// is happening is on top of it and holds everything else still.
pub struct Progress<'a> {
    pub title: &'a str,
    /// Which step of how many, and its name.
    pub step: &'a str,
    pub steps: &'a [StepLine<'a>],
    /// What the user is entitled to know while it runs.
    pub note: &'a str,
    /// How far through, from nothing to one.
    pub through: f32,
}

/// How wide the dialog is, and how its corners are cut. The design gives this
/// one a softer corner than a control: it is a surface, not a button.
const DIALOG_WIDTH: f32 = 436.0;
const DIALOG_RADIUS: u8 = 8;
/// The dialog's own padding, which is not the window's: 14 across, and a little
/// less under a heading than over it.
const DIALOG_PAD: f32 = 14.0;
const UNDER_A_HEADING: f32 = 12.0;
const ABOVE_A_FOOT: f32 = 11.0;
/// One step's row, and the gaps along it.
const STEP_ROW: f32 = 27.0;
const ALONG_A_STEP: f32 = 10.0;
const STEP_DOT: f32 = 5.0;
/// Between the heading of the dialog and the line under it.
const UNDER_A_TITLE: f32 = 2.0;
/// The bar across the foot, which is a line rather than a trough.
const PROGRESS_BAR: f32 = 2.0;

pub fn progress_dialog(ui: &mut Ui, palette: Palette, progress: &Progress<'_>) {
    // Over everything, bars included: the window is holding still, and a scrim
    // that stopped at the working area would say that the bars are not.
    let window = ui.ctx().viewport_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("progress-dialog"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(window, CornerRadius::ZERO, palette.scrim);

    // The scrim takes the pointer as well as the light. Everything behind it is
    // still drawn - the work is being done to that list - but a control under a
    // scrim that still answered would be a window saying one thing and doing
    // another. Hit testing goes by layer, so a rect on this one absorbs what
    // would otherwise reach the bars.
    let mut sink = Ui::new(
        ui.ctx().clone(),
        egui::Id::new("progress-dialog-scrim"),
        egui::UiBuilder::new().layer_id(layer).max_rect(window),
    );
    sink.allocate_rect(window, Sense::click_and_drag());

    let contents = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        let rounded = |top: bool| CornerRadius {
            nw: if top { DIALOG_RADIUS } else { 0 },
            ne: if top { DIALOG_RADIUS } else { 0 },
            sw: if top { 0 } else { DIALOG_RADIUS },
            se: if top { 0 } else { DIALOG_RADIUS },
        };

        Frame::new()
            .fill(palette.bg)
            .corner_radius(rounded(true))
            .inner_margin(Margin {
                left: DIALOG_PAD as i8,
                right: DIALOG_PAD as i8,
                top: DIALOG_PAD as i8,
                bottom: UNDER_A_HEADING as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(progress.title)
                        .font(font::emphasis(ui.ctx(), font::DIALOG_TITLE))
                        .color(palette.ink),
                );
                ui.add_space(UNDER_A_TITLE);
                ui.label(
                    RichText::new(progress.step).font(font::plain(font::NOTE)).color(palette.ink_3),
                );
            });

        Frame::new()
            .inner_margin(Margin {
                left: DIALOG_PAD as i8,
                right: DIALOG_PAD as i8,
                top: UNDER_A_HEADING as i8,
                bottom: DIALOG_PAD as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                for (at, step) in progress.steps.iter().enumerate() {
                    step_row(ui, palette, at, step);
                }
            });

        Frame::new()
            .inner_margin(Margin {
                left: DIALOG_PAD as i8,
                right: DIALOG_PAD as i8,
                top: 0,
                bottom: ABOVE_A_FOOT as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    RichText::new(progress.note).font(font::plain(font::NOTE)).color(palette.ink_3),
                );
            });

        Frame::new()
            .fill(palette.bg)
            .corner_radius(rounded(false))
            .inner_margin(Margin::symmetric(DIALOG_PAD as i8, UNDER_A_HEADING as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let percent = format!("{}%", (progress.through * 100.0).round());
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            RichText::new(percent)
                                .font(font::mono(font::MONO_TIGHT))
                                .color(palette.ink_3),
                        );
                        ui.add_space(ALONG_A_STEP);
                        let (bar, _) = ui.allocate_exact_size(
                            vec2(ui.available_width(), PROGRESS_BAR),
                            Sense::hover(),
                        );
                        ui.painter().rect_filled(bar, CornerRadius::ZERO, palette.line);
                        let mut through = bar;
                        through.set_right(bar.left() + bar.width() * progress.through);
                        ui.painter().rect_filled(through, CornerRadius::ZERO, palette.accent);
                    });
                });
            });
    };

    // As tall as its contents, and centred in the window as a whole rather than
    // in the working area: it is about the window, and the bars are behind the
    // same scrim.
    let mut probe = Ui::new(
        ui.ctx().clone(),
        egui::Id::new("progress-dialog-measure"),
        egui::UiBuilder::new().sizing_pass().invisible().max_rect(Rect::from_min_size(
            window.min,
            vec2(DIALOG_WIDTH, window.height()),
        )),
    );
    contents(&mut probe);

    let rect =
        Rect::from_center_size(window.center(), vec2(DIALOG_WIDTH, probe.min_rect().height()));
    painter.rect_filled(rect, CornerRadius::same(DIALOG_RADIUS), palette.panel_2);
    let mut dialog = Ui::new(
        ui.ctx().clone(),
        egui::Id::new("progress-dialog-contents"),
        egui::UiBuilder::new().layer_id(layer).max_rect(rect),
    );
    contents(&mut dialog);
}

/// One step of a preparation, as a row of the progress list.
///
/// Every step is drawn whether or not this plan runs it. A step that vanished
/// would change the count under a reader who is watching it move, and "not run"
/// is a thing they need to be able to see afterwards - which is also why a
/// skipped step keeps its place and loses only its number.
fn step_row(ui: &mut Ui, palette: Palette, at: usize, step: &StepLine<'_>) {
    use crate::work::State;
    // The design says each state four times over - the number, the mark, the
    // label and the word at the end - so they are decided in one place.
    let (mark, number_ink, label_ink, meta) = match step.state {
        State::Waiting => (None, palette.ink_3, palette.ink_3, ""),
        State::NotRun => (None, palette.ink_3, palette.ink_3, "not run"),
        State::Running => (Some(palette.accent), palette.accent_text, palette.ink, ""),
        State::Done => (Some(palette.ink_2), palette.ink_3, palette.ink_2, "done"),
        State::Failed => (Some(palette.err), palette.err_text, palette.err_text, "failed"),
    };
    let number =
        if step.state == State::NotRun { "-".to_owned() } else { (at + 1).to_string() };
    let emphasis = step.state == State::Running;

    let (rect, _) = ui.allocate_exact_size(vec2(ui.available_width(), STEP_ROW), Sense::hover());
    let mut line = ui.new_child(
        egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)),
    );
    line.spacing_mut().item_spacing.x = 0.0;
    line.label(RichText::new(number).font(font::mono(font::MONO_TIGHT)).color(number_ink));
    line.add_space(ALONG_A_STEP);

    // The mark is a shape and not a character, so a step that has not been
    // reached leaves a hole the same size rather than shifting its label.
    let (dot, _) = line.allocate_exact_size(vec2(STEP_DOT, STEP_DOT), Sense::hover());
    if let Some(colour) = mark {
        line.painter().circle_filled(dot.center(), STEP_DOT / 2.0, colour);
    }
    line.add_space(ALONG_A_STEP);

    let label = RichText::new(step.label)
        .font(if emphasis {
            font::emphasis(line.ctx(), font::CONTROL)
        } else {
            font::plain(font::CONTROL)
        })
        .color(label_ink);
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if !meta.is_empty() {
            ui.label(RichText::new(meta).font(font::mono(font::MONO_TIGHT)).color(palette.ink_3));
        }
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add(egui::Label::new(label).truncate());
        });
    });
}

/// How wide the list of files being dropped is. Fixed, because it is what
/// centres the block; a name longer than this is truncated rather than allowed
/// to move the whole listing sideways.
const LISTING_WIDTH: f32 = 318.0;
/// One line of that list, and the space between two of them.
const LISTING_ROW: f32 = 27.0;
const BETWEEN_LISTING_ROWS: f32 = 1.0;
/// From the edge of a listing row to its contents, which is tighter than a
/// window's own padding.
const LISTING_PAD: f32 = 10.0;
/// Between a file's number and its name.
const BESIDE_A_NUMBER: f32 = 9.0;
/// Between the three parts of the overlay: the icon, the heading, the listing
/// and the line naming the extensions.
const BETWEEN_OVERLAY_PARTS: f32 = 16.0;
/// How far the drop target's marks sit in from the window's edge, and how long
/// their arms are. Further in than the border they sit inside.
const DROP_MARK_INSET: f32 = 20.0;
const DROP_MARK_ARM: f32 = 12.0;
/// The dash the design draws the drop border with.
const DASH: f32 = 5.0;

/// One thing under the pointer, and what a drop would do with it.
pub struct Hovering {
    pub name: String,
    /// What is on the right of the row: how many documents a folder holds, or
    /// that this one will be passed over. Empty for an ordinary document, which
    /// needs no comment.
    pub note: String,
    /// Stated per file before the drop rather than after it. From the name
    /// alone, because that is all there is to go on while the file still
    /// belongs to the operating system.
    pub accepted: bool,
}

/// The whole window as a drop target, while something is over it.
///
/// Painted on the foreground layer rather than composed into a panel, because
/// the design covers everything - both bars included - and a drop is not aimed
/// at any particular region of the window.
pub fn drop_target(ui: &mut Ui, palette: Palette, heading: &str, files: &[Hovering]) {
    // The viewport rather than the content area: the whole window is the
    // target, bars included, and the scrim has to reach the edges of what the
    // user is dragging over.
    let window = ui.ctx().viewport_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new("drop-target"));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(window, CornerRadius::ZERO, palette.scrim);
    let border = window.shrink(metric::GAP);
    painter.rect_filled(border, CornerRadius::ZERO, palette.accent_soft);
    let stroke = Stroke::new(metric::HAIRLINE, palette.accent);
    // Dashed, as the design draws it: a solid outline reads as an edge of the
    // window, and a target is a place something is about to land.
    for (from, to) in
        [(border.left_top(), border.right_top()), (border.left_bottom(), border.right_bottom())]
    {
        painter.add(egui::Shape::dashed_line(&[from, to], stroke, DASH, DASH));
    }
    for (from, to) in
        [(border.left_top(), border.left_bottom()), (border.right_top(), border.right_bottom())]
    {
        painter.add(egui::Shape::dashed_line(&[from, to], stroke, DASH, DASH));
    }
    corner_marks_of(&painter, stroke, window.shrink(DROP_MARK_INSET), DROP_MARK_ARM);

    let mut overlay = Ui::new(
        ui.ctx().clone(),
        egui::Id::new("drop-target-contents"),
        egui::UiBuilder::new().layer_id(layer).max_rect(window),
    );
    // Centred in the window, measured rather than guessed: the block is as tall
    // as the number of files being dragged makes it.
    centred_vertically(&mut overlay, window, |ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        ui.vertical_centered(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                RichText::new(icon::DROP)
                    .font(font::icon(ui.ctx(), font::ICON_LARGE))
                    .color(palette.accent),
            );
            ui.add_space(BETWEEN_OVERLAY_PARTS);
            ui.label(
                RichText::new(heading)
                    .font(font::emphasis(ui.ctx(), font::HEADING))
                    .color(palette.ink),
            );
            // What is being dropped, by name. A count alone cannot be checked
            // against what the pointer is carrying, and a drop is a decision
            // made before it lands.
            if !files.is_empty() {
                ui.add_space(BETWEEN_OVERLAY_PARTS);
                listing(ui, palette, files);
            }
            ui.add_space(BETWEEN_OVERLAY_PARTS);
            extensions(ui, palette);
        });
    });
}

/// The files under the pointer, one to a line.
fn listing(ui: &mut Ui, palette: Palette, files: &[Hovering]) {
    let mut accepted = 0;
    for file in files {
        // A fixed width, so the block is centred as a block and the names line
        // up under one another. A row laid out left to right would take the
        // full width and start at the edge instead.
        let (rect, _) = ui.allocate_exact_size(vec2(LISTING_WIDTH, LISTING_ROW), Sense::hover());
        if file.accepted {
            accepted += 1;
            ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.panel);
        }
        let number =
            if file.accepted { accepted.to_string() } else { REFUSED_NUMBER.to_owned() };
        let ink = if file.accepted { palette.ink } else { palette.ink_3 };

        let mut line = ui.new_child(
            egui::UiBuilder::new()
                .max_rect(rect.shrink2(vec2(LISTING_PAD, 0.0)))
                .layout(Layout::left_to_right(Align::Center)),
        );
        line.spacing_mut().item_spacing.x = 0.0;
        line.label(
            RichText::new(number)
                .font(font::mono(font::MONO_TIGHT))
                .color(if file.accepted { palette.accent_text } else { palette.ink_3 }),
        );
        line.add_space(BESIDE_A_NUMBER);
        let mut name = RichText::new(&file.name).font(font::mono(font::MONO)).color(ink);
        if !file.accepted {
            name = name.strikethrough();
        }
        line.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !file.note.is_empty() {
                ui.label(
                    RichText::new(&file.note).font(font::plain(font::NOTE)).color(palette.ink_3),
                );
            }
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                ui.add(egui::Label::new(name).truncate());
            });
        });
        ui.add_space(BETWEEN_LISTING_ROWS);
    }
}

/// What stands where a number would, against a file a drop will pass over.
const REFUSED_NUMBER: &str = "--";

#[cfg(test)]
mod tests {
    use super::*;

    /// A row the width of the window the design is drawn at.
    fn a_row(height: f32) -> Rect {
        Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(metric::WINDOW[0], height))
    }

    fn edges(cell: Rect) -> (f32, f32) {
        (cell.left(), cell.right())
    }

    /// The column boundaries the bundle's own `EntryRow` produces at 820 wide,
    /// read out of it rather than derived here: `66px minmax(0,1fr) 106px 130px
    /// 84px`, twelve apart, twelve in from each edge.
    ///
    /// Worth a test because this is the fault that survived a whole session of
    /// looking at the screen. The identity column sat 96 pixels right of the
    /// design's, and nothing about the picture said so.
    #[test]
    fn an_entry_row_is_divided_as_the_bundle_divides_it() {
        let columns = Columns::across(a_row(metric::ROW));
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 452.0));
        assert_eq!(edges(columns.uuid), (464.0, 570.0));
        assert_eq!(edges(columns.status), (582.0, 712.0));
        assert_eq!(edges(columns.actions), (724.0, 808.0));
    }

    /// The same, for `CatalogRow`: `66px minmax(0,1fr) 116px 56px 142px 92px`.
    #[test]
    fn a_catalog_row_is_divided_as_the_bundle_divides_it() {
        let columns = CatalogColumns::across(a_row(metric::CATALOG_ROW));
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 354.0));
        assert_eq!(edges(columns.author), (366.0, 482.0));
        assert_eq!(edges(columns.version), (494.0, 550.0));
        assert_eq!(edges(columns.status), (562.0, 704.0));
        assert_eq!(edges(columns.actions), (716.0, 808.0));
    }

    /// The empty state's pair, against the bundle's `EmptyState` rendered at
    /// its own preview size: both controls 32 tall, 8 apart, the action padded
    /// 14 and the alternative 12.
    ///
    /// The heights are asserted as a relationship rather than as a number,
    /// because the rule the design is expressing is that the pair matches the
    /// primary beside it - which is what borrowing the toolbar's 26-tall button
    /// broke, and what a reader of two equal constants would not see.
    #[test]
    fn an_empty_state_offers_its_pair_at_one_height() {
        assert_eq!(
            metric::EMPTY_CONTROL,
            metric::ACTION,
            "both controls stand at the primary's height"
        );
        assert_ne!(
            metric::EMPTY_CONTROL,
            metric::CONTROL,
            "and not at a bar control's, which is what they used to borrow"
        );
        assert_eq!(metric::EMPTY_ACTION_PAD, 14.0);
        assert_eq!(metric::EMPTY_ALT_PAD, 12.0);
        // The gap the design states, which is the toolbar's rather than the
        // style's default `TIGHT`.
        assert_eq!(metric::TOOL_GAP, 8.0);
    }

    /// The overflow menu, against the bundle with the menu forced open and
    /// `getBoundingClientRect` asked for every box: the menu 200 by 131 with
    /// its right edge 8 from the window, four items of 190 by 28 touching, and
    /// a rule 186 wide 93 below the menu's top.
    ///
    /// The design states `min-width:190px` on a box that is not `border-box`,
    /// so 190 is what the items are laid out at and 200 is what the menu draws
    /// across. Both are asserted, because reading either one as the other is
    /// how ten pixels go missing.
    #[test]
    fn the_overflow_menu_is_measured_as_the_bundle_measures_it() {
        let border = 2.0 * metric::HAIRLINE;
        let padding = 2.0 * f32::from(metric::MENU_MARGIN);
        assert_eq!(metric::MENU + padding + border, 200.0, "the menu's drawn width");

        // Four items, with the rule and its air between the third and fourth.
        let rule = 2.0 * metric::MENU_RULE_GAP + metric::HAIRLINE;
        assert_eq!(
            border + padding + 4.0 * metric::MENU_ITEM + rule,
            131.0,
            "the menu's drawn height"
        );
        assert_eq!(
            metric::HAIRLINE + f32::from(metric::MENU_MARGIN) + 3.0 * metric::MENU_ITEM
                + metric::MENU_RULE_GAP,
            93.0,
            "how far the rule sits below the menu's top"
        );
        assert_eq!(metric::MENU - 2.0 * metric::MENU_RULE_INSET, 186.0, "the rule's width");

        // Inside one item: eight of padding, a sixteen-pixel icon, then nine.
        // The bundle puts the label 33 from the item's left edge.
        assert_eq!(metric::MENU_PAD_X + font::ICON + metric::MENU_GAP, 33.0);
        assert_eq!(metric::MENU_ITEM - 2.0 * metric::MENU_PAD_Y, font::ICON);

        // The menu hangs off the bar, not off the control: the control clears
        // the window's right edge by the bar's own padding and the menu by
        // eight, so the menu overhangs the control by the difference.
        assert_eq!(metric::PAD - metric::MENU_OVERHANG, 8.0);
    }
}
