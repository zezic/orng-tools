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
    self, Align, Color32, CornerRadius, Frame, Layout, Margin, Rect, Response, Sense,
    Stroke, Ui, vec2,
};
use orng_tools::{Kind, Placement, TheDocument};

use crate::status::{Action, Consequences, Offer, Published, Status};
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

/// The frame behind a full-window screen.
///
/// The background colour rather than the list's row colour, because a screen is
/// not a list: the design draws Settings, Restore and About on `--bg` and gives
/// each a header of its own, which is the same statement it makes by separating
/// the bars from the working area by colour. No margin, because the screen's own
/// body states the padding the design gives it.
pub fn screen(palette: Palette) -> Frame {
    Frame::new().fill(palette.bg).inner_margin(Margin::ZERO)
}

/// One of the two top-level views.
///
/// Drawn as a chip that fills when it is the current view, which is what the
/// design does. Not an underline and not a radio button: this is a switch
/// between two activities, and it has to read as one of them being held.
pub fn view_tab(ui: &mut Ui, palette: Palette, label: &str, current: bool) -> Response {
    let text = font::run(
        label,
        if current {
            font::emphasis(ui.ctx(), font::CONTROL)
        } else {
            font::plain(font::CONTROL)
        },
    )
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
pub fn filter_chip(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    count: usize,
    on: bool,
    width: Width,
) -> Response {
    let ink = if on { palette.ink } else { palette.ink_3 };
    let mut text = egui::text::LayoutJob::default();
    text.append(
        label,
        0.0,
        egui::TextFormat { color: ink, ..font::format(font::plain(font::CHIP)) },
    );
    text.append(
        &count.to_string(),
        metric::TIGHT,
        egui::TextFormat { color: palette.ink_3, ..font::format(font::mono(font::MONO_TIGHT)) },
    );

    let button = egui::Button::new(text)
        .fill(if on { palette.btn } else { Color32::TRANSPARENT })
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(vec2(0.0, metric::CONTROL));
    ui.scope(|ui| {
        // The design tightens a chip beside the inspector rather than letting
        // the three of them push the field it shares the toolbar with below
        // what a search field is worth having.
        ui.spacing_mut().button_padding.x = match width {
            Width::Full => metric::CHIP_PAD,
            Width::Narrow => metric::NARROW_CHIP_PAD,
        };
        ui.add(button).on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
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
            color: icon_ink,
            valign: Align::Center,
            ..font::format(font::icon(ui.ctx(), font::ICON))
        },
    );
    if !label.is_empty() {
        job.append(
            label,
            gap,
            egui::TextFormat {
                color: ink,
                valign: Align::Center,
                ..font::format(font::plain(size))
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

/// How old the catalog index is, stated beside the view switch.
///
/// Catalog view only, and the first of the two things the design puts there -
/// `InstallBar.dc.html:36`. Quiet grey while the index is current and the
/// accent once it is stale, which is `:93`: at that point the age stops being a
/// fact about the window and becomes the reason the control beside it has grown
/// a word.
pub fn freshness(ui: &mut Ui, palette: Palette, stated: &str, stale: bool) {
    let ink = if stale { palette.accent_text } else { palette.ink_3 };
    ui.label(font::run(stated, font::plain(font::CHIP)).color(ink));
}

/// The second: go and ask the catalog again.
///
/// **Two shapes and one control** - `InstallBar.dc.html:94-97`. A bare glyph in
/// `ink_3` on no ground while the index is current, and a filled button
/// carrying the word once it is stale. 22 tall either way, which makes it the
/// shortest control in any bar and two under the tabs it sits beside.
///
/// The label is stated rather than left to the text, for the reason
/// [`row_action`] states its own: the current shape is a glyph and nothing
/// else, and a button built from one is announced as a private-use codepoint.
/// `Refresh catalog` is the bundle's own `title` at `:37`.
pub fn refresh(ui: &mut Ui, palette: Palette, stale: bool) -> Response {
    let (text, fill, min, pad) = if stale {
        (
            with_icon(ui, icon::REFRESH, "Refresh", font::CHIP, palette.ink, palette.ink),
            palette.btn,
            vec2(0.0, metric::REFRESH),
            metric::REFRESH_PAD_X,
        )
    } else {
        // Square, and the padding is zeroed to let it be one: the glyph is 16
        // across in a box the design states at 22, and `min_size` is a floor
        // that cannot bring a wider box back down.
        (
            egui::WidgetText::from(
                font::run(icon::REFRESH, font::icon(ui.ctx(), font::ICON)).color(palette.ink_3),
            ),
            Color32::TRANSPARENT,
            vec2(metric::REFRESH, metric::REFRESH),
            0.0,
        )
    };
    let button = egui::Button::new(text)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(min);
    let response = ui
        .scope(|ui| {
            ui.spacing_mut().button_padding = vec2(pad, 0.0);
            // The theme's interactive size is a bar control's 26 and this one
            // is 22, and `min_size` alone is a floor - the same correction
            // [`catalog_action`] makes for its 24.
            ui.spacing_mut().interact_size.y = metric::REFRESH;
            filled_button(ui, fill, palette.btn_hover, button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        })
        .inner;
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), REFRESH_CATALOG)
    });
    response.on_hover_text(REFRESH_CATALOG)
}

/// What the refresh control is, wherever it is asked about: the tooltip, the
/// accessibility tree and the test that presses it.
pub const REFRESH_CATALOG: &str = "Refresh catalog";

/// A screen's own version of [`small_button`]: `Copy diagnostics`, `Licences`.
///
/// The same box the bar draws - 26 tall, padded ten, six from icon to word -
/// and two things different, both of which a screen does to every control it
/// carries: rounded by the field's four rather than a control's three, and set
/// in the secondary ink rather than the primary. Measured off `AboutScreen`
/// rendered at its own preview size, where the pair comes out 132 and 89 wide
/// seven apart.
///
/// The same relationship [`group_control`] has to this one, one surface further
/// out: a group's controls are the group's, and a screen's are the screen's.
pub fn screen_button(ui: &mut Ui, palette: Palette, glyph: &str, label: &str) -> Response {
    let button = egui::Button::new(with_icon(
        ui,
        glyph,
        label,
        font::CHIP,
        palette.ink_2,
        palette.ink_2,
    ))
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
    .min_size(vec2(0.0, metric::CONTROL));
    ui.scope(|ui| {
        // Across the box, so the height is `min_size` alone.
        ui.spacing_mut().button_padding = vec2(metric::SCREEN_BUTTON_PAD_X, 0.0);
        filled_button(ui, palette.btn, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
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
    let button = egui::Button::new(font::run(label, font::plain(size)).color(palette.ink_2))
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

/// How wide a run of widgets comes out, before any of it is drawn.
///
/// A pass in a `Ui` that paints nothing and takes no input, so the closure that
/// answers the question is the closure that draws it, and the two cannot come
/// to disagree - which a second function returning "how wide this will be"
/// eventually would. The same trick [`stacked_cell`] uses to place a block
/// whose height is not known until it has been laid out.
///
/// **Laid out left to right rather than in the caller's own layout.** This is
/// asked from a bar, whose `Ui` is still the panel's top-down one at the point
/// the question arises; measuring in that stacks the run into a column and
/// answers with the width of its widest member. Which the toolbar's search
/// field then took as licence to draw full width over the chips beside it.
///
/// The salt keeps two measurements on one line from sharing an id, and with it
/// the interaction memory of whatever is inside them.
pub fn measured(ui: &Ui, salt: impl egui::AsIdSalt, contents: impl FnOnce(&mut Ui)) -> f32 {
    let mut probe = Ui::new(
        ui.ctx().clone(),
        ui.id().with(salt),
        egui::UiBuilder::new()
            .sizing_pass()
            .invisible()
            .max_rect(ui.max_rect())
            .layout(Layout::left_to_right(Align::Center)),
    );
    *probe.spacing_mut() = ui.spacing().clone();
    contents(&mut probe);
    probe.min_rect().width()
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

/// How wide the search field draws, on a toolbar with `fixed` pixels already
/// spoken for.
///
/// The field is the toolbar's flexible thing: `flex:1 1 auto` with a cap and a
/// 64-pixel floor, which across the box is `cap` and [`metric::SEARCH_FLOOR`].
/// The cap is the one number the two toolbars disagree about -
/// [`metric::SEARCH_FIELD`] against [`metric::CATALOG_SEARCH_FIELD`] - because
/// the catalog's placeholder names four fields where Local's names two.
///
/// What is easy to miss is the second flexible item - a bare `flex:1 1 0`
/// between the kind chips and the controls at the right end - because the two
/// grow and shrink *together*. So the field gets its own content width plus half
/// of whatever the toolbar has left over, and the gap gets the other half.
///
/// Which is why the bundle draws the Local field 216 across at 820 and 146 at
/// 548, rather than the 207 that "the field takes what is left" would give.
///
/// **Until the gap reaches its own floor**, which is where the sharing stops:
/// the bundle writes `min-width:8px` on it, and a flex item at its minimum
/// stops shrinking while the other one carries on. That is the catalog toolbar
/// beside the panel, where the gap is pinned at eight and the field takes the
/// whole of the remainder - 82 rather than the 84 an even split would give.
///
/// `content` is measured from the hint rather than from what has been typed,
/// which the design does not do: its field is sized by whichever text is in it.
/// A field that widened under the pointer as somebody typed into it would push
/// every chip beside it along, and nothing is worth that.
pub fn search_width(available: f32, fixed: f32, content: f32, cap: f32) -> f32 {
    let spare = available - fixed - content;
    let share = content + spare / 2.0;
    let most = available - fixed - metric::FLEXIBLE_GAP_FLOOR;
    share.min(most).clamp(metric::SEARCH_FLOOR, cap)
}

/// What the search field asks for before anything is shared out: the glyph, the
/// gap, the hint and the padding on both sides.
pub fn search_content(ui: &Ui, hint: &str) -> f32 {
    font::ICON
        + metric::TIGHT
        + text_width(ui, hint, font::plain(font::CONTROL))
        + 2.0 * metric::TOOL_GAP
}

/// The search field: one filled, rounded box holding the glyph and the text.
///
/// The icon is *inside* the field in the bundle, not sitting on the bar beside
/// it. That is the difference between a field with an affordance in it and an
/// icon that happens to be next to a box.
pub fn search_field(ui: &mut Ui, palette: Palette, query: &mut String, hint: &str, width: f32) {
    Frame::new()
        .fill(palette.field)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .inner_margin(Margin::symmetric(metric::TOOL_GAP as i8, 0))
        .show(ui, |ui| {
            ui.set_height(metric::CONTROL);
            ui.horizontal_centered(|ui| {
                ui.label(
                    font::run(icon::SEARCH, font::icon(ui.ctx(), font::ICON))
                        .color(palette.ink_3),
                );
                // Stated, not inherited: a bar zeroes egui's own item spacing so
                // that the design's gaps are the only ones, and this is one of
                // the design's gaps.
                ui.add_space(metric::TIGHT);
                // What is left of the field once the glyph, the gap and the two
                // margins have taken theirs. The width passed in is the whole
                // box, so the box is what matches the bundle rather than the
                // text area inside it.
                let text = width - font::ICON - metric::TIGHT - 2.0 * metric::TOOL_GAP;
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
    primary(ui, palette, false, label, icon, enabled, reason)
}

/// The same press, where pressing it asks the system for administrator rights.
///
/// Windows' own convention: a shield before the words of any control that will
/// raise the consent dialog, so the dialog is not a surprise and nothing of our
/// own has to be put in front of it. Always pressable - a press that cannot
/// ask is refused, and wears no shield.
pub fn elevating_button(ui: &mut Ui, palette: Palette, label: &str, icon: &str) -> Response {
    primary(ui, palette, true, label, icon, true, "")
}

fn primary(
    ui: &mut Ui,
    palette: Palette,
    shield: bool,
    label: &str,
    icon: &str,
    enabled: bool,
    reason: &str,
) -> Response {
    let (fill, ink) =
        if enabled { (palette.accent, palette.accent_ink) } else { (palette.btn, palette.ink_3) };
    let glyph = egui::TextFormat {
        color: ink,
        valign: Align::Center,
        ..font::format(font::icon(ui.ctx(), font::ICON))
    };
    // The label first and the icon after it, as the design has it: the words
    // say what will happen and the arrow says only that something will. The
    // shield goes before both, where Windows puts it.
    let mut text = egui::text::LayoutJob::default();
    if shield {
        text.append(icon::ELEVATES, 0.0, glyph.clone());
    }
    text.append(
        label,
        if shield { metric::TOOL_GAP } else { 0.0 },
        egui::TextFormat {
            color: ink,
            valign: Align::Center,
            ..font::format(font::emphasis(ui.ctx(), font::ACTION))
        },
    );
    if !icon.is_empty() {
        text.append(icon, metric::TOOL_GAP, glyph);
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
        font::run(icon::OVERFLOW, font::icon(ui.ctx(), font::ICON)).color(palette.ink_2),
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
        .shadow(palette.menu_shadow)
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
    /// Go and ask the catalog again. Not [`RESET`]'s single counter-clockwise
    /// arrow, which puts a setting back to what it was: this one is a round
    /// trip, and the design draws it as one.
    pub const REFRESH: &str = light::ARROWS_CLOCKWISE;
    pub const RESTORE: &str = light::CLOCK_COUNTER_CLOCKWISE;
    pub const ABOUT: &str = light::INFO;
    pub const COPY: &str = light::COPY;
    /// Put away something that has already happened.
    pub const DISMISS: &str = light::X;
    /// On the primary action: an arrow when the press leads somewhere, a tick
    /// when it simply does the thing.
    pub const PREPARE: &str = light::ARROW_RIGHT;
    pub const APPLY: &str = light::CHECK;
    /// Before the words of a press that will ask Windows for administrator
    /// rights - see [`super::elevating_button`]. Not the bundle's: it draws no
    /// state for rights at all. Round 3 item 9.
    pub const ELEVATES: &str = light::SHIELD;
    /// The one icon an empty state is built around.
    pub const DROP: &str = light::TRAY_ARROW_DOWN;
    pub const NO_INSTALL: &str = light::FOLDER_DASHED;
    pub const UNREADABLE: &str = light::QUESTION;
    pub const NO_MATCH: &str = light::FUNNEL_X;
    pub const CATALOG: &str = light::PACKAGE;
    /// Show a document where it lives, in the system's own file manager. The
    /// same glyph as `CHANGE_INSTALL` and a different idea, so it is named
    /// again rather than borrowed.
    pub const REVEAL: &str = light::FOLDER_OPEN;
    /// The four remedies a row offers besides revealing. Each is drawn only in
    /// the states that can use it - see [`crate::status`].
    pub const ASSIGN: &str = light::FINGERPRINT;
    /// Go and find a document the entry says should be there and is not. Not
    /// [`REVEAL`]'s folder: nothing is there to open, and the design colours
    /// this one with the accent because it is the remedy rather than a look.
    pub const LOCATE: &str = light::FILE_MAGNIFYING_GLASS;
    /// Take a row out of the removal queue.
    pub const UNDO: &str = light::ARROW_U_UP_LEFT;
    /// Queue an entry for removal, or drop a staged row nothing was written for.
    pub const REMOVE: &str = light::TRASH;
    /// Where a registered document came from: a file the user chose, or the
    /// catalog. The catalog's is `CATALOG`.
    pub const LOCAL_FILE: &str = light::FILE;
    /// Where a registered document actually is.
    pub const LINKED: &str = light::LINK;
    pub const COPIED: &str = light::COPY_SIMPLE;
    pub const UNRESOLVED: &str = light::LINK_BREAK;
    /// Who wrote a published item, which is the trust signal in the catalog.
    pub const AUTHOR: &str = light::USER;
    /// What it is licensed under.
    pub const LICENCE: &str = light::SCALES;
    /// The change that published it, which is where the review of it is.
    pub const PROVENANCE: &str = light::GIT_PULL_REQUEST;
    pub const HOMEPAGE: &str = light::LINK_SIMPLE;
    /// Whether this installation can load it.
    pub const COMPATIBLE: &str = light::CHECK_CIRCLE;
    pub const INCOMPATIBLE: &str = light::WARNING_CIRCLE;
    /// The item that takes a superseded one's place. The same glyph as
    /// `PREPARE` and a different idea, so it is named again.
    pub const REPLACEMENT: &str = light::ARROW_RIGHT;
    /// Out of a full-window screen, back to the view it was opened from.
    pub const BACK: &str = light::ARROW_LEFT;
    /// On to another one. The mark on a row that leads somewhere, which is not
    /// the same idea as the arrow on the primary action.
    pub const ONWARD: &str = light::CARET_RIGHT;
    /// Choose a directory. The same glyph as `CHANGE_INSTALL` and `REVEAL`, and
    /// a third idea: this one is a setting rather than a session or a file.
    pub const BROWSE: &str = light::FOLDER_OPEN;
    /// Put a setting back to whatever was auto-detected.
    pub const RESET: &str = light::ARROW_COUNTER_CLOCKWISE;
    /// One of a set of choices, exactly one of which is in force.
    pub const CHOSEN: &str = light::RADIO_BUTTON;
    pub const UNCHOSEN: &str = light::CIRCLE;
    /// Something that is on or off by itself.
    pub const CHECKED: &str = light::CHECK_SQUARE;
    pub const UNCHECKED: &str = light::SQUARE;
    /// Take the diagnostics report away to put in a bug report. Not `COPY`,
    /// which copies one value: this one copies a page.
    pub const COPY_REPORT: &str = light::CLIPBOARD_TEXT;
    /// A screen with no backups on it yet. Not [`DROP`]: nothing is being
    /// invited here, there is simply nothing kept.
    pub const NO_BACKUP: &str = light::ARCHIVE;
    /// What restoring costs, beside the sentence saying it.
    pub const WARNING: &str = light::WARNING;
    /// This application's own mark, on the About screen. The same glyph as
    /// [`CATALOG`] and a different idea - that one is what the catalog
    /// publishes, this one is the thing drawing the window - so it is named
    /// again rather than borrowed.
    pub const PRODUCT: &str = light::PACKAGE;
    /// Show the backups directory in the system's own file manager. A fourth
    /// idea on the same folder glyph, and the only one that is neither a setting
    /// nor a single file.
    pub const BACKUPS: &str = light::FOLDER_OPEN;
    /// The three appearances, in the order the switch draws them.
    pub const FOLLOW_SYSTEM: &str = light::DESKTOP;
    pub const LIGHT: &str = light::SUN;
    pub const DARK: &str = light::MOON;
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
    line.label(font::run(title, font::emphasis(ui.ctx(), font::CHIP)).color(tone));
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(
            font::run(count.to_string(), font::mono(font::MONO_TIGHT))
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

/// How much of the window the list has.
///
/// The inspector takes 272 of the design's 820 and the list draws in what is
/// left. Not the same row squeezed: the design gives the narrow list a grid of
/// its own, which drops the identity and shortens the two columns at the right
/// end. A row standing beside a panel that is already naming one entry in full
/// does not need to repeat its UUID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Width {
    /// The whole page.
    Full,
    /// The page beside the inspector.
    Narrow,
}

/// The columns of an entry row, as the design's grid has them.
pub struct Columns {
    pub kind: Rect,
    pub name: Rect,
    /// The identity, which only the full grid has room for. `None` is the
    /// narrow row rather than an empty rectangle, so a caller has to decide what
    /// to do about it instead of drawing a UUID into nothing.
    pub uuid: Option<Rect>,
    pub status: Rect,
    /// What the row itself can do. Reserved even while nothing is drawn in it:
    /// the design hides these controls off hover rather than removing them, so
    /// the columns before it do not move when the pointer arrives.
    pub actions: Rect,
}

impl Columns {
    fn across(row: Rect, width: Width) -> Columns {
        match width {
            Width::Full => {
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
                Columns { kind, name, uuid: Some(uuid), status, actions }
            }
            Width::Narrow => {
                let [kind, name, status, actions] = grid(
                    row,
                    [
                        Column::Fixed(metric::KIND_COLUMN),
                        Column::Rest,
                        Column::Fixed(metric::NARROW_STATUS_COLUMN),
                        Column::Fixed(metric::NARROW_ACTIONS_COLUMN),
                    ],
                );
                Columns { kind, name, uuid: None, status, actions }
            }
        }
    }
}

/// The columns of a catalog row. A different grid, because a catalog row
/// answers a different question: not "which of mine is this" but "what is this
/// and who made it", so the identity gives way to the author and the version.
pub struct CatalogColumns {
    pub kind: Rect,
    pub name: Rect,
    /// Both go beside the detail panel, for the reason the identity does: the
    /// panel is already naming the author and the version in full.
    pub author: Option<Rect>,
    pub version: Option<Rect>,
    pub status: Rect,
    pub actions: Rect,
}

impl CatalogColumns {
    fn across(row: Rect, width: Width) -> CatalogColumns {
        match width {
            Width::Full => {
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
                CatalogColumns {
                    kind,
                    name,
                    author: Some(author),
                    version: Some(version),
                    status,
                    actions,
                }
            }
            Width::Narrow => {
                let [kind, name, status, actions] = grid(
                    row,
                    [
                        Column::Fixed(metric::KIND_COLUMN),
                        Column::Rest,
                        Column::Fixed(metric::NARROW_CATALOG_STATUS_COLUMN),
                        Column::Fixed(metric::NARROW_CATALOG_ACTIONS_COLUMN),
                    ],
                );
                CatalogColumns { kind, name, author: None, version: None, status, actions }
            }
        }
    }
}

/// One row of the list: a fixed height, a hover highlight, and its grid.
///
/// `selected` is the row the inspector is open on. The design fills it with the
/// accent at nine per cent and leaves it filled whether or not the pointer is
/// over it, which is what stops the panel from appearing to be about whichever
/// row is under the mouse.
pub fn row(
    ui: &mut Ui,
    palette: Palette,
    width: Width,
    selected: bool,
    contents: impl FnOnce(&mut Ui, &Columns, Controls),
) -> Response {
    let (rect, controls, response) = row_frame(ui, palette, metric::ROW, selected);
    let columns = Columns::across(rect, width);
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    contents(&mut content, &columns, controls);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One row of the catalog, which is taller: the description sits under the name.
pub fn catalog_row(
    ui: &mut Ui,
    palette: Palette,
    width: Width,
    selected: bool,
    contents: impl FnOnce(&mut Ui, &CatalogColumns),
) -> Response {
    let (rect, _, response) = row_frame(ui, palette, metric::CATALOG_ROW, selected);
    let columns = CatalogColumns::across(rect, width);
    let mut content = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    contents(&mut content, &columns);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The part both rows share: the space, and the fill behind it.
///
/// Selection outranks the pointer. A hover says "this is what you would open"
/// and a selection says "this is what is open", and the second is the one that
/// has to survive the pointer moving away.
fn row_frame(
    ui: &mut Ui,
    palette: Palette,
    height: f32,
    selected: bool,
) -> (Rect, Controls, Response) {
    let (rect, response) =
        ui.allocate_exact_size(vec2(ui.available_width(), height), Sense::click());
    // Asked of the rectangle rather than of the response. A control drawn
    // inside the row is the widget egui calls hovered while the pointer is on
    // it, so a row that asked `hovered()` would put its own controls away the
    // moment the pointer reached one, and drop its fill under the identity the
    // user was about to click. This still answers no while a menu or a modal
    // covers the row, because those are areas of their own.
    let controls =
        if ui.rect_contains_pointer(rect) { Controls::Shown } else { Controls::Hidden };
    let fill = match (selected, controls) {
        (true, _) => Some(palette.row_selected),
        (false, Controls::Shown) => Some(palette.row_hover),
        (false, Controls::Hidden) => None,
    };
    if let Some(fill) = fill {
        ui.painter().rect_filled(rect, CornerRadius::ZERO, fill);
    }
    (rect, controls, response)
}

/// Whether a row's own controls are showing.
///
/// The design reveals them on hover and *hides* them off it rather than
/// removing them, so the columns before them do not move when the pointer
/// arrives. This therefore decides whether they are drawn and never whether
/// their column is reserved.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Controls {
    Shown,
    Hidden,
}

/// The controls at the right end of a row, in the column reserved for them.
///
/// Which controls those are is the status's answer and not this function's -
/// see [`crate::status`], where the design's table is transcribed once for the
/// two surfaces that draw it.
///
/// Laid out right to left, because the design right-aligns the group and its
/// DOM order puts the removal nearest the edge. At most two are ever offered at
/// once, so the 84 the design reserves is never close to full.
///
/// Answers which one was pressed.
pub fn row_actions(
    ui: &mut Ui,
    palette: Palette,
    at: Rect,
    controls: Controls,
    status: Status,
    consequences: Consequences,
) -> Option<Action> {
    if controls == Controls::Hidden {
        return None;
    }
    let mut pressed = None;
    let mut group = ui.new_child(
        egui::UiBuilder::new().max_rect(at).layout(Layout::right_to_left(Align::Center)),
    );
    group.spacing_mut().item_spacing.x = metric::ROW_ACTION_GAP;
    for action in status.actions().rev() {
        let label = action.hover(status, consequences);
        if row_action(&mut group, palette, action, &label).clicked() {
            pressed = Some(action);
        }
    }
    pressed
}

/// The control at the right end of a catalog row, in the column reserved for it.
///
/// Which one it is is the status's answer and not this function's - see
/// [`crate::status`], where the catalog's table is transcribed beside the entry
/// list's.
///
/// **Drawn whether or not the pointer is on the row**, and that is the design's
/// own difference from an entry row: `CatalogRow.dc.html:98` gates the control
/// on there *being* one and never on hover, and moves only its opacity. A
/// catalog list is read to decide something, and a press that appears only once
/// you are already over the row is one nobody knows is there.
///
/// Answers whether it was pressed.
pub fn catalog_action(
    ui: &mut Ui,
    palette: Palette,
    at: Rect,
    status: &Published,
    asks: bool,
) -> Option<Offer> {
    let offer = status.offer()?;
    // The design colours the words and never the fill: the action bar's primary
    // stays the only accent fill in the window, which is what keeps it the one
    // thing the window is asking for.
    let ink = match status {
        Published::UpdateAvailable => palette.accent_text,
        Published::DownloadFailed | Published::VerificationFailed => palette.err_text,
        _ => palette.ink,
    };
    let mut group = ui.new_child(
        egui::UiBuilder::new().max_rect(at).layout(Layout::right_to_left(Align::Center)),
    );
    // The shield leads, as it does on the action bar's press, where the press
    // will raise Windows' consent dialog.
    let mut text = egui::text::LayoutJob::default();
    if asks && offer.writes() {
        let glyph = egui::TextFormat {
            color: ink,
            valign: Align::Center,
            ..font::format(font::icon(group.ctx(), font::ICON))
        };
        text.append(icon::ELEVATES, 0.0, glyph);
    }
    let leading = if text.is_empty() { 0.0 } else { metric::CATALOG_ACTION_GAP };
    text.append(
        offer.label(),
        leading,
        egui::TextFormat {
            color: ink,
            valign: Align::Center,
            ..font::format(font::emphasis(group.ctx(), font::CHIP))
        },
    );
    let button = egui::Button::new(text)
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .min_size(vec2(0.0, metric::CATALOG_ACTION));
    let pressed = group
        .scope(|ui| {
            ui.spacing_mut().button_padding = vec2(metric::CATALOG_ACTION_PAD_X, 0.0);
            // A button is never shorter than the style's interactive size, and
            // this one is: the theme sets that to a bar control's 26 and the
            // design draws this at 24. `min_size` alone is a floor and not a
            // ceiling, so it cannot bring a control back down.
            ui.spacing_mut().interact_size.y = metric::CATALOG_ACTION;
            filled_button(ui, palette.btn, palette.btn_hover, button)
                .on_hover_cursor(egui::CursorIcon::PointingHand)
        })
        .inner
        .clicked();
    pressed.then_some(offer)
}

/// One of those controls: a square with a mark in it, and nothing else until
/// the pointer is on it.
///
/// Two of the five are coloured apart from the rest, and both times because of
/// what the press does rather than what it looks like.
/// The mark a control wears, which is the design's own and is the same glyph on
/// the row and in the inspector - `EntryRow.dc.html:46-58` and
/// `Inspector.dc.html:148-156` name the same five.
///
/// One map and two callers, because a second one would be free to drift and the
/// drift would be silent: a control with the wrong glyph still works.
pub fn action_glyph(action: Action) -> &'static str {
    match action {
        Action::Assign => icon::ASSIGN,
        Action::Locate => icon::LOCATE,
        Action::Undo => icon::UNDO,
        Action::Reveal => icon::REVEAL,
        Action::Remove => icon::REMOVE,
    }
}

fn row_action(ui: &mut Ui, palette: Palette, action: Action, label: &str) -> Response {
    let glyph = action_glyph(action);
    let (ink, lit, fill) = match action {
        // The remedy on a broken row, and the only one the design gives the
        // accent rather than the quiet grey the others wear. It keeps its
        // colour under the pointer; only the ground behind it arrives.
        Action::Locate => (palette.accent, palette.accent, palette.accent_soft),
        // The one press on a row that can destroy the user's own work, so it
        // turns red under the pointer instead of merely lighting up.
        Action::Remove => (palette.ink_2, palette.err, palette.err_bg),
        Action::Assign | Action::Undo | Action::Reveal => {
            (palette.ink_2, palette.ink, palette.btn_hover)
        }
    };
    let (rect, response) =
        ui.allocate_exact_size(vec2(metric::ROW_ACTION, metric::ROW_ACTION), Sense::click());
    // What it is, for anything reading the window rather than looking at it -
    // the harness included. A button built from the glyph alone would be
    // announced as a private-use codepoint, which is what the sentence on the
    // tooltip is already for.
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });

    if response.hovered() {
        ui.painter().rect_filled(rect, CornerRadius::same(metric::RADIUS), fill);
    }
    // Painted rather than laid out, for the reason [`identity`] gives: a glyph
    // centred both ways in a box of a stated size is one call here and a nest
    // of centring layouts otherwise.
    ui.painter().text(
        rect.center(),
        egui::Align2::CENTER_CENTER,
        glyph,
        font::icon(ui.ctx(), font::ICON),
        if response.hovered() { lit } else { ink },
    );
    response.on_hover_text(label).on_hover_cursor(egui::CursorIcon::PointingHand)
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
/// The ink is the caller's, because it is not always the same quiet grey: the
/// design warms a selected row's supporting text, and this is the first of it.
pub fn kind_label(ui: &mut Ui, ink: Color32, kind: Kind) {
    let label = match kind {
        Kind::Device => "Device",
        Kind::Modulator => "Modulator",
        Kind::Module => "Grid module",
    };
    ui.label(font::run(label, font::plain(font::CHIP)).color(ink));
}

/// How loud a row's status is, as the design's own map from status to token.
///
/// Transcribed here so that a status added to one screen cannot be drawn a
/// different shade on another, and so the reason beside a row is the same colour
/// as the word it explains.
///
/// Grey means settled or inert, neutral means in flight with nothing to decide,
/// orange means a decision is waiting, red means broken.
pub fn status_colour(palette: Palette, status: Status) -> Color32 {
    match status {
        Status::Staged | Status::PendingRestart => palette.ink_2,
        Status::Changed | Status::UpdateAvailable => palette.accent_text,
        Status::MissingFile | Status::Conflict => palette.err_text,
        Status::Registered | Status::Rejected | Status::PendingRemoval => palette.ink_3,
    }
}

/// The same map for a catalog row - `CatalogRow.dc.html:68-76`.
///
/// A second function rather than a second arm, because the two tables are two
/// tables: `Installed` is the quiet grey of something already dealt with and
/// `Registered` is the quieter grey of a row with nothing to say, and they are
/// not the same colour.
pub fn published_colour(palette: Palette, status: &Published) -> Color32 {
    match status {
        Published::UpdateAvailable => palette.accent_text,
        Published::DownloadFailed | Published::VerificationFailed => palette.err_text,
        // Brighter than `Available`, because both of these are about something
        // the user already has.
        Published::Installed | Published::Superseded | Published::Fetching => palette.ink_2,
        Published::Available | Published::Incompatible(_) => palette.ink_3,
    }
}

/// The ink an installed item's *name* is set in.
///
/// The design dims it: a catalog list is read to choose something, and the rows
/// that are not a choice any more step back. `CatalogRow.dc.html:89`, which dims
/// on exactly the two states where the item is on this machine and needs
/// nothing - an update is still a choice, so it keeps the full ink.
pub fn published_name_colour(palette: Palette, status: &Published) -> Color32 {
    match status {
        Published::Installed | Published::Superseded => palette.ink_2,
        _ => palette.ink,
    }
}

/// The ink a row's supporting text is set in, which the design warms while a
/// panel is open on that row - so a selected row reads as one thing rather than
/// as an ordinary row with a colour behind it.
///
/// Here rather than at the two rows that ask, for the reason the module says:
/// a screen composes these and does not pick a colour of its own.
pub fn supporting_ink(palette: Palette, selected: bool) -> Color32 {
    if selected { palette.ink_3_warm } else { palette.ink_3 }
}

/// What state the installation's registry is in, as the design colours it.
pub fn badge_colour(palette: Palette, badge: &str) -> Color32 {
    match badge {
        "Needs re-apply" => palette.accent_text,
        "Modified elsewhere" => palette.err_text,
        _ => palette.ink_2,
    }
}

/// The frame a side panel sits in.
///
/// No margin of its own: the header is full width and fills to the panel's
/// edges, and the body puts in the padding the design gives it.
fn panel(palette: Palette) -> Frame {
    Frame::new().fill(palette.panel_2).inner_margin(Margin::ZERO)
}

/// Where an open side panel ended up, and the debt it leaves behind.
///
/// Handed back by [`aside`] rather than a bare rectangle so that the shadow
/// cannot be forgotten: it has to be painted later than the panel and by
/// somebody else, which is exactly the kind of call that goes missing when a
/// third panel is added. Dropping this instead of paying it is a warning.
#[must_use = "an aside casts a shadow on the page it falls on"]
pub struct Aside(Rect);

/// A panel over the right of the list, as the design draws every one of them:
/// as wide as [`metric::ASIDE`], with no separator line, because the design
/// separates it from the list by a shadow instead.
///
/// Claimed before the region it leaves, so that the toolbar and the rows beside
/// it are laid out in what is left rather than drawn over.
pub fn aside<T>(
    ui: &mut Ui,
    palette: Palette,
    id: &'static str,
    contents: impl FnOnce(&mut Ui) -> T,
) -> (Aside, T) {
    let mut out = None;
    let rect = egui::Panel::right(id)
        .exact_size(metric::ASIDE)
        .resizable(false)
        .show_separator_line(false)
        .frame(panel(palette))
        .show(ui, |ui| out = Some(contents(ui)))
        .response
        .rect;
    (Aside(rect), out.expect("the panel's contents run once"))
}

impl Aside {
    /// The shadow, cast to the *left*, because the design slides the panel over
    /// the right of the list rather than standing it beside the list.
    ///
    /// **Not the frame's own shadow, and painted after the page rather than
    /// with the panel.** A side panel is claimed before the region it leaves
    /// and painted before it too, so a shadow reaching out of the panel reaches
    /// into the page's own rectangle and the page's fill goes straight over it.
    /// Which is why this takes the panel's rect and is called last.
    ///
    /// Clipped to what is left of the panel, because a shadow is a filled
    /// rectangle with a blur on it: in a frame the fill is painted over the
    /// middle of it afterwards, and here there is nothing left to do that.
    pub fn shadow(self, ui: &Ui, palette: Palette) {
        ui.painter()
            .with_clip_rect(Rect::everything_left_of(self.0.left()))
            .add(palette.panel_shadow.as_shape(self.0, CornerRadius::ZERO));
    }
}

/// What the inspector's two editable fields hold while the panel is open.
///
/// Beside the entry rather than inside it. A field is edited one character at a
/// time and an entry is written a whole one at a time, and what closes the gap
/// is leaving the field - so the buffer is what is being typed and the entry is
/// what has been said.
#[derive(Debug, Default, Clone)]
pub struct Words {
    /// What Bitwig's browser shows under the device.
    pub description: String,
    /// The words that find it when typed into that browser.
    pub keywords: Vec<String>,
    /// The keyword being typed, which is not one until it is committed.
    pub adding: String,
}

impl Words {
    /// The words as an entry currently states them.
    pub fn of(description: &str, keywords: &[String]) -> Words {
        Words {
            description: description.to_owned(),
            keywords: keywords.to_vec(),
            adding: String::new(),
        }
    }
}

/// One entry, as the inspector has to say it.
///
/// Everything here is read off the entry and the machine by the caller. The
/// panel states facts; working out what is true is not a drawing job, and a
/// widget that reached for the filesystem could not be rendered from a fixture.
pub struct Inspected<'a> {
    pub kind: Kind,
    pub name: &'a str,
    pub uuid: &'a str,
    /// Where the registry says the document is, relative to the library.
    pub path: &'a str,
    /// Where the document came from, drawn as the design draws it: an icon and
    /// a phrase.
    pub source: &'a str,
    pub source_icon: &'a str,
    /// The change that published it, as a label and the link behind it - the
    /// same pair [`Detailed`] carries, and absent for anything that did not come
    /// from the catalog or that was registered before the entry list recorded
    /// one.
    pub provenance: Option<(&'a str, &'a str)>,
    /// Where the document actually is, which is a different question.
    pub placement: &'a Placement,
    /// Which of the nine states this entry is in, which is what decides the
    /// panel's action list - see [`crate::status`].
    pub status: Status,
    /// What removing this entry would do to its document, which is a setting
    /// and not a property of the entry. Carried here because the panel's
    /// removal control has to name it, exactly as the row's does.
    pub document: TheDocument,
}

/// What was pressed in the inspector, if anything was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Inspecting {
    Nothing,
    Closed,
    CopiedUuid,
    /// The change that published this entry, where it came from the catalog.
    /// The same press the detail panel's own provenance line is, leading to the
    /// same commit - one fact about the item, stated on both surfaces.
    Provenance,
    /// One of the entry's own controls, which are the row's controls with
    /// words on them. The same press and the same consequence: queueing a
    /// removal from here leaves the row there, struck through, until the apply
    /// that takes it away, exactly as the row's own trash does.
    Acted(Action),
    /// A field was finished with: it lost focus, or a keyword was added or
    /// taken away. Whatever is in [`Words`] is what the entry should now say.
    Edited,
}

/// The inspector: everything one entry is, and what can be done about it.
///
/// Drawn into a panel of [`metric::ASIDE`] that the caller has already
/// taken out of the window, so the list beside it has already been laid out
/// narrower. The design draws them as siblings for the same reason: the panel
/// is not an overlay with the list still live underneath.
pub fn inspector(
    ui: &mut Ui,
    palette: Palette,
    item: &Inspected<'_>,
    words: &mut Words,
) -> Inspecting {
    let mut pressed = Inspecting::Nothing;
    // Before the header, not after it. egui puts `item_spacing` between every
    // allocated widget, and the header and the body below it are two of them:
    // six pixels went in between, and every field in the panel drew six low.
    ui.spacing_mut().item_spacing.y = 0.0;
    let closed = panel_header(ui, palette, item.name, |ui| {
        kind_label(ui, palette.ink_3, item.kind);
    });
    if closed.clicked() {
        pressed = Inspecting::Closed;
    }

    panel_body(ui, |ui| {
        label_above(ui, palette, "Display name", metric::UNDER_A_FIELD_LABEL);
        one_line_field(ui, palette, item.name);

        ui.add_space(metric::BETWEEN_GROUPS);
        label_above(ui, palette, "Description", metric::UNDER_A_FIELD_LABEL);
        if paragraph_field(ui, palette, &mut words.description) {
            pressed = Inspecting::Edited;
        }
        ui.add_space(metric::UNDER_A_FIELD_LABEL);
        footnote(ui, palette, "Shown under the device in Bitwig's browser.");

        ui.add_space(metric::BETWEEN_GROUPS);
        label_above(ui, palette, "Search keywords", metric::UNDER_A_FIELD_LABEL);
        if keyword_field(ui, palette, words) {
            pressed = Inspecting::Edited;
        }
        ui.add_space(metric::UNDER_A_FIELD_LABEL);
        footnote(ui, palette, "Proposed from the name.");

        rule(ui, palette, metric::BETWEEN_GROUPS);
        if let Some(fact) = facts(ui, palette, item) {
            pressed = fact;
        }

        rule(ui, palette, metric::BETWEEN_GROUPS);
        // The design's own line: "the panel's status rules are the row's status
        // rules, not a second set". So this list is [`Status::actions`], in the
        // order the row lays them out, and the defect the README names -
        // offering `Reveal file` on the one entry whose file cannot be found -
        // is unreachable rather than avoided.
        for action in item.status.actions() {
            let control =
                panel_action(ui, palette, action_glyph(action), action.in_the_panel(), weight_of(action));
            // The one control here whose tooltip is not its label: what
            // removing does to the document is the user's setting and not a
            // property of the control, so the control says which way it is set.
            let control = match action {
                Action::Remove => {
                    control.on_hover_text(crate::status::removal_consequence(item.document))
                }
                _ => control,
            };
            if control.clicked() {
                pressed = Inspecting::Acted(action);
            }
        }
    });
    pressed
}

/// The header both panels wear: what kind of thing this is, what it is called,
/// and the one control that puts the panel away.
///
/// The tag is the caller's, because it is the only thing the two differ in -
/// the inspector names the kind as a word and the catalog's detail sets it as a
/// lowercase monospaced tag. Everything else here is one shape in both bundles,
/// down to the eight between the tag and the name.
///
/// Answers whether the close was pressed.
fn panel_header(ui: &mut Ui, palette: Palette, name: &str, tag: impl FnOnce(&mut Ui)) -> Response {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), metric::ASIDE_HEADER),
        Sense::hover(),
    );
    // A fill of its own, and darker than the panel: the design separates the
    // header from the fields by colour rather than by a line, exactly as it
    // separates the bars from the working area.
    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.panel);

    let mut line = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(metric::PAD, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    line.spacing_mut().item_spacing.x = 0.0;
    tag(&mut line);
    line.add_space(metric::TOOL_GAP);

    // The close first, from the right, so the name is truncated by what is
    // left rather than pushing the control off the panel.
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        let closed = dismiss(ui, palette);
        ui.add_space(metric::TOOL_GAP);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.add(
                egui::Label::new(
                    font::run(name, font::emphasis(ui.ctx(), font::ROW_NAME)).color(palette.ink),
                )
                .truncate(),
            );
        });
        closed
    })
    .inner
}

/// The bar at the foot of a panel, on the header's colour rather than the body's.
///
/// Claimed before [`panel_body`] and never after it, for the reason the aside
/// itself is claimed before the page: the body scrolls in what is left, and a
/// bar allocated after it would be laid out inside the scrolling region and
/// travel with the content.
fn panel_foot<T>(ui: &mut Ui, palette: Palette, contents: impl FnOnce(&mut Ui) -> T) -> T {
    let mut out = None;
    egui::Panel::bottom("panel-foot")
        .frame(
            Frame::new().fill(palette.panel).inner_margin(Margin::symmetric(
                metric::PANEL_FOOT_PAD_X as i8,
                metric::PANEL_FOOT_PAD_Y as i8,
            )),
        )
        .show_separator_line(false)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = metric::ALONG_A_PANEL_FOOT;
                // The taller of the two controls, stated rather than arrived
                // at: `align-items:center` centres the shorter one against the
                // bar, and a row that is only as tall as whatever was laid out
                // first has nothing to centre the next thing against - which
                // leaves the 30 sitting on the 32's top edge, one pixel out.
                ui.set_min_height(metric::PANEL_PRIMARY);
                out = Some(contents(ui));
            });
        });
    out.expect("the foot's contents run once")
}

/// The removal at the left end of that bar.
///
/// `--ink-2`, and red only under the pointer: the same promise the row's own
/// removal makes, made the same way. What becomes of the document is a setting,
/// so the caller states it on the hover rather than this function guessing.
/// **Painted rather than laid out as a button**, because what the pointer
/// changes here is the ink and not the ground - `style-hover:color:var(
/// --err-text)` and no fill of its own. A button carries its colour in the text
/// it was built from, so it is settled before anything can be asked about the
/// pointer; the design's own order is the other way round.
fn panel_remove(ui: &mut Ui, palette: Palette, consequence: &str) -> Response {
    let font = font::plain(font::CONTROL);
    let words = ui.painter().layout_no_wrap("Remove".to_owned(), font, palette.ink_2);
    let (rect, response) = ui.allocate_exact_size(
        vec2(words.size().x + 2.0 * metric::PANEL_REMOVE_PAD_X, metric::PANEL_REMOVE),
        Sense::click(),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), "Remove")
    });

    ui.painter().rect_filled(rect, CornerRadius::same(metric::RADIUS), palette.btn);
    let ink = if response.hovered() { palette.err_text } else { palette.ink_2 };
    ui.painter().galley(
        rect.center() - words.size() / 2.0,
        ui.painter().layout_no_wrap("Remove".to_owned(), font::plain(font::CONTROL), ink),
        ink,
    );
    response.on_hover_cursor(egui::CursorIcon::PointingHand).on_hover_text(consequence)
}

/// The primary at the right end of it, which is the one accent fill either panel
/// draws.
fn panel_primary(ui: &mut Ui, palette: Palette, label: &str) -> Response {
    let button =
        egui::Button::new(font::run(label, font::emphasis(ui.ctx(), font::ACTION)).color(palette.accent_ink))
            .stroke(Stroke::NONE)
            .corner_radius(CornerRadius::same(metric::RADIUS))
            .min_size(vec2(0.0, metric::PANEL_PRIMARY));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(metric::PANEL_PRIMARY_PAD_X, 0.0);
        filled_button(ui, palette.accent, palette.accent, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// The scrolling body a panel puts its fields in, with the padding the design
/// gives it.
///
/// The gaps inside are stated one at a time by whatever is drawn in here: the
/// design states every one of them, and egui's own six between widgets is not
/// one of them.
fn panel_body(ui: &mut Ui, contents: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new()
            .inner_margin(Margin::symmetric(metric::PAD as i8, metric::ASIDE_PAD_Y as i8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                contents(ui);
            });
    });
}

/// The heading over a field, a fact or a group of settings, with the air the
/// design puts under it.
///
/// Public because the Settings screen repeats it five times and is drawn from
/// `app.rs`, on the other side of this module. The two beside it -
/// [`one_line_field`] and [`footnote`] - looked like the same case and are not:
/// Settings' path boxes are monospaced and four pixels shorter, and its
/// explanatory sentences are 10.5 on the design's own leading where a footnote is
/// 10 on none. Both were measured rather than assumed, and both got their own
/// shape: `path_field` and [`font::wrapping`].
pub fn label_above(ui: &mut Ui, palette: Palette, label: &str, gap: f32) {
    ui.label(font::run(label, font::emphasis(ui.ctx(), font::NOTE)).color(palette.ink_2));
    ui.add_space(gap);
}

/// The smallest thing the design writes: what Bitwig does with the field above.
fn footnote(ui: &mut Ui, palette: Palette, text: &str) {
    ui.label(font::run(text, font::plain(font::FOOTNOTE)).color(palette.ink_3));
}

/// A field holding one line.
fn one_line_field(ui: &mut Ui, palette: Palette, text: &str) {
    field_frame(palette, Margin::symmetric(metric::FIELD_PAD as i8, 0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_height(metric::FIELD);
        ui.horizontal_centered(|ui| {
            ui.add(
                egui::Label::new(
                    font::run(text, font::plain(font::FIELD_VALUE)).color(palette.ink),
                )
                .truncate(),
            );
        });
    });
}

/// A field holding a sentence, which grows with the sentence and never shrinks
/// below the height the design draws it empty at.
///
/// Answers whether it was finished with, which is when it lost focus: what is
/// typed into a description is a description only once the user has stopped.
fn paragraph_field(ui: &mut Ui, palette: Palette, text: &mut String) -> bool {
    field_frame(
        palette,
        Margin::symmetric(metric::FIELD_PAD as i8, metric::PARAGRAPH_PAD_Y as i8),
    )
    .show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(metric::PARAGRAPH - 2.0 * metric::PARAGRAPH_PAD_Y);
        ui.add(
            egui::TextEdit::multiline(text)
                .desired_width(f32::INFINITY)
                .desired_rows(1)
                .font(font::plain(font::CONTROL))
                .text_color(palette.ink)
                // The box is the frame above, and its padding is that frame's.
                .frame(Frame::NONE)
                .margin(Margin::ZERO),
        )
        .lost_focus()
    })
    .inner
}

/// The box the search keywords sit in, and the one being typed.
///
/// Answers whether the list changed.
fn keyword_field(ui: &mut Ui, palette: Palette, words: &mut Words) -> bool {
    let mut changed = false;
    // The design's `min-height` is on the content and not on the box, so an
    // empty keyword field is 27 inside its padding rather than 27 overall.
    keyword_box(ui, palette, metric::KEYWORDS, |ui| {
        let mut drop = None;
        for (at, word) in words.keywords.iter().enumerate() {
            if keyword(ui, palette, word, true) {
                drop = Some(at);
            }
        }
        if let Some(at) = drop {
            words.keywords.remove(at);
            changed = true;
        }

        // What is left of the row the chips ended on, and never less than the
        // words in it: `available_width` is the whole row in a wrapped layout
        // rather than the rest of it, so asking for that put the field on a
        // line of its own and made a one-keyword box two rows tall. The floor
        // is what wraps it when there is genuinely no room.
        const ADD: &str = "add...";
        let rest = ui.available_size_before_wrap().x;
        let least = text_width(ui, ADD, font::plain(font::NOTE));
        let typing = ui.add(
            egui::TextEdit::singleline(&mut words.adding)
                // Named, because egui would otherwise derive its id from where
                // it sits in the layout - and it sits after the chips, so
                // committing a word moves it, gives it a new id and drops the
                // focus that had just been handed back.
                .id_salt("add-keyword")
                .hint_text(font::run(ADD, font::plain(font::NOTE)).color(palette.ink_3))
                .desired_width(rest.max(least))
                .font(font::plain(font::NOTE))
                .text_color(palette.ink)
                .frame(Frame::NONE)
                .margin(Margin::ZERO),
        );
        // Enter and clicking away are the same statement, and a singleline
        // field gives up focus on Enter - so both arrive here. Whitespace
        // separates keywords in the bundle Bitwig reads, so it separates them
        // here: whatever was typed splits into words.
        if typing.lost_focus() && !words.adding.trim().is_empty() {
            for word in words.adding.split_whitespace() {
                if !words.keywords.iter().any(|had| had == word) {
                    words.keywords.push(word.to_owned());
                    changed = true;
                }
            }
            words.adding.clear();
            // Enter means "and another", so the field is handed its focus back
            // rather than dropping the user out of a list they are half way
            // through.
            if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                typing.request_focus();
            }
        }
    });
    changed
}

/// The box a row of keyword chips sits in, in either panel.
///
/// **`interact_size.y` is pinned to the chip's own height.** A wrapped
/// horizontal layout starts its row at that size, and the theme sets it to a
/// bar control's 26 - which made a box holding one 18-tall chip four pixels
/// taller than the design draws one, with all four of them above the chip.
///
/// `least` is what the box is never shorter than, which the design states for
/// the field that can be typed into and not for the one that cannot.
fn keyword_box(ui: &mut Ui, palette: Palette, least: f32, contents: impl FnOnce(&mut Ui)) {
    field_frame(
        palette,
        Margin::symmetric(metric::KEYWORDS_PAD_X as i8, metric::KEYWORDS_PAD_Y as i8),
    )
    .show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_min_height(least);
        ui.spacing_mut().item_spacing = vec2(metric::BETWEEN_KEYWORDS, metric::BETWEEN_KEYWORDS);
        ui.spacing_mut().interact_size.y = metric::KEYWORD;
        ui.horizontal_wrapped(contents);
    });
}

/// One keyword: a word on the accent, which is how the design says these are
/// the entry's own rather than something read off it, and the control that
/// takes it away.
///
/// Answers whether that control was pressed.
fn keyword(ui: &mut Ui, palette: Palette, word: &str, removable: bool) -> bool {
    let mut removed = false;
    Frame::new()
        .fill(palette.accent)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .inner_margin(Margin::symmetric(metric::KEYWORD_PAD_X as i8, 0))
        .show(ui, |ui| {
            ui.set_height(metric::KEYWORD);
            ui.spacing_mut().item_spacing.x = metric::ALONG_A_CHIP;
            ui.horizontal_centered(|ui| {
                ui.label(font::run(word, font::plain(font::NOTE)).color(palette.accent_ink));
                if !removable {
                    return;
                }
                removed = ui
                    .add(
                        egui::Label::new(
                            font::run(REMOVE_KEYWORD, font::plain(font::CHIP))
                                // The design draws this at three quarters, so
                                // it reads as the word's own control rather
                                // than as a second word beside it.
                                .color(palette.accent_ink.gamma_multiply(0.75)),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked();
            });
        });
    removed
}

/// The mark that takes a keyword away, which the design draws as a multiplication
/// sign rather than as an icon. Escaped rather than typed, as `SEPARATOR` is,
/// and both faces were checked for the glyph.
const REMOVE_KEYWORD: &str = "\u{d7}";

/// The surface a field is written on.
///
/// The margin is the caller's because every field the design draws states its
/// own, and the fill and the four-pixel corner are what they all share.
fn field_frame(palette: Palette, margin: Margin) -> Frame {
    Frame::new()
        .fill(palette.field)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(margin)
}

/// A hairline between two groups, with the group gap on both sides of it.
///
/// The gap is the caller's because it is the panel's and not the rule's: in
/// both bundles the hairline is a child of the body column, so the air around
/// it is that column's own `gap` - 14 in the inspector and 13 in the catalog's
/// detail.
fn rule(ui: &mut Ui, palette: Palette, gap: f32) {
    ui.add_space(gap);
    let (rect, _) =
        ui.allocate_exact_size(vec2(ui.available_width(), metric::HAIRLINE), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.line);
    ui.add_space(gap);
}

/// What is true of this entry rather than what can be typed into it: the
/// identity, where the registry points, where it came from, and where it is.
///
/// Answers which of its two presses happened, if either did.
fn facts(ui: &mut Ui, palette: Palette, item: &Inspected<'_>) -> Option<Inspecting> {
    label_above(ui, palette, "UUID", metric::UNDER_A_FACT);
    // In a `horizontal`, which takes the height of what is in it. A bare
    // `with_layout` in a top-down column takes the whole of what is left of the
    // column instead, and the four facts below this one end up off the panel.
    //
    // The control first, from the right, so that the identity is laid out in
    // what is left of the line rather than pushing the copy off the panel.
    let copied = ui
        .horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = metric::TIGHT;
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let copy = ui
                    .add(
                        egui::Label::new(
                            font::run(icon::COPY, font::icon(ui.ctx(), font::ICON))
                                .color(palette.ink_3),
                        )
                        .sense(Sense::click()),
                    )
                    .on_hover_text("Copy")
                    .on_hover_cursor(egui::CursorIcon::PointingHand);
                ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                    ui.label(font::run(item.uuid, font::mono(font::MONO)).color(palette.ink_2));
                });
                copy.clicked()
            })
            .inner
        })
        .inner;

    ui.add_space(metric::BETWEEN_FACTS);
    label_above(ui, palette, "Registered library path", metric::UNDER_A_FACT);
    ui.label(font::run(item.path, font::mono(font::MONO)).color(palette.ink_2));

    ui.add_space(metric::BETWEEN_FACTS);
    label_above(ui, palette, "Source", metric::UNDER_A_FACT);
    ui.label(labelled_icon(
        ui,
        item.source_icon,
        item.source,
        font::CHIP,
        palette.ink_2,
        palette.ink_3,
        metric::ALONG_A_PANEL_ACTION,
    ));
    // Under the source and inside the same group, which is why this is the one
    // gap here that is the group's own rather than [`metric::BETWEEN_FACTS`] -
    // `Inspector.dc.html:84` sets the Source block on `gap:3px`. It says where
    // this machine's copy came through, which is a different fact from the
    // catalog detail's line about the item; the two carry the same URL because
    // it is the same commit.
    let mut reviewed = None;
    if let Some((label, _)) = item.provenance {
        ui.add_space(metric::UNDER_A_FACT);
        let line = format!("Reviewed in {label}");
        if ui
            .add(
                egui::Label::new(font::run(line, font::plain(font::NOTE)).color(palette.accent_text))
                    .sense(Sense::click()),
            )
            .on_hover_cursor(egui::CursorIcon::PointingHand)
            .clicked()
        {
            reviewed = Some(Inspecting::Provenance);
        }
    }

    ui.add_space(metric::BETWEEN_FACTS);
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = metric::TIGHT;
        ui.label(font::run("Placement", font::emphasis(ui.ctx(), font::NOTE)).color(palette.ink_2));
        placement_chip(ui, palette, item.placement);
    });
    // The identity first, because it is the press above: two presses cannot
    // happen in one frame, and if they somehow did the nearer one is the one
    // the pointer is on.
    copied.then_some(Inspecting::CopiedUuid).or(reviewed)
}

/// Where a registered document actually is, as the design tones it: a link is
/// the goal state, a copy is merely a fact, and nothing there is an error.
fn placement_chip(ui: &mut Ui, palette: Palette, placement: &Placement) {
    let (tone, glyph, label) = match placement {
        Placement::Linked(_) => (Tone::Ok, icon::LINKED, "linked"),
        Placement::Copied(_) => (Tone::Neutral, icon::COPIED, "copied"),
        Placement::Unresolved(_) => (Tone::Err, icon::UNRESOLVED, "unresolved"),
    };
    Frame::new()
        .fill(tone.wash(palette))
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .inner_margin(Margin::symmetric(metric::PLACEMENT_PAD_X as i8, 0))
        .show(ui, |ui| {
            ui.set_height(metric::PLACEMENT);
            ui.horizontal_centered(|ui| {
                ui.label(labelled_icon(
                    ui,
                    glyph,
                    label,
                    font::FOOTNOTE,
                    tone.colour(palette),
                    tone.colour(palette),
                    metric::ALONG_A_CHIP,
                ));
            });
        });
}

/// How loud one of the inspector's actions is.
///
/// Two are coloured apart from the rest, and both for what the press means
/// rather than for what it looks like. Coloured before the pointer arrives
/// rather than under it: the panel has room to write the words, so it does not
/// wait to warn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Weight {
    Ordinary,
    /// The remedy on a broken entry, which the design draws in `--accent-text`
    /// so that the one action worth taking is the one the eye finds.
    Remedy,
    Destructive,
}

/// How loud each control is in the panel - `Inspector.dc.html:148-156`.
///
/// The same two the row singles out, and they have to agree: a locate drawn
/// quiet in one place and accent in the other says the remedy matters on one
/// surface and not the other.
fn weight_of(action: Action) -> Weight {
    match action {
        Action::Locate => Weight::Remedy,
        Action::Remove => Weight::Destructive,
        Action::Assign | Action::Undo | Action::Reveal => Weight::Ordinary,
    }
}

/// One line of the inspector's action list.
///
/// Its icon lines up with the labels above it and its fill reaches seven
/// further out on both sides, which is what the design's `margin:0 -7px`
/// against `padding:5px 7px` means. So the space is claimed in the column and
/// only the fill is drawn outside it: allocating the wider box instead would
/// push the whole action list seven pixels left of everything else in the
/// panel.
fn panel_action(
    ui: &mut Ui,
    palette: Palette,
    glyph: &str,
    label: &str,
    weight: Weight,
) -> Response {
    let (ink, wash) = match weight {
        Weight::Ordinary => (palette.ink_2, palette.btn_hover),
        Weight::Remedy => (palette.accent_text, palette.accent_soft),
        Weight::Destructive => (palette.err, palette.err_bg),
    };
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), metric::PANEL_ACTION),
        Sense::click(),
    );
    if response.hovered() {
        ui.painter().rect_filled(
            rect.expand2(vec2(metric::PANEL_ACTION_PAD_X, 0.0)),
            CornerRadius::same(metric::FIELD_RADIUS),
            wash,
        );
    }
    let mut line = ui.new_child(
        egui::UiBuilder::new().max_rect(rect).layout(Layout::left_to_right(Align::Center)),
    );
    let text = labelled_icon(
        &line,
        glyph,
        label,
        font::CONTROL,
        ink,
        ink,
        metric::ALONG_A_PANEL_ACTION,
    );
    line.label(text);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// One published item, as the catalog's detail panel has to say it.
///
/// The inspector's opposite number, and it answers a different question: not
/// "what is this thing I have" but "should I install this". So it leads with
/// who wrote it, what it is for, whether this Bitwig can load it and what it is
/// licensed under, and the identity is the last line rather than the first
/// field.
pub struct Detailed<'a> {
    pub kind: Kind,
    pub name: &'a str,
    pub author: &'a str,
    pub version: &'a str,
    pub description: &'a str,
    /// The oldest Bitwig that loads it, and whether this one is old enough.
    pub requires: &'a str,
    pub compatible: bool,
    pub licence: &'a str,
    pub keywords: &'a [String],
    pub uuid: &'a str,
    /// The change that published it, as a label and the link behind it. Absent
    /// in an index generated inside a pull request, which cannot name the
    /// commit that has not merged yet.
    pub provenance: Option<(&'a str, &'a str)>,
    pub homepage: Option<&'a str>,
    /// The item that takes this one's place, if one does.
    pub replaced_by: Option<&'a str>,
    /// Which of the design's eight states this item is in, which is what decides
    /// the foot of the panel - see [`crate::status`].
    pub status: &'a Published,
    /// What removing it would do to its document, for the removal to name, in
    /// the words the row's own control uses. Only read where the item is
    /// installed, which is the only state that offers the control.
    pub document: TheDocument,
}

/// What was pressed in the detail panel, if anything was.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Detailing {
    Nothing,
    Closed,
    /// The change that published the item, which is where the review of it is.
    Provenance,
    Homepage,
    /// The item that replaces this one.
    Replacement,
    /// The one control the item's state offers, whichever that is.
    ///
    /// The single press variant, for the reason the inspector has one: what the
    /// panel offers is the row's own control with room for a word, so what it
    /// does has to be the row's press and not a second implementation of it.
    Acted(Offer),
    /// Take the entry this item is installed as out of the list. The queued
    /// removal the Local row already has, because an installed catalog item is
    /// a registered entry and there is only one way to stop being one.
    Removed,
}

/// The catalog's detail panel.
pub fn detail(ui: &mut Ui, palette: Palette, item: &Detailed<'_>) -> Detailing {
    let mut pressed = Detailing::Nothing;
    // For the reason written on the inspector's: the header and the body under
    // it are two allocated widgets, and egui would put six pixels between them.
    ui.spacing_mut().item_spacing.y = 0.0;
    let closed = panel_header(ui, palette, item.name, |ui| {
        ui.label(font::run(kind_tag(item.kind), font::mono(font::MONO_TIGHT)).color(palette.ink_3));
    });
    if closed.clicked() {
        pressed = Detailing::Closed;
    }

    // Before the body, which scrolls in what is left. A state with neither
    // control draws no bar at all, which is a shape the bundle already has:
    // both of its halves are behind an `sc-if`.
    let primary = item.status.primary();
    if item.status.installed() || primary.is_some() {
        panel_foot(ui, palette, |ui| {
            if item.status.installed()
                && panel_remove(ui, palette, crate::status::removal_consequence(item.document))
                    .clicked()
            {
                pressed = Detailing::Removed;
            }
            if let Some(offer) = primary {
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    if panel_primary(ui, palette, offer.label()).clicked() {
                        pressed = Detailing::Acted(offer);
                    }
                });
            }
        });
    }

    panel_body(ui, |ui| {
        if let Some(replacement) = item.replaced_by {
            if superseded(ui, palette, replacement) {
                pressed = Detailing::Replacement;
            }
            ui.add_space(metric::BETWEEN_DETAIL_GROUPS);
        }

        // Who wrote it and which publication this is, on one line: the author
        // is the trust signal and the version is what an update is measured
        // against.
        ui.horizontal(|ui| {
            ui.spacing_mut().item_spacing.x = 0.0;
            let text = labelled_icon(
                ui,
                icon::AUTHOR,
                item.author,
                font::CONTROL,
                palette.ink,
                palette.ink_3,
                metric::ALONG_A_DETAIL_LINE,
            );
            ui.label(text);
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                ui.label(font::run(item.version, font::mono(font::MONO)).color(palette.ink_3));
            });
        });

        ui.add_space(metric::BETWEEN_DETAIL_GROUPS);
        ui.label(
            font::wrapping(item.description, font::CONTROL, font::Leading::Describing)
                .color(palette.ink_2),
        );

        ui.add_space(metric::BETWEEN_DETAIL_GROUPS);
        // Whether this installation can load it at all, which is the one thing
        // here that is about the machine rather than about the item, and the
        // one the design colours.
        let (glyph, ink) = if item.compatible {
            (icon::COMPATIBLE, palette.ink_2)
        } else {
            (icon::INCOMPATIBLE, palette.err_text)
        };
        let requires = format!("Requires Bitwig {} or newer", item.requires);
        detail_fact(ui, glyph, &requires, ink);
        ui.add_space(metric::BETWEEN_FACTS);
        detail_fact(ui, icon::LICENCE, item.licence, palette.ink_2);

        ui.add_space(metric::BETWEEN_DETAIL_GROUPS);
        label_above(ui, palette, "Search keywords", metric::UNDER_A_FIELD_LABEL);
        // No minimum on this one, unlike the inspector's: a published item's
        // keywords are the author's and there is nothing to type into, so the
        // box is as tall as what is in it.
        keyword_box(ui, palette, metric::KEYWORD, |ui| {
            for word in item.keywords {
                keyword(ui, palette, word, false);
            }
        });
        ui.add_space(metric::UNDER_A_FIELD_LABEL);
        footnote(ui, palette, "Editable in Local once installed.");

        rule(ui, palette, metric::BETWEEN_DETAIL_GROUPS);

        if let Some((label, _)) = item.provenance {
            let reviewed = format!("Reviewed in {label}");
            if detail_link(ui, palette, icon::PROVENANCE, &reviewed) {
                pressed = Detailing::Provenance;
            }
            ui.add_space(metric::BETWEEN_FACTS);
        }
        if let Some(homepage) = item.homepage {
            if detail_link(ui, palette, icon::HOMEPAGE, homepage) {
                pressed = Detailing::Homepage;
            }
            ui.add_space(metric::BETWEEN_FACTS);
        }
        label_above(ui, palette, "UUID", metric::UNDER_A_FACT);
        ui.label(font::run(item.uuid, font::mono(font::MONO)).color(palette.ink_3));
    });
    pressed
}

/// The kind as the detail panel tags it: lowercased and in the monospaced face,
/// which is the design's way of making it a tag rather than a word.
///
/// Not the same words as [`kind_label`]'s - a grid module is tagged `module`
/// here and named `Grid module` in a list - so the two maps are two facts.
fn kind_tag(kind: Kind) -> &'static str {
    match kind {
        Kind::Device => "device",
        Kind::Modulator => "modulator",
        Kind::Module => "module",
    }
}

/// One line of the detail panel: an icon, and what it is about.
fn detail_fact(ui: &mut Ui, glyph: &str, text: &str, ink: Color32) {
    ui.add(egui::Label::new(fact_run(ui, glyph, text, ink, ink)).truncate());
}

/// The same line drawn as somewhere to go, which is the only kind that senses a
/// press. Answers whether it was pressed.
///
/// The colour is this function's own rather than the caller's: what makes a
/// line a link is that it leads somewhere, and reading that back off the ink it
/// was handed made every future fact drawn in the accent into a link.
fn detail_link(ui: &mut Ui, palette: Palette, glyph: &str, text: &str) -> bool {
    let run = fact_run(ui, glyph, text, palette.accent_text, palette.ink_3);
    ui.add(egui::Label::new(run).truncate().sense(Sense::click()))
        .on_hover_cursor(egui::CursorIcon::PointingHand)
        .clicked()
}

/// The icon and the words of a detail line, set as one run so the two share a
/// baseline.
fn fact_run(ui: &Ui, glyph: &str, text: &str, ink: Color32, icon_ink: Color32) -> egui::WidgetText {
    labelled_icon(ui, glyph, text, font::CHIP, ink, icon_ink, metric::ALONG_A_DETAIL_LINE)
}

/// The notice that a newer device has taken this one's place.
///
/// A wash and a control rather than a status word, because the thing to say is
/// not "this is old" but "there is another one, and installing it will not
/// disturb the projects you already have" - which is what a new identity buys
/// and the only reason the catalog publishes both.
///
/// Answers whether the control was pressed.
fn superseded(ui: &mut Ui, palette: Palette, replacement: &str) -> bool {
    let mut go = false;
    Frame::new()
        .fill(palette.accent_soft)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(Margin::symmetric(metric::NOTICE_PAD_X as i8, metric::NOTICE_PAD_Y as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(
                font::run(
                    "A newer version exists as a separate device",
                    font::emphasis(ui.ctx(), font::CONTROL),
                )
                .color(palette.ink),
            );
            ui.add_space(metric::IN_A_NOTICE);
            ui.label(
                font::wrapping(
                    format!(
                        "{replacement} replaces this one. It has its own identity, so \
                         installing it leaves your projects alone and both can be installed \
                         at once."
                    ),
                    font::NOTE,
                    font::Leading::Noticing,
                )
                .color(palette.ink_2_warm),
            );
            ui.add_space(metric::IN_A_NOTICE);
            ui.horizontal(|ui| {
                // The words first and the arrow after them, as the design has
                // it and as the primary action has it: the words say where this
                // goes and the arrow says only that it goes somewhere.
                let mut text = egui::text::LayoutJob::default();
                text.append(
                    &format!("See {replacement}"),
                    0.0,
                    egui::TextFormat {
                        color: palette.ink,
                        valign: Align::Center,
                        ..font::format(font::plain(font::CHIP))
                    },
                );
                text.append(
                    icon::REPLACEMENT,
                    metric::TIGHT,
                    egui::TextFormat {
                        color: palette.ink,
                        valign: Align::Center,
                        ..font::format(font::icon(ui.ctx(), font::ICON))
                    },
                );
                let button = egui::Button::new(text)
                    .stroke(Stroke::NONE)
                    .corner_radius(CornerRadius::same(metric::RADIUS))
                    .min_size(vec2(0.0, metric::NOTICE_CONTROL));
                ui.spacing_mut().button_padding = vec2(metric::NOTICE_CONTROL_PAD, 0.0);
                go = filled_button(ui, palette.btn, palette.btn_hover, button)
                    .on_hover_cursor(egui::CursorIcon::PointingHand)
                    .clicked();
            });
        });
    go
}

/// The header a full-window screen wears: the way back, what this screen is,
/// and nothing else.
///
/// **A screen is not a panel.** The three surfaces behind the overflow are
/// `width:100%; height:100%` on the page colour with a header of their own, so
/// this stands in for the install bar rather than sitting under it - which is why
/// it is two pixels taller than that bar and carries no path, no badge and no
/// view switch.
///
/// `from` is the view the screen was opened out of, and it is on the control
/// rather than beside it: the design labels the way back with where it goes.
///
/// `trailing` is whatever the screen puts at the right end - the Restore
/// screen's `Open backups folder`, and nothing on the other two. A slot rather
/// than a fourth argument saying which control, because what goes there is the
/// screen's business and its width is not known here.
///
/// Answers whether the way back was pressed.
pub fn screen_header(
    ui: &mut Ui,
    palette: Palette,
    from: &str,
    glyph: &str,
    title: &str,
    trailing: impl FnOnce(&mut Ui),
) -> Response {
    let (rect, _) = ui.allocate_exact_size(
        vec2(ui.available_width(), metric::SCREEN_HEADER),
        Sense::hover(),
    );
    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.panel);

    let mut line = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(metric::PAD, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    // The design states every gap along this header, and they are all the same
    // nine. egui's own six between widgets would be a tenth of a bar's width of
    // air nobody asked for, spread over four gaps.
    line.spacing_mut().item_spacing.x = 0.0;

    let back = back_control(&mut line, palette, from);
    line.add_space(metric::SCREEN_HEADER_GAP);
    upright_rule(&mut line, palette);
    line.add_space(metric::SCREEN_HEADER_GAP);
    line.label(font::run(glyph, font::icon(line.ctx(), font::ICON)).color(palette.ink_2));
    line.add_space(metric::SCREEN_HEADER_GAP);
    // The design tracks 13.5px at -0.02em here and at -0.015em in the install
    // bar, and `font::tracking` answers from the face and the size - which are
    // the same in both. The derived -0.015 is what this draws: the difference is
    // 0.0675 of a pixel a character, which is half a pixel across the word
    // `Settings` and under one across the longest title there is. Raised with the
    // designer rather than worked around, because keying tracking on the call
    // site would give every other run in the window a way to lose it.
    line.label(font::run(title, font::emphasis(line.ctx(), font::INSTALL_TITLE)).color(palette.ink));
    // Laid out from the right in what the title left, so a long title gives way
    // to the control rather than pushing it off the header.
    line.with_layout(Layout::right_to_left(Align::Center), trailing);
    back
}

/// The way out of a screen: an arrow, and the name of what it goes back to.
///
/// Allocated and painted rather than built as a `Button`, because the design's
/// padding is not symmetric - six on the left against nine on the right, so that
/// the arrow lines up with the window's own edge padding instead of sitting nine
/// in from it - and `button_padding` is one value for both sides.
fn back_control(ui: &mut Ui, palette: Palette, from: &str) -> Response {
    let text = labelled_icon(
        ui,
        icon::BACK,
        from,
        font::CONTROL,
        palette.ink_2,
        palette.ink_2,
        metric::ALONG_A_BACK,
    );
    let width = metric::BACK_PAD_LEFT
        + font::ICON
        + metric::ALONG_A_BACK
        + text_width(ui, from, font::plain(font::CONTROL))
        + metric::BACK_PAD_RIGHT;
    let (rect, response) =
        ui.allocate_exact_size(vec2(width, metric::BACK), Sense::click());
    if response.hovered() {
        ui.painter().rect_filled(
            rect,
            CornerRadius::same(metric::FIELD_RADIUS),
            palette.btn_hover,
        );
    }
    let mut inside = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(metric::BACK_PAD_LEFT, 0.0)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    inside.label(text);
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A hairline standing up rather than lying down: the one in a screen's header,
/// between where you came from and where you are.
fn upright_rule(ui: &mut Ui, palette: Palette) {
    let (rect, _) =
        ui.allocate_exact_size(vec2(metric::HAIRLINE, metric::SCREEN_RULE), Sense::hover());
    ui.painter().rect_filled(rect, CornerRadius::ZERO, palette.line);
}

/// How wide a screen's column runs, and how much air the design puts over it.
///
/// Two, because the bundle states two and they are not the same: Settings runs
/// 620 wide under 16, About 560 under 20. That is not a rounding - what is in
/// them differs. A column of settings is rows to work down, and a narrower
/// measure would squeeze a path field; About is prose to read, and prose set
/// 620 wide is prose nobody finishes. The design makes the same distinction
/// three times inside an empty state, where each block of words gets a narrower
/// measure than the one above it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Measure {
    /// Settings: rows of controls.
    Controls,
    /// About: prose.
    Prose,
}

impl Measure {
    const fn column(self) -> f32 {
        match self {
            Measure::Controls => metric::SCREEN_COLUMN,
            Measure::Prose => metric::ABOUT_COLUMN,
        }
    }

    const fn top(self) -> f32 {
        match self {
            Measure::Controls => metric::SCREEN_PAD_TOP,
            Measure::Prose => metric::ABOUT_PAD_TOP,
        }
    }
}

/// The scrolling body of a screen, with the padding and the column width the
/// design gives it.
///
/// The column is a `max-width`, so a window wider than the design's leaves the
/// settings where they are instead of stretching a path field across it. The
/// gaps inside are stated one at a time by whatever is drawn in here, as they are
/// in a panel's body and for the same reason.
///
/// **`interact_size.y` is zeroed here, and that is load-bearing.** The theme sets
/// it to a bar control's 26, and a horizontal layout starts its row at that
/// height - so every row in a screen came out at 26 whatever the design said it
/// was, which put four pixels into the Paths group, two into Appearance, two into
/// Diagnostics and ten into the row at the foot, and slid every group below each
/// of them down the window. The same trap [`keyword_box`] pins its way out of, in
/// the one place that can answer it for a whole screen: every control down here
/// states its own height, which is what the design does too.
pub fn screen_body(ui: &mut Ui, measure: Measure, contents: impl FnOnce(&mut Ui)) {
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new()
            .inner_margin(Margin {
                left: metric::SCREEN_PAD_X as i8,
                right: metric::SCREEN_PAD_X as i8,
                top: measure.top() as i8,
                bottom: metric::SCREEN_PAD_BOTTOM as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width().min(measure.column()));
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = 0.0;
                contents(ui);
            });
    });
}

/// How much air a group of settings puts around what is in it.
///
/// Three values because the design states three, and named by what each is for
/// rather than by its number: the box does not vary, and what varies is whether
/// it holds rows of controls, one control that carries its own height, or a list
/// of choices that are each washed and need only a seam of panel showing between
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Padding {
    Rows,
    Switch,
    Choices,
}

impl Padding {
    fn margin(self) -> Margin {
        match self {
            Padding::Rows => {
                Margin::symmetric(metric::GROUP_PAD_X as i8, metric::GROUP_PAD_Y as i8)
            }
            Padding::Switch => {
                Margin::symmetric(metric::GROUP_PAD_X as i8, metric::SWITCH_PAD_Y as i8)
            }
            Padding::Choices => Margin::same(metric::CHOICES_PAD as i8),
        }
    }
}

/// The surface a group of settings is written on.
///
/// The panel colour rather than the field colour, and that distinction is the
/// design's: a field is where something is written and a group is where things
/// live. Rounded by the field's four rather than a control's three, because it is
/// a surface and not something to press - except for the two the design does make
/// pressable, which are pressable by what is drawn in them rather than by being a
/// different shape.
pub fn group_frame(palette: Palette, pad: Padding) -> Frame {
    Frame::new()
        .fill(palette.panel)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(pad.margin())
}

/// One row of a paths group: what the path is for, where it is, and whatever
/// controls the row offers.
///
/// The label column is fixed at the design's 128, so the three paths in Settings
/// start at one x down the group instead of each starting after its own word. The
/// controls are laid out from the right and the path takes what they leave -
/// exactly the order the install bar puts its own path in, and for the same
/// reason: a path is the longest thing on the line and the least urgent, so it is
/// the thing that gives way.
/// `path` is `None` where there is no path to state - a machine with no
/// installation found, or one whose build could not be read. The row is drawn
/// anyway, and says [`NO_PATH`]: a row that vanished would make the Paths group
/// a different height depending on what went wrong, and the bundle has no mockup
/// of either state to say otherwise. See `design-review.md` round 3 item 5.
pub fn path_row(
    ui: &mut Ui,
    palette: Palette,
    label: &str,
    path: Option<&str>,
    controls: impl FnOnce(&mut Ui),
) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = metric::ALONG_A_PATH_ROW;
        ui.allocate_ui_with_layout(
            vec2(metric::PATH_LABEL_COLUMN, ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| {
                // Asked for *and* insisted on. A child `Ui` is allocated at what
                // it came to rather than at what it was offered, so without this
                // the column is as wide as the longest word in it - which is a
                // column that moves when a label is reworded, and the whole point
                // of a fixed one is that the three paths start at one x.
                ui.set_min_width(metric::PATH_LABEL_COLUMN);
                ui.add(
                    egui::Label::new(
                        font::run(label, font::plain(font::CONTROL)).color(palette.ink_2),
                    )
                    .truncate(),
                );
            },
        );
        ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
            controls(ui);
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                path_field(ui, palette, path.unwrap_or(NO_PATH));
            });
        });
    });
}

/// What a path row says where there is nothing to say.
const NO_PATH: &str = "-";

/// A path, in a box, as a value to read rather than a box to type in.
///
/// Not [`one_line_field`]: that one is the inspector's, 27 tall and set in the
/// proportional face because a display name is words. A path is monospaced and
/// the design draws its box four pixels shorter.
fn path_field(ui: &mut Ui, palette: Palette, path: &str) {
    field_frame(palette, Margin::symmetric(metric::PATH_FIELD_PAD_X as i8, 0)).show(ui, |ui| {
        ui.set_width(ui.available_width());
        ui.set_height(metric::PATH_FIELD);
        ui.horizontal_centered(|ui| {
            ui.add(
                egui::Label::new(font::run(path, font::mono(font::MONO)).color(palette.ink))
                    .truncate(),
            )
            .on_hover_text(path);
        });
    });
}

/// How loud a control inside a group is.
///
/// The design writes two: the quiet one beside a value it acts on, and the one
/// that is the point of the group it sits in - which it sets in the primary ink
/// and pads a pixel wider on both sides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Emphasis {
    Quiet,
    Loud,
}

/// A control inside a group: `Browse`, `Restore...`, `Copy report`.
///
/// Not [`small_button`], which is a bar's. The design draws these two pixels
/// shorter around the same sixteen-pixel icon, rounds them by the field's four
/// rather than a control's three, pads them by eight where a bar pads by ten, and
/// sets the gap from icon to label at five where a bar sets six. The same
/// relationship [`empty_action`] has to `small_button`, and for the same reason:
/// a screen's controls are the screen's.
pub fn group_control(
    ui: &mut Ui,
    palette: Palette,
    glyph: &str,
    label: &str,
    emphasis: Emphasis,
) -> Response {
    let (ink, pad) = match emphasis {
        Emphasis::Quiet => (palette.ink_2, metric::GROUP_CONTROL_PAD_X),
        Emphasis::Loud => (palette.ink, metric::WIDE_CONTROL_PAD_X),
    };
    let button = egui::Button::new(labelled_icon(
        ui,
        glyph,
        label,
        font::CHIP,
        ink,
        ink,
        metric::ALONG_A_GROUP_CONTROL,
    ))
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
    .min_size(vec2(0.0, metric::GROUP_CONTROL));
    ui.scope(|ui| {
        // Across the box, so the height is `min_size` alone and the vertical
        // padding must not add to it.
        ui.spacing_mut().button_padding = vec2(pad, 0.0);
        filled_button(ui, palette.btn, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// Put a path back to whatever was auto-detected: the glyph and nothing else.
///
/// The one box in the design that is not square - 26 by 25 - and the one control
/// that says what it does only on hover, which is the design's own `title`
/// attribute rather than an invention here.
pub fn reset_control(ui: &mut Ui, palette: Palette) -> Response {
    let button = egui::Button::new(
        font::run(icon::RESET, font::icon(ui.ctx(), font::ICON)).color(palette.ink_3),
    )
    .stroke(Stroke::NONE)
    .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
    .min_size(vec2(metric::RESET[0], metric::RESET[1]));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = egui::Vec2::ZERO;
        filled_button(ui, palette.btn, palette.btn_hover, button)
            .on_hover_text("Reset to auto-detected")
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// The space a reset control would have taken, where the row has nothing to
/// reset.
///
/// Held rather than dropped, so that the control before it lines up with the
/// controls on the rows above instead of sliding out to the edge of the group.
/// The design holds it the same way, with an empty box of the same width.
pub fn reset_slot(ui: &mut Ui) {
    ui.add_space(metric::RESET[0]);
}

/// One of a set of choices, exactly one of which is in force.
///
/// A mark, the line naming it, and a sentence saying what choosing it means. The
/// whole band is the control, because the sentence is what somebody reads before
/// deciding and the mark is not what they aim at.
///
/// Which mark, which ink and which wash are here rather than at the call site,
/// for the reason this module exists: the design's map from "this is the one in
/// force" to a filled mark, an accent, a warmed sentence and a wash is one map,
/// and a second screen picking its own would drift from it.
pub fn choice(ui: &mut Ui, palette: Palette, chosen: bool, title: &str, note: &str) -> Response {
    let marked = Marked {
        glyph: if chosen { icon::CHOSEN } else { icon::UNCHOSEN },
        mark_ink: if chosen { palette.accent } else { palette.ink_3 },
        title,
        note,
        note_ink: if chosen { palette.ink_3_warm } else { palette.ink_3 },
        wash: if chosen { palette.accent_soft } else { Color32::TRANSPARENT },
        pad: Margin::symmetric(metric::CHOICE_PAD_X as i8, metric::CHOICE_PAD_Y as i8),
        gap: metric::ALONG_A_CHOICE,
    };
    marked_row(ui, palette, &marked)
}

/// Something that is on or off by itself, drawn as a choice without the list
/// around it.
///
/// No wash, because there is nothing for it to be in force *against*, and the
/// sentence is brought up to the secondary ink rather than warmed: the design
/// makes this one louder when it is on because what it turns on deletes a file.
/// The padding is the group's own, since this row *is* the group.
pub fn switched(ui: &mut Ui, palette: Palette, on: bool, title: &str, note: &str) -> Response {
    let marked = Marked {
        glyph: if on { icon::CHECKED } else { icon::UNCHECKED },
        mark_ink: if on { palette.accent } else { palette.ink_3 },
        title,
        note,
        note_ink: if on { palette.ink_2 } else { palette.ink_3 },
        wash: Color32::TRANSPARENT,
        pad: Margin::ZERO,
        gap: metric::ALONG_A_CHECK,
    };
    marked_row(ui, palette, &marked)
}

/// What [`choice`] and [`switched`] both are, once the map from state to colour
/// has been applied.
struct Marked<'a> {
    glyph: &'a str,
    mark_ink: Color32,
    title: &'a str,
    note: &'a str,
    note_ink: Color32,
    wash: Color32,
    pad: Margin,
    gap: f32,
}

/// A mark, a line and a sentence under the line, as one control.
///
/// The mark is nudged one pixel down, which is the design's own `margin-top:1px`:
/// a sixteen-pixel glyph beside an 11.5-pixel line sits above that line
/// otherwise, and the pair reads as two things rather than one.
fn marked_row(ui: &mut Ui, palette: Palette, marked: &Marked<'_>) -> Response {
    let outer = Frame::new()
        .fill(marked.wash)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(marked.pad)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = marked.gap;
                ui.add_space(0.0);
                ui.vertical(|ui| {
                    ui.add_space(metric::MARK_DROP);
                    ui.label(
                        font::run(marked.glyph, font::icon(ui.ctx(), font::ICON))
                            .color(marked.mark_ink),
                    );
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    ui.label(
                        font::run(marked.title, font::plain(font::CONTROL)).color(palette.ink),
                    );
                    ui.add_space(metric::UNDER_A_CHOICE);
                    ui.label(
                        font::wrapping(marked.note, font::NOTE, font::Leading::Explaining)
                            .color(marked.note_ink),
                    );
                });
            });
        })
        .response;
    outer.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// The numbers one of the design's segmented switches is drawn to.
///
/// There are two of them and they are the same control: a well, segments inside
/// it, exactly one held. What differs is four numbers and the weight of the held
/// word, so the shape is written once and each surface states its own - which is
/// what [`segmented`] and [`install_filter`] are, rather than one function with
/// a flag saying which screen called it.
struct Segments {
    /// Whichever of the two surface colours the thing under it is not. The
    /// appearance switch sits on a group and takes the page colour; the install
    /// filter sits on the page and takes the panel's.
    well: Color32,
    /// The well's own height, and a segment's inside it.
    height: f32,
    segment: f32,
    pad_x: f32,
    held: Held,
}

/// How the held segment's word is set.
///
/// The design writes the appearance switch's at the ordinary weight and the
/// catalog's install filter at 500, and that is the only difference between the
/// two switches that is not a number.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Held {
    Plain,
    Heavier,
}

/// A switch between a few states, drawn as one control rather than as a row of
/// buttons: the well behind the segments says they belong together.
///
/// Answers which segment was pressed, if one was.
fn segments<T: Copy + PartialEq>(
    ui: &mut Ui,
    palette: Palette,
    shape: &Segments,
    current: T,
    options: impl Iterator<Item = (T, &'static str, &'static str)>,
) -> Option<T> {
    let mut pressed = None;
    Frame::new()
        .fill(shape.well)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(Margin::same(metric::SEGMENTS_PAD as i8))
        .show(ui, |ui| {
            ui.set_height(shape.height - 2.0 * metric::SEGMENTS_PAD);
            ui.spacing_mut().item_spacing.x = metric::BETWEEN_SEGMENTS;
            ui.spacing_mut().button_padding = vec2(shape.pad_x, 0.0);
            // `min_size` is a floor and not a ceiling, and the theme sets a bar
            // control's 26 here - so a segment asked for 22 was drawn 26 and
            // filled the well it was meant to sit two pixels inside.
            ui.spacing_mut().interact_size.y = shape.segment;
            // Stated, and `horizontal_centered` would not do: it *inherits* the
            // caller's direction, and the order of these is the design's rather
            // than the row's. Laid out inside a right-to-left group - which is
            // how the Local toolbar reaches its own right end - the switch drew
            // `Updatable Installed All`, three answers in the order nobody
            // wrote them in.
            ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                for (value, glyph, label) in options {
                    let on = value == current;
                    let ink = if on { palette.ink } else { palette.ink_3 };
                    let button = egui::Button::new(segment_text(ui, shape, glyph, label, ink, on))
                        .stroke(Stroke::NONE)
                        // The design rounds a segment by three where it rounds
                        // the well around it by four, so the segment sits inside
                        // the corner rather than cutting across it.
                        .corner_radius(CornerRadius::same(metric::RADIUS))
                        .min_size(vec2(0.0, shape.segment));
                    let base = if on { palette.btn } else { Color32::TRANSPARENT };
                    if filled_button(ui, base, palette.btn_hover, button)
                        .on_hover_cursor(egui::CursorIcon::PointingHand)
                        .clicked()
                    {
                        pressed = Some(value);
                    }
                }
            });
        });
    pressed
}

/// What one segment says, which is a glyph and a word or a word on its own.
///
/// Not [`labelled_icon`]: that puts the design's gap in front of the label
/// unconditionally, and a segment with no glyph would then draw five pixels
/// indented from its own padding. And the held word may be set heavier, which is
/// a choice about the label alone.
fn segment_text(
    ui: &Ui,
    shape: &Segments,
    glyph: &str,
    label: &str,
    ink: Color32,
    on: bool,
) -> egui::WidgetText {
    let mut job = egui::text::LayoutJob::default();
    if !glyph.is_empty() {
        job.append(
            glyph,
            0.0,
            egui::TextFormat {
                color: ink,
                valign: Align::Center,
                ..font::format(font::icon(ui.ctx(), font::ICON))
            },
        );
    }
    let face = if on && shape.held == Held::Heavier {
        font::emphasis(ui.ctx(), font::CHIP)
    } else {
        font::plain(font::CHIP)
    };
    job.append(
        label,
        if glyph.is_empty() { 0.0 } else { metric::ALONG_A_SEGMENT },
        egui::TextFormat { color: ink, valign: Align::Center, ..font::format(face) },
    );
    job.into()
}

/// The appearance switch on the Settings screen - `SettingsScreen.dc.html:133`.
pub fn segmented<T: Copy + PartialEq>(
    ui: &mut Ui,
    palette: Palette,
    current: T,
    options: &[(T, &'static str, &'static str)],
) -> Option<T> {
    let shape = Segments {
        well: palette.bg,
        height: metric::SEGMENTS,
        segment: metric::SEGMENT,
        pad_x: metric::SEGMENT_PAD_X,
        held: Held::Plain,
    };
    segments(ui, palette, &shape, current, options.iter().copied())
}

/// The catalog toolbar's install filter - `CatalogToolbar.dc.html:33-37`.
///
/// No glyphs, because these are not three things to do: they are one question
/// about the list, asked three ways, and a word is the whole of each answer.
/// The well takes the panel colour rather than the page's, which is the switch
/// on Settings the other way up - a control reads as one control by standing
/// clear of what is behind it, and what is behind it here is the bar.
pub fn install_filter<T: Copy + PartialEq>(
    ui: &mut Ui,
    palette: Palette,
    current: T,
    options: &[(T, &'static str)],
) -> Option<T> {
    let shape = Segments {
        well: palette.panel,
        height: metric::CONTROL,
        segment: metric::INSTALL_FILTER_SEGMENT,
        pad_x: metric::INSTALL_FILTER_PAD_X,
        held: Held::Heavier,
    };
    segments(ui, palette, &shape, current, options.iter().map(|(v, l)| (*v, "", *l)))
}

/// The diagnostics block: a column of monospaced lines, meant to be copied
/// whole.
///
/// Not wrapped, which is the design's `white-space:pre`. Every line is a label
/// and a value with the labels aligned by spaces, so a wrap would put a value
/// under a label it does not belong to - and the point of this block is that it
/// can be pasted into a bug report and read by somebody else.
pub fn report(ui: &mut Ui, palette: Palette, text: &str) {
    field_frame(palette, Margin::symmetric(metric::REPORT_PAD_X as i8, metric::REPORT_PAD_Y as i8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                egui::Label::new(
                    font::run(text, font::mono(font::MONO_TIGHT))
                        .color(palette.ink_2)
                        .line_height(Some(font::Leading::Reporting.over(font::MONO_TIGHT))),
                )
                .wrap_mode(egui::TextWrapMode::Extend),
            );
        });
}

/// The row at the foot of a screen that leads to another one: an icon, a line, a
/// value at the right end, and the mark that says there is more.
///
/// The whole row is the control, and it has no heading over it - the design draws
/// it as a group-shaped thing that is not a group, which is what says it is the
/// last item rather than another setting.
pub fn screen_link(
    ui: &mut Ui,
    palette: Palette,
    glyph: &str,
    label: &str,
    value: &str,
) -> Response {
    let row = group_frame(palette, Padding::Switch)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.x = metric::ALONG_A_SCREEN_LINK;
            ui.horizontal(|ui| {
                ui.set_height(font::ICON);
                ui.label(font::run(glyph, font::icon(ui.ctx(), font::ICON)).color(palette.ink_2));
                // The mark first, from the right, so the line gives way to it.
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        font::run(icon::ONWARD, font::icon(ui.ctx(), font::ICON))
                            .color(palette.ink_3),
                    );
                    ui.label(font::run(value, font::mono(font::MONO)).color(palette.ink_3));
                    ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
                        ui.add(
                            egui::Label::new(
                                font::run(label, font::plain(font::CONTROL)).color(palette.ink),
                            )
                            .truncate(),
                        );
                    });
                });
            });
        })
        .response;
    row.interact(Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// A notice on a screen: an icon, and what it is about beside it, on a box of
/// its own.
///
/// One component for both screens' notices, because the bundle draws one box:
/// the same padding, the same gap from glyph to words, the same radius. What
/// differs is the tone and whether there is a line that has to be read even if
/// the rest is not.
pub struct Notice<'a> {
    pub tone: Tone,
    pub glyph: &'a str,
    /// Absent where the notice is a single paragraph, which is About's.
    pub headline: Option<&'a str>,
    pub body: &'a str,
    /// How the paragraph is set. Not derivable from the tone: the design sets
    /// the Restore warning on a notice's leading and About's trademark line on
    /// prose's, and both are 10.5.
    pub leading: font::Leading,
}

pub fn notice(ui: &mut Ui, palette: Palette, notice: &Notice<'_>) {
    Frame::new()
        .fill(notice.tone.surface(palette))
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .inner_margin(Margin::symmetric(
            metric::SCREEN_NOTICE_PAD_X as i8,
            metric::SCREEN_NOTICE_PAD_Y as i8,
        ))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.horizontal_top(|ui| {
                ui.spacing_mut().item_spacing.x = metric::ALONG_A_SCREEN_NOTICE;
                // Nudged down for the reason [`marked_row`] nudges a mark: a
                // 16-pixel glyph beside a 10.5 line sits above it otherwise.
                ui.vertical(|ui| {
                    ui.add_space(metric::MARK_DROP);
                    ui.label(
                        font::run(notice.glyph, font::icon(ui.ctx(), font::ICON))
                            .color(notice.tone.mark(palette)),
                    );
                });
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    if let Some(headline) = notice.headline {
                        ui.label(
                            font::run(headline, font::emphasis(ui.ctx(), font::CONTROL))
                                .color(palette.ink),
                        );
                        ui.add_space(metric::UNDER_A_NOTICE_HEADLINE);
                    }
                    ui.label(
                        font::wrapping(notice.body, font::NOTE, notice.leading)
                            .color(notice.tone.supporting(palette)),
                    );
                });
            });
        });
}

/// The Restore screen's body: a warning that stays put, and a list that scrolls
/// under it.
///
/// Not [`screen_body`]. That one is a column of a stated measure under a stated
/// amount of air, which is what a screen of settings or of prose is; this is a
/// list that runs the width of the window with a fixed block above it, and the
/// design gives it its own three paddings. The warning is outside the scroll
/// because what it says is true of the press at the foot, and a warning that
/// scrolled away would be a warning the user could press without.
pub fn restore_body(ui: &mut Ui, warning: impl FnOnce(&mut Ui), list: impl FnOnce(&mut Ui)) {
    // Before the first allocation, not inside the closures: egui's six pixels
    // otherwise land between the warning and the list on top of the design's.
    ui.spacing_mut().item_spacing.y = 0.0;
    ui.spacing_mut().interact_size.y = 0.0;
    Frame::new()
        .inner_margin(Margin {
            left: metric::PAD as i8,
            right: metric::PAD as i8,
            top: metric::PAD as i8,
            bottom: 0,
        })
        .show(ui, warning);
    egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
        Frame::new()
            .inner_margin(Margin {
                left: metric::PAD as i8,
                right: metric::PAD as i8,
                top: metric::ABOVE_A_BACKUP_LIST as i8,
                bottom: metric::UNDER_A_BACKUP_LIST as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.spacing_mut().interact_size.y = 0.0;
                list(ui);
            });
    });
}

/// The list of backups: rows on one rounded panel, with a seam of it showing
/// between them.
///
/// The panel is what the rows are written on rather than what each row carries,
/// which is the design's `background` on the column and a one-pixel `gap`
/// between its children. A row's own wash goes over that, so the seam is the
/// panel and not a line drawn on it.
pub fn backup_list(ui: &mut Ui, palette: Palette, rows: impl FnOnce(&mut Ui)) {
    Frame::new()
        .fill(palette.panel)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.spacing_mut().item_spacing.y = metric::BETWEEN_BACKUPS;
            rows(ui);
        });
}

/// One backup, as the Restore screen states it.
pub struct Taken<'a> {
    /// The day and time the copy was taken, formatted where it was read - see
    /// [`crate::restore`]. Never formatted here: a timestamp turned into words
    /// at the point of drawing is turned into the drawing machine's words.
    pub when: &'a str,
    /// Which build it is of, and what it holds.
    pub what: &'a str,
    pub size: &'a str,
    /// The newest, which the design marks.
    pub latest: bool,
    pub chosen: bool,
    /// Where in the list it sits. Decides the zebra, and which of its corners
    /// are rounded - a row's own wash is drawn over the panel, so a square first
    /// row would fill in the corners the list rounds.
    pub at: usize,
    pub of: usize,
}

/// Draw one, and answer whether it was chosen.
pub fn backup_row(ui: &mut Ui, palette: Palette, taken: &Taken<'_>) -> Response {
    let (rect, response) = ui.allocate_exact_size(
        vec2(ui.available_width(), metric::BACKUP_ROW),
        Sense::click(),
    );
    let wash = if taken.chosen {
        palette.accent_soft
    } else if taken.at.is_multiple_of(2) {
        palette.zebra
    } else {
        Color32::TRANSPARENT
    };
    ui.painter().rect_filled(rect, ends(taken.at, taken.of), wash);

    let mut line = ui.new_child(
        egui::UiBuilder::new()
            .max_rect(rect.shrink2(vec2(metric::BACKUP_ROW_PAD_X, metric::BACKUP_ROW_PAD_Y)))
            .layout(Layout::left_to_right(Align::Center)),
    );
    line.spacing_mut().item_spacing.x = 0.0;
    let mark = if taken.chosen { icon::CHOSEN } else { icon::UNCHOSEN };
    let mark_ink = if taken.chosen { palette.accent } else { palette.ink_3 };
    line.label(font::run(mark, font::icon(line.ctx(), font::ICON)).color(mark_ink));
    line.add_space(metric::ALONG_A_BACKUP_ROW);

    // The size first, from the right, so the line that can run long is the one
    // that gives way - the same order the install bar puts its path in.
    let supporting = if taken.chosen { palette.ink_3_warm } else { palette.ink_3 };
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        ui.label(font::run(taken.size, font::mono(font::MONO)).color(supporting));
        ui.add_space(metric::ALONG_A_BACKUP_ROW);
        ui.with_layout(Layout::left_to_right(Align::Center), |ui| {
            ui.vertical(|ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = metric::BESIDE_A_BACKUP_DATE;
                    ui.label(
                        font::run(taken.when, font::plain(font::FIELD_VALUE))
                            .color(palette.ink),
                    );
                    if taken.latest {
                        latest_mark(ui, palette);
                    }
                });
                ui.add_space(metric::UNDER_A_BACKUP_DATE);
                ui.add(
                    egui::Label::new(
                        font::run(taken.what, font::mono(font::MONO_TIGHT)).color(supporting),
                    )
                    .truncate(),
                );
            });
        });
    });
    response.on_hover_cursor(egui::CursorIcon::PointingHand)
}

/// Which corners of a row inside a rounded list are rounded: the outer ones of
/// the first and the last, and none of the rest.
fn ends(at: usize, of: usize) -> CornerRadius {
    let r = metric::FIELD_RADIUS;
    let first = at == 0;
    let last = at + 1 == of;
    CornerRadius {
        nw: if first { r } else { 0 },
        ne: if first { r } else { 0 },
        sw: if last { r } else { 0 },
        se: if last { r } else { 0 },
    }
}

/// The mark on the newest backup.
fn latest_mark(ui: &mut Ui, palette: Palette) {
    Frame::new()
        .fill(palette.accent_soft)
        .corner_radius(CornerRadius::same(metric::RADIUS))
        .inner_margin(Margin::symmetric(metric::LATEST_PAD_X as i8, metric::LATEST_PAD_Y as i8))
        .show(ui, |ui| {
            ui.label(
                font::run(LATEST, font::emphasis(ui.ctx(), font::FOOTNOTE))
                    .color(palette.accent_text),
            );
        });
}

const LATEST: &str = "Latest";

/// Which foot a way out stands on.
///
/// The design draws the control twice and not the same way: a screen's is 29
/// tall, padded 12 and rounded as a field is, and a dialog's is 30, padded 13
/// and rounded as a control is - `RestoreScreen.dc.html:79` against
/// `ORNG Registry.dc.html:246`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Foot {
    Screen,
    Dialog,
}

/// The way out of a decision, beside the press that makes it.
pub fn cancel_button(ui: &mut Ui, palette: Palette, label: &str, foot: Foot) -> Response {
    let (height, pad_x, radius) = match foot {
        Foot::Screen => (metric::CANCEL, metric::CANCEL_PAD_X, metric::FIELD_RADIUS),
        Foot::Dialog => (metric::DIALOG_CANCEL, metric::DIALOG_CANCEL_PAD_X, metric::RADIUS),
    };
    let button = egui::Button::new(font::run(label, font::plain(font::CONTROL)).color(palette.ink_2))
        .stroke(Stroke::NONE)
        .corner_radius(CornerRadius::same(radius))
        .min_size(vec2(0.0, height));
    ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(pad_x, 0.0);
        filled_button(ui, palette.btn, palette.btn_hover, button)
            .on_hover_cursor(egui::CursorIcon::PointingHand)
    })
    .inner
}

/// A screen's primary action, where what it does cannot be taken back.
///
/// The one control in the window the design outlines in the error colour instead
/// of filling with the accent. That is not decoration: the accent is what the
/// window offers, and this is what it will do to an installation that is already
/// working. The icon leads the words here where the action bar's follows them -
/// an arrow says only that something will happen, and this glyph names what.
///
/// Disabled, it is the design's own disabled primary: the button fill and the
/// tertiary ink, with the reason on it rather than left to be guessed at.
pub fn screen_primary(
    ui: &mut Ui,
    palette: Palette,
    glyph: &str,
    label: &str,
    enabled: bool,
    reason: &str,
) -> Response {
    let ink = if enabled { palette.err } else { palette.ink_3 };
    let text = labelled_icon(
        ui,
        glyph,
        label,
        font::ACTION,
        ink,
        ink,
        metric::ALONG_A_SCREEN_PRIMARY,
    );
    let (fill, stroke) = if enabled {
        (Color32::TRANSPARENT, Stroke::new(metric::HAIRLINE, ink))
    } else {
        (palette.btn, Stroke::NONE)
    };
    let button = egui::Button::new(text)
        .fill(fill)
        .stroke(stroke)
        .corner_radius(CornerRadius::same(metric::FIELD_RADIUS))
        .min_size(vec2(0.0, metric::SCREEN_PRIMARY));
    let response = ui.scope(|ui| {
        ui.spacing_mut().button_padding = vec2(metric::SCREEN_PRIMARY_PAD_X, 0.0);
        ui.add_enabled(enabled, button)
    })
    .inner;
    if enabled {
        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        response.on_disabled_hover_text(reason)
    }
}

/// The block About opens with: this application's mark, its name, and the line
/// saying which build it is and what it is running on.
pub fn identity(ui: &mut Ui, palette: Palette, name: &str, build: &str) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = metric::ALONG_AN_IDENTITY;
        let (tile, _) = ui.allocate_exact_size(
            vec2(metric::PRODUCT_MARK, metric::PRODUCT_MARK),
            Sense::hover(),
        );
        ui.painter().rect_filled(
            tile,
            CornerRadius::same(metric::PRODUCT_MARK_RADIUS),
            palette.accent,
        );
        // Painted rather than laid out: the glyph is centred in both directions
        // inside a box of a stated size, which is one call here and a nest of
        // centring layouts otherwise.
        ui.painter().text(
            tile.center(),
            egui::Align2::CENTER_CENTER,
            icon::PRODUCT,
            font::icon(ui.ctx(), font::PRODUCT_ICON),
            palette.accent_ink,
        );
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            ui.label(font::run(name, font::emphasis(ui.ctx(), font::PRODUCT)).color(palette.ink));
            ui.add_space(metric::UNDER_A_PRODUCT_NAME);
            ui.label(font::run(build, font::mono(font::MONO_FACT)).color(palette.ink_3));
        });
    });
}

/// What one of About's fact rows states.
///
/// An enum rather than a string and a colour, because the two rows differ in
/// what they *are* and not in how they are drawn: two are values read off the
/// installation and the third is an answer to a question. The design says so -
/// it sets the values in the monospaced face and leaves them the primary ink,
/// and sets the answer in the proportional one and colours it.
pub enum Fact<'a> {
    /// Something read off the installation, in the face a number gets: this is
    /// what somebody copies into a bug report.
    Read(&'a str),
    /// Whether this build's anchors were located.
    Resolution(bool),
}

/// One line of what About says about the installation it found.
///
/// The label column is fixed at the design's 96, so the three values start at
/// one x rather than each after its own word - the same reason [`path_row`]
/// fixes its own.
pub fn fact_row(ui: &mut Ui, palette: Palette, label: &str, fact: &Fact<'_>) {
    ui.horizontal(|ui| {
        ui.spacing_mut().item_spacing.x = metric::ALONG_A_FACT_ROW;
        ui.allocate_ui_with_layout(
            vec2(metric::FACT_LABEL_COLUMN, ui.available_height()),
            Layout::left_to_right(Align::Center),
            |ui| {
                ui.set_min_width(metric::FACT_LABEL_COLUMN);
                ui.label(font::run(label, font::plain(font::CHIP)).color(palette.ink_3));
            },
        );
        match fact {
            Fact::Read(value) => ui.label(
                font::run(*value, font::mono(font::MONO_FACT)).color(palette.ink),
            ),
            Fact::Resolution(resolved) => {
                let (text, ink) = if *resolved {
                    (ANCHORS_FOUND, palette.ink)
                } else {
                    (ANCHORS_MISSING, palette.err_text)
                };
                ui.label(font::run(text, font::plain(font::CHIP)).color(ink))
            }
        };
    });
}

/// What About says about whether this build could be read. The design's own two
/// lines, and the only place in the window that states it in words rather than
/// as a badge.
const ANCHORS_FOUND: &str = "All anchors located";
const ANCHORS_MISSING: &str = "Anchors not located";

/// What a value that could not be read says, which both of About's read rows can
/// need: an installation may be found and still not state its build.
pub const UNREAD: &str = "-";

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

    /// The glyph's own colour, which is not the headline's. The design writes a
    /// warning's icon in `--warn` and its headline in `--accent-text`: one is a
    /// shape and reads at full strength, the other is a line of type and would
    /// glare.
    fn mark(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.ink_3,
            Tone::Ok => palette.ok,
            Tone::Warn => palette.warn,
            Tone::Err => palette.err,
        }
    }

    /// The box a notice on a screen is written on.
    ///
    /// Neutral is the panel and not [`Tone::wash`]'s tint, which is the design's
    /// own distinction: a notice with nothing wrong in it is a statement on a
    /// surface, and tinting it would make it read as a fourth condition beside
    /// the three that mean something.
    fn surface(self, palette: Palette) -> Color32 {
        match self {
            Tone::Neutral => palette.panel,
            toned => toned.wash(palette),
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
    /// A second, named way out, drawn beside the action.
    ///
    /// For the one banner that is a question rather than a statement: where the
    /// press does something the user cannot undo, refusing has to be a word
    /// they can read and not the dismiss mark, which says "put this away" and
    /// not "do not do it".
    pub cancel: Option<&'a str>,
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
    /// The named refusal, which is not [`Answered::Dismissed`]: one says the
    /// press must not happen and the other says the words have been read.
    Cancelled,
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
                // **The controls first, from the right**, so the two lines wrap
                // in what is left of the bar rather than under them. The same
                // order `panel_header` puts its close control in, and for the
                // same reason: a block that takes the whole width first leaves
                // nothing for the control to be laid out in, so the control is
                // drawn over the text instead of beside it. Only a body long
                // enough to reach that far shows it, which is why this stood.
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
                    // After the action in this right-to-left layout, so it sits
                    // to its left: the press that does the thing is the one at
                    // the end, as it is in the confirmation dialog.
                    if let Some(cancel) = banner.cancel {
                        ui.add_space(metric::TOOL_GAP);
                        if outlined_button(ui, palette.ink_2, cancel).clicked() {
                            pressed = Answered::Cancelled;
                        }
                    }
                    ui.with_layout(Layout::top_down(Align::Min), |ui| {
                        ui.spacing_mut().item_spacing.y = BETWEEN_THE_LINES;
                        ui.label(
                            font::run(banner.title, font::emphasis(ui.ctx(), font::CONTROL))
                                .color(banner.tone.colour(palette)),
                        );
                        ui.label(
                            font::run(banner.body, font::plain(font::NOTE))
                                .color(banner.tone.supporting(palette)),
                        );
                    });
                });
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
        font::run(icon::DISMISS, font::icon(ui.ctx(), font::ICON)).color(palette.ink_3),
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
        font::run(label, font::emphasis(ui.ctx(), font::CONTROL)).color(colour),
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
    /// The four corner marks. Its own flag rather than something derived from
    /// [`Empty::minor`], because the bundle makes it a prop of its own and the
    /// Restore screen is what proves the two apart: full-sized, and drawn
    /// without them, because it is already inside a surface with a header and a
    /// bar at the foot saying what region this is.
    pub marks: bool,
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
    if empty.marks {
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
        ui.label(font::run(empty.icon, font::icon(ui.ctx(), icon_size)).color(icon_ink));
        ui.add_space(stack::AFTER_ICON);
        ui.label(
            font::run(empty.title, font::emphasis(ui.ctx(), title_size))
                .color(palette.ink),
        );
        ui.add_space(stack::AFTER_TITLE);
        ui.allocate_ui_with_layout(
            vec2(BODY_WIDTH, 0.0),
            Layout::top_down(Align::Center),
            |ui| {
                ui.spacing_mut().item_spacing.y = 0.0;
                // On the design's own leading. `EmptyState.dc.html:34` states
                // `line-height:1.6` here and this was drawn on egui's default,
                // which is the fault `040128e` took out of the catalog detail's
                // two paragraphs: wrong per line, so invisible at one and wrong
                // by a line at five. Every one of these bodies wraps.
                ui.label(
                    font::wrapping(empty.body, font::CONTROL, font::Leading::Introducing)
                        .color(palette.ink_2),
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
                                font::wrapping(aside, font::CHIP, font::Leading::Describing)
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
                        font::wrapping(foot, font::NOTE, font::Leading::Describing)
                            .color(palette.ink_3),
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
    let mono = || egui::TextFormat { color: palette.ink_3, ..font::format(font::mono(font::MONO)) };
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

/// A path as the window draws it: the design's separators rather than the
/// platform's.
///
/// `Path::display` prints back whichever separator the platform uses, so any
/// path that was `join`ed reads `~/.orng\entries.tsv` on Windows. That is the
/// fault `render::fixture` already records one layer out - `join` put a
/// backslash into the install bar's own path and seventeen snapshots failed on
/// Windows and nowhere else, by the pixels two glyphs cost. The three Settings
/// snapshots failed the same way for the same reason: that screen is the only
/// one drawing paths this application builds rather than paths a fixture spells
/// out, and it draws seven of them.
///
/// The drawn form is already not the platform's. `diagnostics::shortened` writes
/// `~/`, which is not how Windows spells a home directory at all, and the bundle
/// draws forward slashes in every path it draws. One spelling is also what makes
/// two diagnostics reports comparable, which is the whole reason that block
/// exists.
///
/// Substituted only where the platform's separator *is* the backslash, so that a
/// Unix file whose *name* contains one still draws its own name.
pub fn drawn_path(path: &std::path::Path) -> String {
    let text = path.display().to_string();
    if cfg!(windows) { with_drawn_separators(&text) } else { text }
}

/// The substitution apart from the platform, so that what it does can be
/// asserted on any machine rather than only on the one it is for.
fn with_drawn_separators(text: &str) -> String {
    text.replace('\\', "/")
}

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

/// What a preparation asks before it starts: the plan, and the pair of
/// controls that answer it.
///
/// The one press in the window that confirms, and the design says why in the
/// sentence it leads with - it is the one operation that modifies Bitwig Studio
/// itself. Every word here is the caller's, as [`Progress`]'s are: this draws a
/// plan and does not compose one.
pub struct Confirmation<'a> {
    pub title: &'a str,
    /// The small word at the right of the heading, saying what the body is.
    pub tag: &'a str,
    pub lead: &'a str,
    pub plan: &'a [PlanLine],
    /// What is true afterwards, under the plan.
    pub note: &'a str,
    pub cancel: &'a str,
    pub primary: &'a str,
    /// The glyph after the primary's words.
    pub icon: &'a str,
    /// Whether the primary asks the system for administrator rights, and so
    /// wears the shield.
    pub elevates: bool,
}

/// One numbered line of a plan.
pub struct PlanLine {
    pub text: String,
    /// Whether this is the line that changes the installation. The design
    /// numbers that one in the accent and every other in the tertiary ink, so
    /// that a reader skimming the list finds the line that matters.
    pub modifies_the_installation: bool,
}

/// What a confirmation was answered with, on the frame it was answered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Cancel,
    Proceed,
}

/// How wide each dialog is, and how their corners are cut. The design gives
/// them a softer corner than a control: they are surfaces, not buttons.
const PROGRESS_WIDTH: f32 = 436.0;
const CONFIRMATION_WIDTH: f32 = 476.0;
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
/// The plan, `ORNG Registry.dc.html:231-241`: a pixel more over it than over
/// the step list, eleven between the sentence, the list and the note, each line
/// padded five above and below, and its number a pixel down from its words and
/// ten before them.
const ABOVE_A_PLAN: f32 = 13.0;
const BETWEEN_PLAN_BLOCKS: f32 = 11.0;
const PLAN_LINE_PAD: f32 = 5.0;
const ALONG_A_PLAN_LINE: f32 = 10.0;
const UNDER_A_PLAN_NUMBER: f32 = 1.0;
/// Between the two controls at a dialog's foot.
const ALONG_A_DIALOG_FOOT: f32 = 8.0;

/// A dialog over the window, holding it still.
///
/// Drawn twice, like every block the design centres: once into a sizing pass
/// to learn how tall it is, then for real in the middle of the window. The
/// contents are the caller's; the scrim, the surface and the centring are the
/// same for every dialog, and are here so that no dialog can be centred in the
/// working area by mistake - it is about the window, and the bars are behind
/// the same scrim.
fn dialog<R>(
    ui: &mut Ui,
    palette: Palette,
    name: &str,
    width: f32,
    contents: impl Fn(&mut Ui) -> R,
) -> R {
    // Over everything, bars included: the window is holding still, and a scrim
    // that stopped at the working area would say that the bars are not.
    let window = ui.ctx().viewport_rect();
    let layer = egui::LayerId::new(egui::Order::Foreground, egui::Id::new(name));
    let painter = ui.ctx().layer_painter(layer);
    painter.rect_filled(window, CornerRadius::ZERO, palette.scrim);

    // The scrim takes the pointer and the keyboard as well as the light.
    // Everything behind it is still drawn - the work is being done to that
    // list - but a control under a scrim that still answered would be a window
    // saying one thing and doing another. Two things do it, and egui's own
    // `Modal` needs the same two. Hit testing goes by layer and consults no
    // modal, so a rect on this one absorbs every click that would otherwise
    // reach the bars. Keyboard focus goes by the modal layer and consults no
    // rect: without it, Tab walked the bar under the scrim and Enter pressed
    // what it found there, which opened Settings behind a dialog saying the
    // window was holding still.
    let mut sink = Ui::new(
        ui.ctx().clone(),
        egui::Id::new(name).with("scrim"),
        egui::UiBuilder::new().layer_id(layer).max_rect(window),
    );
    // The two flags and not `click_and_drag()`, which is also focusable: the
    // scrim is not a control, and Tab must not stop on it.
    sink.allocate_rect(window, Sense::CLICK | Sense::DRAG);
    ui.ctx().memory_mut(|memory| memory.set_modal_layer(layer));

    let contents = |ui: &mut Ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        // Nothing in a dialog is a bar control, and the theme's floor under a
        // row would put nine pixels into its heading and ten into every line
        // of a plan. Rows that have a height say so.
        ui.spacing_mut().interact_size.y = 0.0;
        contents(ui)
    };

    let mut probe = Ui::new(
        ui.ctx().clone(),
        egui::Id::new(name).with("measure"),
        egui::UiBuilder::new().sizing_pass().invisible().max_rect(Rect::from_min_size(
            window.min,
            vec2(width, window.height()),
        )),
    );
    // egui lays a sizing pass out with every widget in it live, so the
    // controls it measures are ghosts that could be pressed or focused. They
    // are not, because the pass is on the background layer and the modal layer
    // above puts it out of reach of both - which the confirmation's own test
    // holds, by counting the stops Tab makes.
    contents(&mut probe);

    let rect = Rect::from_center_size(window.center(), vec2(width, probe.min_rect().height()));
    painter.rect_filled(rect, CornerRadius::same(DIALOG_RADIUS), palette.panel_2);
    let mut dialog = Ui::new(
        ui.ctx().clone(),
        egui::Id::new(name).with("contents"),
        egui::UiBuilder::new().layer_id(layer).max_rect(rect),
    );
    contents(&mut dialog)
}

/// Which edge of the dialog a band meets, and so which of its corners are cut.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Edge {
    Top,
    Bottom,
}

/// A dialog's heading, or its foot: the page colour across the dialog, rounded
/// on the side that meets its edge and square where it meets the body.
fn dialog_band<R>(
    ui: &mut Ui,
    palette: Palette,
    edge: Edge,
    margin: Margin,
    contents: impl FnOnce(&mut Ui) -> R,
) -> R {
    let top = edge == Edge::Top;
    let rounded = CornerRadius {
        nw: if top { DIALOG_RADIUS } else { 0 },
        ne: if top { DIALOG_RADIUS } else { 0 },
        sw: if top { 0 } else { DIALOG_RADIUS },
        se: if top { 0 } else { DIALOG_RADIUS },
    };
    Frame::new()
        .fill(palette.bg)
        .corner_radius(rounded)
        .inner_margin(margin)
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            contents(ui)
        })
        .inner
}

/// The band over a dialog's body: the design's 14 above a heading and 12 under
/// it.
fn heading_margin() -> Margin {
    Margin {
        left: DIALOG_PAD as i8,
        right: DIALOG_PAD as i8,
        top: DIALOG_PAD as i8,
        bottom: UNDER_A_HEADING as i8,
    }
}

pub fn progress_dialog(ui: &mut Ui, palette: Palette, progress: &Progress<'_>) {
    dialog(ui, palette, "progress-dialog", PROGRESS_WIDTH, |ui| {
        dialog_band(ui, palette, Edge::Top, heading_margin(), |ui| {
            ui.label(
                font::run(progress.title, font::emphasis(ui.ctx(), font::DIALOG_TITLE))
                    .color(palette.ink),
            );
            ui.add_space(UNDER_A_TITLE);
            ui.label(font::run(progress.step, font::plain(font::NOTE)).color(palette.ink_3));
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
                    font::run(progress.note, font::plain(font::NOTE)).color(palette.ink_3),
                );
            });

        let foot = Margin::symmetric(DIALOG_PAD as i8, UNDER_A_HEADING as i8);
        dialog_band(ui, palette, Edge::Bottom, foot, |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().item_spacing.x = 0.0;
                let percent = format!("{}%", (progress.through * 100.0).round());
                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                    ui.label(
                        font::run(percent, font::mono(font::MONO_TIGHT)).color(palette.ink_3),
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
    });
}

/// The plan a preparation confirms with, over the window, and what it was
/// answered with.
///
/// `ORNG Registry.dc.html:223-252`. The heading carries the title and, at its
/// right, the word for what the body is; the body is the sentence that says why
/// this confirms, the numbered plan, and the note under it; the foot is the way
/// out and the press, the press at the right. The primary is the action bar's
/// own control - the design draws it the same 32 tall with the same words and
/// the same arrow, one pixel of accent outline apart - so a reader sees the
/// press they made a moment ago, asked again.
pub fn confirmation_dialog(
    ui: &mut Ui,
    palette: Palette,
    confirmation: &Confirmation<'_>,
) -> Option<Answer> {
    dialog(ui, palette, "confirmation-dialog", CONFIRMATION_WIDTH, |ui| {
        dialog_band(ui, palette, Edge::Top, heading_margin(), |ui| {
            // One row, as tall as the title, with the tag centred on it.
            let title = font::emphasis(ui.ctx(), font::DIALOG_TITLE);
            let height = ui.fonts_mut(|fonts| fonts.row_height(&title));
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), height),
                Layout::left_to_right(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.label(font::run(confirmation.title, title).color(palette.ink));
                    ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                        ui.label(
                            font::run(confirmation.tag, font::plain(font::NOTE))
                                .color(palette.ink_3),
                        );
                    });
                },
            );
        });

        Frame::new()
            .inner_margin(Margin {
                left: DIALOG_PAD as i8,
                right: DIALOG_PAD as i8,
                top: ABOVE_A_PLAN as i8,
                bottom: DIALOG_PAD as i8,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    font::wrapping(confirmation.lead, font::CONTROL, font::Leading::Describing)
                        .color(palette.ink_2),
                );
                ui.add_space(BETWEEN_PLAN_BLOCKS);
                for (at, line) in confirmation.plan.iter().enumerate() {
                    plan_line(ui, palette, at, line);
                }
                ui.add_space(BETWEEN_PLAN_BLOCKS);
                ui.label(
                    font::wrapping(confirmation.note, font::NOTE, font::Leading::Planning)
                        .color(palette.ink_3),
                );
            });

        let foot = Margin::symmetric(DIALOG_PAD as i8, UNDER_A_HEADING as i8);
        dialog_band(ui, palette, Edge::Bottom, foot, |ui| {
            // As tall as the press, so the way out beside it is centred on it
            // rather than hung from the top of a row the theme sized.
            ui.allocate_ui_with_layout(
                vec2(ui.available_width(), metric::ACTION),
                Layout::right_to_left(Align::Center),
                |ui| {
                    ui.spacing_mut().item_spacing.x = 0.0;
                    let (primary, icon) = (confirmation.primary, confirmation.icon);
                    let pressed = if confirmation.elevates {
                        elevating_button(ui, palette, primary, icon)
                    } else {
                        primary_button(ui, palette, primary, icon, true, "")
                    };
                    ui.add_space(ALONG_A_DIALOG_FOOT);
                    let way_out = cancel_button(ui, palette, confirmation.cancel, Foot::Dialog);
                    if pressed.clicked() {
                        Some(Answer::Proceed)
                    } else if way_out.clicked() {
                        Some(Answer::Cancel)
                    } else {
                        None
                    }
                },
            )
            .inner
        })
    })
}

/// One line of the plan: its number, and its words wrapped beside it.
///
/// The number is a pixel lower than the words and set in the tertiary ink, or
/// in the accent for the one line that changes the installation. A line that
/// wraps keeps its number at the top, as the design has it.
fn plan_line(ui: &mut Ui, palette: Palette, at: usize, line: &PlanLine) {
    let number_ink =
        if line.modifies_the_installation { palette.accent_text } else { palette.ink_3 };
    ui.add_space(PLAN_LINE_PAD);
    ui.with_layout(Layout::left_to_right(Align::Min), |ui| {
        ui.spacing_mut().item_spacing.x = 0.0;
        ui.vertical(|ui| {
            ui.add_space(UNDER_A_PLAN_NUMBER);
            ui.label(
                font::run((at + 1).to_string(), font::mono(font::MONO_TIGHT)).color(number_ink),
            );
        });
        ui.add_space(ALONG_A_PLAN_LINE);
        ui.add(
            egui::Label::new(
                font::wrapping(line.text.as_str(), font::CHIP, font::Leading::Planning)
                    .color(palette.ink),
            )
            .wrap(),
        );
    });
    ui.add_space(PLAN_LINE_PAD);
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
    line.label(font::run(number, font::mono(font::MONO_TIGHT)).color(number_ink));
    line.add_space(ALONG_A_STEP);

    // The mark is a shape and not a character, so a step that has not been
    // reached leaves a hole the same size rather than shifting its label.
    let (dot, _) = line.allocate_exact_size(vec2(STEP_DOT, STEP_DOT), Sense::hover());
    if let Some(colour) = mark {
        line.painter().circle_filled(dot.center(), STEP_DOT / 2.0, colour);
    }
    line.add_space(ALONG_A_STEP);

    let label = font::run(
        step.label,
        if emphasis {
            font::emphasis(line.ctx(), font::CONTROL)
        } else {
            font::plain(font::CONTROL)
        },
    )
        .color(label_ink);
    line.with_layout(Layout::right_to_left(Align::Center), |ui| {
        if !meta.is_empty() {
            ui.label(font::run(meta, font::mono(font::MONO_TIGHT)).color(palette.ink_3));
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
                font::run(icon::DROP, font::icon(ui.ctx(), font::ICON_LARGE))
                    .color(palette.accent),
            );
            ui.add_space(BETWEEN_OVERLAY_PARTS);
            ui.label(
                font::run(heading, font::emphasis(ui.ctx(), font::HEADING))
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
            font::run(number, font::mono(font::MONO_TIGHT))
                .color(if file.accepted { palette.accent_text } else { palette.ink_3 }),
        );
        line.add_space(BESIDE_A_NUMBER);
        let mut name = font::run(&file.name, font::mono(font::MONO)).color(ink);
        if !file.accepted {
            name = name.strikethrough();
        }
        line.with_layout(Layout::right_to_left(Align::Center), |ui| {
            if !file.note.is_empty() {
                ui.label(
                    font::run(&file.note, font::plain(font::NOTE)).color(palette.ink_3),
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

    /// The substitution itself, which is the half of [`drawn_path`] that can be
    /// asserted on a machine whose separator is already the right one.
    ///
    /// Written as the failure was seen: `~/.orng\entries.tsv` is what the
    /// Settings report drew on Windows where every other machine drew
    /// `~/.orng/entries.tsv`, and those two glyphs are the 43 to 48 pixels the
    /// three Settings snapshots differed by.
    #[test]
    fn a_drawn_path_spells_its_separators_the_way_the_design_does() {
        assert_eq!(with_drawn_separators(r"~/.orng\entries.tsv"), "~/.orng/entries.tsv");
        assert_eq!(
            with_drawn_separators(r"~/.orng\backups\6.1-94a98411"),
            "~/.orng/backups/6.1-94a98411"
        );
        // Already right is left alone rather than doubled.
        assert_eq!(with_drawn_separators("~/.orng/entries.tsv"), "~/.orng/entries.tsv");
    }

    /// And the whole of it, against a path built the way the application builds
    /// every path it draws: by `join`, which is what puts the platform's
    /// separator in. Cannot fail on the machine this was written on - it is kept
    /// for the machine it can, the same way
    /// `render::the_drawn_installation_path_is_not_a_function_of_the_platform`
    /// is.
    #[test]
    fn a_joined_path_draws_the_same_on_every_platform() {
        let joined = std::path::Path::new(".orng").join("backups").join("6.1-94a98411");
        assert_eq!(drawn_path(&joined), ".orng/backups/6.1-94a98411");
    }

    /// A row the width of the window the design is drawn at.
    fn a_row(height: f32) -> Rect {
        a_row_of(metric::WINDOW[0], height)
    }

    fn a_row_of(width: f32, height: f32) -> Rect {
        Rect::from_min_size(egui::pos2(0.0, 0.0), vec2(width, height))
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
        let columns = Columns::across(a_row(metric::ROW), Width::Full);
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 452.0));
        assert_eq!(edges(columns.uuid.expect("the full grid carries the identity")), (464.0, 570.0));
        assert_eq!(edges(columns.status), (582.0, 712.0));
        assert_eq!(edges(columns.actions), (724.0, 808.0));
    }

    /// The same row beside the inspector: `66px minmax(0,1fr) 116px 76px`, and
    /// no identity at all.
    ///
    /// Asserted at 533 rather than at 548, because 533 is what the bundle's own
    /// list measures once the scrollbar has taken its fifteen - which is the
    /// state the shell was probed in, so these are its numbers rather than an
    /// arithmetic of ours. The grid is a function of the rectangle it is given,
    /// so the width it is asked about is the width the answer is about.
    #[test]
    fn the_narrow_row_is_divided_as_the_bundle_divides_it() {
        const BESIDE_THE_INSPECTOR: f32 = 533.0;
        let columns = Columns::across(a_row_of(BESIDE_THE_INSPECTOR, metric::ROW), Width::Narrow);
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 305.0));
        assert_eq!(columns.uuid, None, "the narrow grid has no room for an identity");
        assert_eq!(edges(columns.status), (317.0, 433.0));
        assert_eq!(edges(columns.actions), (445.0, 521.0));
    }

    /// The same, for `CatalogRow`: `66px minmax(0,1fr) 116px 56px 142px 92px`.
    #[test]
    fn a_catalog_row_is_divided_as_the_bundle_divides_it() {
        let columns = CatalogColumns::across(a_row(metric::CATALOG_ROW), Width::Full);
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 354.0));
        assert_eq!(edges(columns.author.expect("the full grid names the author")), (366.0, 482.0));
        assert_eq!(edges(columns.version.expect("and the version")), (494.0, 550.0));
        assert_eq!(edges(columns.status), (562.0, 704.0));
        assert_eq!(edges(columns.actions), (716.0, 808.0));
    }

    /// And beside the detail panel: `66px minmax(0,1fr) 108px 122px`, at the
    /// 533 the shell's list measures once its scrollbar has taken fifteen -
    /// the same width the entry list's narrow grid is measured at, and for the
    /// same reason.
    #[test]
    fn the_narrow_catalog_row_is_divided_as_the_bundle_divides_it() {
        const BESIDE_THE_DETAIL: f32 = 533.0;
        let columns =
            CatalogColumns::across(a_row_of(BESIDE_THE_DETAIL, metric::CATALOG_ROW), Width::Narrow);
        assert_eq!(edges(columns.kind), (12.0, 78.0));
        assert_eq!(edges(columns.name), (90.0, 267.0));
        assert_eq!(columns.author, None, "the narrow grid has no room for an author");
        assert_eq!(columns.version, None);
        assert_eq!(edges(columns.status), (279.0, 387.0));
        assert_eq!(edges(columns.actions), (399.0, 521.0));
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

    /// The search field, against both widths the shell was probed at.
    ///
    /// Neither number is "whatever is left", which is the thing worth a test:
    /// the design gives the field `flex:1 1 auto` and puts a second flexible
    /// box between the chips and the controls at the right end, so the two
    /// share what the toolbar has spare. At 820 the field's share would put it
    /// at 262 and the cap holds it to 216; at 548 the spare is 138 and the
    /// field takes 69 of it. "The rest" would have drawn 207 there.
    ///
    /// The inputs are the bundle's own boxes, out of the two probes: the chips,
    /// the factory toggle, `Add files...`, four gaps of eight, and the field's
    /// own content. **Four gaps where `App::local_toolbar` passes three** - the
    /// toggle is deliberately not drawn, so that bar has one box fewer and one
    /// gap fewer, and the field is given what the missing box leaves. What is
    /// under test here is the formula, and the formula is the same on both.
    #[test]
    fn the_search_field_is_as_wide_as_the_bundle_draws_it() {
        let gaps = 4.0 * metric::TOOL_GAP;
        let wide = search_width(796.0, 237.0 + 62.0 + 98.0 + gaps, 157.0, metric::SEARCH_FIELD);
        assert_eq!(wide, metric::SEARCH_FIELD, "at 820 the cap is what holds it");
        let narrow = search_width(524.0, 225.0 + 16.0 + 36.0 + gaps, 77.0, metric::SEARCH_FIELD);
        assert_eq!(narrow, 146.0, "beside the inspector");
    }

    /// And the catalog's, which is the same arithmetic against a bar with one
    /// box fewer and a cap a hundred pixels higher.
    ///
    /// Both numbers are `CatalogToolbar.dc.html` probed at its own two widths.
    /// The inputs are its boxes - the three chips at 237.31 in *both* widths,
    /// because this bar's `chip()` takes no `narrow` argument, the install
    /// filter at 172.41, three gaps of eight, and the field's own content.
    ///
    /// **The narrow one is where the even split stops being the answer.** The
    /// share would put the field at 83.73 and the bundle draws 82.28, because
    /// the flexible gap beside it has reached its `min-width:8px` and stopped
    /// shrinking while the field carried on. A formula that only ever halved
    /// the spare would draw this bar a pixel and a half too wide and push the
    /// install filter off the right edge of the window.
    #[test]
    fn the_catalogs_search_field_is_as_wide_as_its_own_bundle_draws_it() {
        let gaps = 3.0 * metric::TOOL_GAP;
        let fixed = 237.31 + 172.41 + gaps;
        let wide = search_width(796.0, fixed, 288.78, metric::CATALOG_SEARCH_FIELD);
        assert_eq!(wide, metric::CATALOG_SEARCH_FIELD, "at 820 the cap is what holds it");
        // The wider of the two caps, which is the whole reason there are two.
        const _: () = assert!(metric::CATALOG_SEARCH_FIELD > metric::SEARCH_FIELD);

        let narrow = search_width(524.0, fixed, 77.17, metric::CATALOG_SEARCH_FIELD);
        assert!(
            (narrow - 82.28).abs() < 0.01,
            "beside the panel the bundle draws 82.28, not {narrow}"
        );
        // What the gap has left, which is the whole reason that number is not
        // the share: eight, exactly, and never less.
        assert_eq!(524.0 - fixed - narrow, metric::FLEXIBLE_GAP_FLOOR);
        // And still clear of the floor a search field stops being worth having
        // at, which is what lets this bar keep the chips at their full padding
        // where the Local bar has to tighten them.
        assert!(narrow > metric::SEARCH_FLOOR);
    }

    /// The install filter, against `CatalogToolbar.dc.html:33-37` and against
    /// the switch it is nearly a copy of.
    ///
    /// Constants against constants, which proves less than a measurement - the
    /// drawn control is measured in `render.rs`. What this pins is the pair of
    /// numbers that differ, because the temptation is to call these one control
    /// and draw both at the appearance switch's height.
    #[test]
    fn the_install_filter_stands_at_a_bar_controls_height() {
        const _: () = assert!(
            metric::INSTALL_FILTER_SEGMENT + 2.0 * metric::SEGMENTS_PAD == metric::CONTROL,
            "the well is the height of everything else on the bar"
        );
        const _: () = assert!(metric::INSTALL_FILTER_SEGMENT != metric::SEGMENT);
        const _: () = assert!(metric::INSTALL_FILTER_PAD_X != metric::SEGMENT_PAD_X);
    }

    /// The inspector, against the shell probed with the panel open.
    ///
    /// Every assertion here is a sum against a number that was measured, not a
    /// constant against itself: what the list is left, where the first label
    /// lands, how tall an empty keyword box is, and the two chips whose height
    /// is set by the sixteen-pixel icon in them rather than by their text.
    #[test]
    fn the_inspector_is_measured_as_the_bundle_measures_it() {
        assert_eq!(metric::WINDOW[0] - metric::ASIDE, 548.0, "what the list is left");

        // The panel's top is the toolbar's, and its first label sits 53 below
        // that: the header, then the body's own padding. The probe puts it at
        // 125 with the window's content starting at 72.
        assert_eq!(metric::ASIDE_HEADER + metric::ASIDE_PAD_Y, 53.0);
        // Then the label, five, and a 27-tall field: 143 in the probe.
        assert_eq!(metric::UNDER_A_FIELD_LABEL + metric::FIELD, 32.0);

        // The keyword box holding one row of chips. The design's `min-height`
        // is on the content and not on the box, so this is 37 and not 27 - the
        // difference between a box that fits its chips and one that does not.
        assert_eq!(metric::KEYWORDS + 2.0 * metric::KEYWORDS_PAD_Y, 37.0);
        assert_eq!(
            metric::KEYWORDS.max(metric::KEYWORD),
            metric::KEYWORDS,
            "a single row of chips does not fill the box, which is the point of the minimum"
        );

        // The placement chip is sized by the sixteen-pixel icon in it, with one
        // and a half above and below. The keyword chip beside it holds text
        // instead and the design draws it one pixel shorter, which is the pair
        // a reader would otherwise round to the same number.
        assert_eq!(metric::PLACEMENT - font::ICON, 2.0 * 1.5);
        assert_eq!(metric::PLACEMENT - metric::KEYWORD, 1.0);
        // An action is sized by its icon as well, by five - the rule a menu
        // item is laid out by, and the same icon.
        assert_eq!(metric::PANEL_ACTION - font::ICON, 2.0 * 5.0);
    }

    /// The two panels are one shape apart from the air between their groups.
    ///
    /// Both bundles state 272 wide, a 40-tall header and a `13px 12px` body, and
    /// then disagree: `Inspector.dc.html`'s body column is `gap:14px` and
    /// `CatalogDetail.dc.html`'s is `gap:13px`. The hairline between two groups
    /// is a child of that column, so the air around it is that gap - which is
    /// why `rule` takes it rather than stating one.
    ///
    /// This pins the pair against the bundles. That the rule actually *uses*
    /// the caller's is what `catalog-detail.png` holds: hardcoding either one
    /// moves everything below the rule in the other panel by two.
    #[test]
    fn the_two_panels_space_their_groups_as_their_own_bundles_do() {
        assert_eq!(metric::BETWEEN_GROUPS, 14.0, "the inspector's");
        assert_eq!(metric::BETWEEN_DETAIL_GROUPS, 13.0, "the catalog detail's");
        assert_ne!(
            metric::BETWEEN_GROUPS,
            metric::BETWEEN_DETAIL_GROUPS,
            "two numbers a pixel apart are the kind a reader rounds to one"
        );
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

    /// A screen's header, against `SettingsScreen.dc.html` rendered at its own
    /// preview size and asked for every box.
    ///
    /// The bundle puts the back control at 12 and 66 wide, the rule at 87, the
    /// gear at 97 and the title at 122 - so the chain is four gaps of nine, and
    /// what is asserted is the chain rather than the four x positions, because
    /// the control's width is text and text in a mockup is not comparable.
    ///
    /// The header's own height is the one number worth stating twice: it is 44
    /// where the install bar it replaces is 42, and two screens differing by two
    /// pixels is exactly the pair a reader rounds to one.
    #[test]
    fn a_screens_header_is_measured_as_the_bundle_measures_it() {
        assert_eq!(metric::SCREEN_HEADER, 44.0);
        assert_ne!(
            metric::SCREEN_HEADER,
            metric::INSTALL_BAR,
            "a screen's header is not the bar it stands in for"
        );
        // The back control holds a sixteen-pixel icon with four above and below,
        // and is padded asymmetrically so the icon lands where the window's own
        // edge padding puts everything else: six in from twelve is eighteen,
        // which is where the bundle draws the arrow.
        assert_eq!(metric::BACK - font::ICON, 2.0 * 4.0);
        assert_eq!(metric::PAD + metric::BACK_PAD_LEFT, 18.0);
        assert_ne!(
            metric::BACK_PAD_LEFT,
            metric::BACK_PAD_RIGHT,
            "the design pulls the left side in, and a symmetric pad loses that"
        );
        // From the rule to the gear, and from the gear to the title: the gap is
        // the header's own nine, wider than a toolbar's eight.
        assert_eq!(metric::HAIRLINE + metric::SCREEN_HEADER_GAP, 10.0, "87 to 97");
        assert_eq!(font::ICON + metric::SCREEN_HEADER_GAP, 25.0, "97 to 122");
        assert_ne!(metric::SCREEN_HEADER_GAP, metric::TOOL_GAP);
    }

    /// The Settings screen's column and the grid of a path row, against the same
    /// render.
    ///
    /// The bundle puts the column at 12 and 620 wide, the label at 24 in a fixed
    /// 128, the path field at 160, `Browse` ending at 585 and the reset control
    /// at 594 by 620. Only the fixed columns and the gaps are asserted: the
    /// field's width and the control's are functions of text, and the mockup
    /// resolves Inter from a network it does not have.
    #[test]
    fn a_path_row_is_divided_as_the_bundle_divides_it() {
        assert_eq!(metric::SCREEN_COLUMN, 620.0, "the column the groups are capped at");
        // A group's own padding puts its first cell at 24, which is the same 24
        // the window's edge and a row's own padding come to.
        assert_eq!(metric::SCREEN_PAD_X + metric::GROUP_PAD_X, 24.0);
        // Then the label column and one gap: the field starts at 160.
        assert_eq!(
            metric::SCREEN_PAD_X
                + metric::GROUP_PAD_X
                + metric::PATH_LABEL_COLUMN
                + metric::ALONG_A_PATH_ROW,
            160.0
        );
        // And from the right: the column's content ends at 620, the reset
        // control is 26 of it, and a gap of eight stands before the control
        // beside it.
        assert_eq!(
            metric::SCREEN_COLUMN + metric::SCREEN_PAD_X - metric::GROUP_PAD_X - metric::RESET[0],
            594.0,
            "where the reset control starts"
        );
        assert_ne!(metric::RESET[0], metric::RESET[1], "the one control that is not square");

        // A row is as tall as the tallest thing in it, which is that control -
        // and the path field is shorter than the inspector's, because a value to
        // read is not a box to type in.
        assert_eq!(metric::RESET[1], 25.0);
        assert_eq!(metric::PATH_FIELD, 23.0);
        const { assert!(metric::PATH_FIELD < metric::FIELD, "shorter than the inspector's") };
        // Three rows of it, nine apart, inside eleven of padding: 114.
        assert_eq!(
            2.0 * metric::GROUP_PAD_Y
                + 2.0 * metric::RESET[1]
                + metric::GROUP_CONTROL
                + 2.0 * metric::BETWEEN_GROUP_ROWS,
            114.0,
            "the Paths group's drawn height"
        );
    }

    /// A backup row, against `RestoreScreen.dc.html` rendered at its own preview
    /// size and asked for every box.
    ///
    /// The bundle puts the list at 12 and 796 wide with rows at 158, 209 and
    /// 260; inside a row, the mark at 24 by 175, the words at 50, and a content
    /// box of 30. Only the fixed cells are asserted: the size at the right end
    /// and the line under the day are both text.
    #[test]
    fn a_backup_row_is_measured_as_the_bundle_measures_it() {
        // The pitch, which is the row plus the seam of panel the design leaves
        // showing between two of them - the same seam a list of choices leaves,
        // and for the same reason.
        assert_eq!(metric::BACKUP_ROW + metric::BETWEEN_BACKUPS, 51.0);
        assert_eq!(metric::BETWEEN_BACKUPS, metric::BETWEEN_CHOICES);
        // The content box the mark and the two lines are centred in.
        assert_eq!(metric::BACKUP_ROW - 2.0 * metric::BACKUP_ROW_PAD_Y, 30.0);
        // A sixteen-pixel mark centred in that box lands at 175 in a row that
        // starts at 158, which is the seven this comes to.
        assert_eq!((30.0 - font::ICON) / 2.0 + metric::BACKUP_ROW_PAD_Y, 17.0);
        // Where the words start, from the window's edge: the list's own twelve,
        // the row's twelve, the mark, and the design's gap after it.
        assert_eq!(
            metric::PAD + metric::BACKUP_ROW_PAD_X + font::ICON + metric::ALONG_A_BACKUP_ROW,
            50.0
        );
        // The two lines fit that box with nothing to spare, which is what makes
        // the row 50 rather than a number somebody chose: a 12-pixel day, two,
        // and a 10-pixel line on the mono face.
        const {
            assert!(
                font::FIELD_VALUE + metric::UNDER_A_BACKUP_DATE + font::MONO_TIGHT
                    <= metric::BACKUP_ROW - 2.0 * metric::BACKUP_ROW_PAD_Y
            )
        };
    }

    /// The bar at the foot of the Restore screen, against the same render: 48
    /// tall, with `Cancel` at 335 and the primary at 333 inside a bar at 326.
    ///
    /// The primary's box is 33 there and 31 here, because the design draws its
    /// outline *outside* the height it states - so what is asserted is where the
    /// drawn box lands, hairlines included.
    #[test]
    fn the_restore_foot_is_measured_as_the_bundle_measures_it() {
        assert_eq!((metric::SCREEN_FOOT - metric::CANCEL) / 2.0, 9.5, "326 to 335");
        let drawn = metric::SCREEN_PRIMARY + 2.0 * metric::HAIRLINE;
        assert_eq!((metric::SCREEN_FOOT - drawn) / 2.0, 7.5, "326 to 333");
        // The pair is the design's own hierarchy, twice over: the primary is the
        // taller of the two, and both are smaller than the action bar's - this
        // is a screen's decision and not the one thing the window does.
        const { assert!(metric::CANCEL < metric::SCREEN_PRIMARY) };
        const { assert!(metric::SCREEN_PRIMARY < metric::ACTION) };
        const { assert!(metric::SCREEN_FOOT < metric::ACTION_BAR) };
    }

    /// The About screen, against `AboutScreen.dc.html` rendered at its own
    /// preview size: a 560 column at 12, blocks 18 apart, and a fact box whose
    /// three values all start at 130.
    #[test]
    fn the_about_screen_is_measured_as_the_bundle_measures_it() {
        // Where every value starts, from the window's edge: the screen's twelve,
        // the group's twelve, the fixed label column and the gap after it.
        assert_eq!(
            metric::SCREEN_PAD_X
                + metric::GROUP_PAD_X
                + metric::FACT_LABEL_COLUMN
                + metric::ALONG_A_FACT_ROW,
            130.0
        );
        // The column and the air over it are not Settings', and the difference
        // is the whole of what `Measure` carries: prose gets a narrower measure
        // and more room above it than rows of controls do.
        assert_eq!(Measure::Prose.column(), 560.0);
        assert_eq!(Measure::Controls.column(), metric::SCREEN_COLUMN);
        const { assert!(Measure::Prose.column() < Measure::Controls.column()) };
        const { assert!(Measure::Prose.top() > Measure::Controls.top()) };
        // The mark beside the application's name: a 52-pixel tile with a
        // 28-pixel glyph in it, so the air around the glyph is twelve a side -
        // the window's own padding, which is what makes the tile read as a box
        // and not as an icon that grew.
        assert_eq!((metric::PRODUCT_MARK - font::PRODUCT_ICON) / 2.0, metric::PAD);
        // Facts sit closer together than a group's rows do: these are lines to
        // read down, and those are rows to act on.
        const { assert!(metric::BETWEEN_ABOUT_FACTS < metric::BETWEEN_GROUP_ROWS) };
    }

    /// Both screens' notices are one box, and it is not the catalog detail's.
    ///
    /// `RestoreScreen.dc.html:36` and `AboutScreen.dc.html:57` state the same
    /// `padding:10px 12px`, the same `gap:9px` and the same radius, so one
    /// component draws both. `CatalogDetail`'s superseded block states `11px`
    /// and `6px` and is a different thing, which is the pair a reader collapses.
    #[test]
    fn a_screens_notice_is_not_a_panels_notice() {
        assert_eq!(metric::SCREEN_NOTICE_PAD_X, 12.0);
        assert_eq!(metric::SCREEN_NOTICE_PAD_Y, 10.0);
        assert_ne!(metric::SCREEN_NOTICE_PAD_X, metric::NOTICE_PAD_X);
        assert_ne!(metric::ALONG_A_SCREEN_NOTICE, metric::IN_A_NOTICE);
        // And the glyph is nudged by the same one pixel a choice's mark is,
        // which is the design writing `margin-top:1px` in both places.
        assert_eq!(metric::MARK_DROP, 1.0);
    }

    /// The four group boxes down the screen, against where the bundle puts each
    /// one: 80, 229, 369 and 458.
    ///
    /// A chain, because that is what it is: every group is its heading, seven,
    /// its own height, and fifteen to the next. Asserting the four positions
    /// separately would pass with the wrong gap and the wrong heading height
    /// cancelling out, which is how a column comes to be right at the top and
    /// three pixels out at the bottom.
    #[test]
    fn the_settings_groups_are_spaced_as_the_bundle_spaces_them() {
        assert_eq!(metric::BETWEEN_SETTINGS_GROUPS, 15.0);
        assert_ne!(
            metric::BETWEEN_SETTINGS_GROUPS,
            metric::BETWEEN_GROUPS,
            "a screen is not a panel, and the two are a pixel apart"
        );
        // A heading is 10.5 on a 13-pixel line, and the design puts seven under
        // it - so a group's box is 20 below where its heading starts.
        const HEADING: f32 = 13.0;
        let over_a_group = HEADING + metric::UNDER_A_GROUP_HEADING;
        assert_eq!(over_a_group, 20.0);

        // The first heading is the header plus the body's top padding, and the
        // first box is twenty below that.
        let first = metric::SCREEN_HEADER + metric::SCREEN_PAD_TOP;
        assert_eq!(first, 60.0, "where the first heading starts");
        assert_eq!(first + over_a_group, 80.0, "where the Paths box starts");

        // Two choices of 47, one apart, inside five: the placement group is 105,
        // and it starts fifteen and a heading below where Paths ends.
        const CHOICE: f32 = 47.0;
        let placement = 2.0 * metric::CHOICES_PAD + 2.0 * CHOICE + metric::BETWEEN_CHOICES;
        assert_eq!(placement, 105.0);
        let after_paths = 80.0 + 114.0 + metric::BETWEEN_SETTINGS_GROUPS + over_a_group;
        assert_eq!(after_paths, 229.0, "where the placement box starts");

        // The checkbox group is one choice-shaped row in the group's own
        // padding: two lines and the two above and below them.
        const TWO_LINES: f32 = 31.0;
        let removing = 2.0 * metric::GROUP_PAD_Y + TWO_LINES;
        assert_eq!(removing, 53.0);
        let after_placement =
            after_paths + placement + metric::BETWEEN_SETTINGS_GROUPS + over_a_group;
        assert_eq!(after_placement, 369.0, "where the removing-entries box starts");

        // And the switch, which carries its own height and gets nine either
        // side rather than eleven.
        let appearance = 2.0 * metric::SWITCH_PAD_Y + metric::SEGMENTS;
        assert_eq!(appearance, 46.0);
        // 458 in the bundle and 457 in the chain, and the pixel is the bundle's.
        // A 10.5-pixel heading has a fractional line box, and the render rounds
        // it down at the first three headings and up at the fourth - so the
        // design's own four numbers are not four steps of one arithmetic. The
        // chain is held to a pixel here rather than being bent to reproduce a
        // rounding, which is the standard this is judged by anyway.
        let fourth = after_placement + removing + metric::BETWEEN_SETTINGS_GROUPS + over_a_group;
        assert!(
            (fourth - 458.0f32).abs() <= 1.0,
            "the appearance box starts at {fourth}, the bundle draws it at 458"
        );
    }

    /// The appearance switch, against the bundle's own boxes: a well 220 by 28
    /// with two of padding, and three segments 24 tall touching two apart.
    #[test]
    fn the_appearance_switch_is_measured_as_the_bundle_measures_it() {
        assert_eq!(metric::SEGMENTS - 2.0 * metric::SEGMENTS_PAD, metric::SEGMENT);
        // The design rounds the well by the field's four and a segment inside it
        // by a control's three, so the segment sits inside the corner rather
        // than cutting across it.
        const { assert!(metric::RADIUS < metric::FIELD_RADIUS) };
        // Inside a segment: ten of padding, a sixteen-pixel icon, five, the word.
        // The bundle puts `System`'s icon 10 in from the segment's left edge.
        assert_eq!(metric::SEGMENT_PAD_X, 10.0);
        assert_eq!(metric::ALONG_A_SEGMENT, 5.0);
        assert_eq!(metric::SEGMENT - font::ICON, 2.0 * 4.0);
    }
}
