// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Every colour, measurement and text style the interface uses.
//!
//! One definition, applied once to the context. Nothing that draws may name a
//! colour or a number of its own: a widget that reaches for `Color32::from_rgb`
//! is a widget that will not follow the theme when it changes, and light mode is
//! already drawn and waiting to be built.
//!
//! The values are the design's own, read out of the handoff bundle's custom
//! properties rather than eyeballed from a screenshot. Dark is `:root` there and
//! so is the default here; light is the same set under `data-theme="light"`.

use std::sync::Arc;

use eframe::egui::{
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, FontId, Stroke, TextStyle,
};

/// A colour as the design writes one, so the two can be compared by eye.
const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// A colour the design gives as `rgba(...)` over the page.
const fn hexa(rgb: u32, alpha: u8) -> Color32 {
    Color32::from_rgba_premultiplied(
        (((rgb >> 16) as u8) as u16 * alpha as u16 / 255) as u8,
        (((rgb >> 8) as u8) as u16 * alpha as u16 / 255) as u8,
        ((rgb as u8) as u16 * alpha as u16 / 255) as u8,
        alpha,
    )
}

/// The palette, named as the design names it.
///
/// Held as data rather than as constants so the light theme is a second value
/// of the same type rather than a second code path.
///
/// Every slot the design defines is transcribed, including the few nothing
/// draws with yet. This is a copy of an external document rather than an API
/// invented here, and a palette with holes in it is one that gets filled in by
/// whoever next needs a colour, somewhere other than this file.
#[allow(dead_code, reason = "a full transcription of the design's tokens")]
#[derive(Debug, Clone, Copy)]
pub struct Palette {
    pub page: Color32,
    pub bg: Color32,
    pub panel: Color32,
    pub panel_2: Color32,
    pub row: Color32,
    pub row_alt: Color32,
    pub row_hover: Color32,
    pub line: Color32,
    pub line_soft: Color32,
    /// Primary text.
    pub ink: Color32,
    /// Secondary text: values beside their labels.
    pub ink_2: Color32,
    /// Tertiary text: labels, units, anything supporting.
    pub ink_3: Color32,
    pub accent: Color32,
    pub accent_text: Color32,
    /// Text drawn *on* the accent.
    pub accent_ink: Color32,
    pub accent_soft: Color32,
    pub ok: Color32,
    pub warn: Color32,
    pub err: Color32,
    pub err_text: Color32,
    pub info: Color32,
    pub field: Color32,
    pub btn: Color32,
    pub btn_hover: Color32,
    /// Laid over everything when the window is a drop target, so what is behind
    /// it reads as out of reach rather than merely dimmed.
    pub scrim: Color32,
}

impl Palette {
    /// The default, and what every mockup in the bundle is drawn in.
    pub const DARK: Palette = Palette {
        page: hex(0x000000),
        bg: hex(0x0a0a0a),
        panel: hex(0x161616),
        panel_2: hex(0x1e1e1e),
        row: hex(0x0a0a0a),
        row_alt: hex(0x1e1e1e),
        row_hover: hex(0x262626),
        line: hex(0x242424),
        line_soft: hex(0x1a1a1a),
        ink: hex(0xf2f2f2),
        ink_2: hex(0xbcbcbc),
        ink_3: hex(0x949494),
        accent: hex(0xff5a1f),
        accent_text: hex(0xff7a45),
        accent_ink: hex(0x0a0a0a),
        accent_soft: hexa(0xff5a1f, 26),
        ok: hex(0xf2f2f2),
        warn: hex(0xff5a1f),
        err: hex(0xff4036),
        err_text: hex(0xff6259),
        info: hex(0xa9a9a9),
        field: hex(0x000000),
        btn: hex(0x282828),
        btn_hover: hex(0x333333),
        scrim: hexa(0x000000, 189),
    };

    /// Drawn, and not yet reachable from the interface. Kept here because a
    /// second palette is what proves the first one is not hard-coded anywhere
    /// else, and because leaving it out would invite exactly that.
    pub const LIGHT: Palette = Palette {
        page: hex(0xffffff),
        bg: hex(0xf2f2f2),
        panel: hex(0xffffff),
        panel_2: hex(0xe4e4e4),
        row: hex(0xf2f2f2),
        row_alt: hex(0xe4e4e4),
        row_hover: hex(0xdcdcdc),
        line: hex(0xc6c6c6),
        line_soft: hex(0xdadada),
        ink: hex(0x171717),
        ink_2: hex(0x3e3e3e),
        ink_3: hex(0x5e5e5e),
        accent: hex(0xe8500f),
        accent_text: hex(0x9e3809),
        accent_ink: hex(0xffffff),
        accent_soft: hexa(0xe8500f, 26),
        ok: hex(0x171717),
        warn: hex(0xe8500f),
        err: hex(0xbf2a1c),
        err_text: hex(0xa32316),
        info: hex(0x3e3e3e),
        field: hex(0xececec),
        btn: hex(0xe2e2e2),
        btn_hover: hex(0xd8d8d8),
        scrim: hexa(0xffffff, 168),
    };
}

/// Measurements, in the same spirit: named once, never typed into a layout.
pub mod metric {
    /// Gap between a label and the thing it labels.
    pub const TIGHT: f32 = 6.0;
    /// Gap between two things that are not part of one phrase. Two view names
    /// set six pixels apart read as one sentence, which is what this is for.
    pub const GAP: f32 = 16.0;
    /// Inside a panel, from its edge to its content.
    pub const PAD: f32 = 14.0;
    /// One entry row. Tall enough for two lines of text plus breathing room.
    pub const ROW_HEIGHT: f32 = 44.0;
    /// The toolbar and the install bar.
    pub const BAR_HEIGHT: f32 = 46.0;
    /// Corner of a panel, a field or a button.
    pub const RADIUS: u8 = 6;
    /// Every separating line in the design is one pixel. Named because it is a
    /// decision, and because an untyped `1.0` is ambiguous to the compiler here.
    pub const HAIRLINE: f32 = 1.0;
    /// The underline marking the current view, which is the one line that is
    /// meant to be seen rather than merely to divide.
    pub const UNDERLINE: f32 = 2.0;
}

/// Load the design's faces.
///
/// Called once. The families are lists and egui walks them in order, and the
/// order matters for the mono face: the Iosevka here is a subset of about four
/// hundred glyphs, so Inter sits behind it and answers for anything it does not
/// carry.
///
/// Not for our own text, which is ASCII by house rule - an arrow is written
/// `->`. It is for text this application does not choose. A device name and a
/// library path come out of a document somebody else made, and either may hold
/// anything Unicode allows; without a face behind Iosevka those draw as boxes,
/// in the one column a user is most likely to be reading carefully.
pub fn install_fonts(ctx: &egui::Context) {
    const INTER: &[u8] = include_bytes!("../../../assets/fonts/InterDisplay-Regular.ttf");
    const MONO: &[u8] = include_bytes!("../../../assets/fonts/subset-Iosevka-Regular-Extended.ttf");

    let mut fonts = FontDefinitions::default();
    for (name, bytes) in [("inter", INTER), ("iosevka", MONO)] {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let family = |fonts: &mut FontDefinitions, key: FontFamily, faces: &[&str]| {
        let list = fonts.families.entry(key).or_default();
        for (at, face) in faces.iter().enumerate() {
            list.insert(at, (*face).to_owned());
        }
    };
    // Only the two families egui binds itself. A `FontFamily::Name` is not bound
    // until the frame after `set_fonts`, so anything drawn in the first frame
    // that asked for one would panic - which is exactly what the empty state,
    // the only thing here with a heading in it, did.
    family(&mut fonts, FontFamily::Proportional, &["inter"]);
    family(&mut fonts, FontFamily::Monospace, &["iosevka", "inter"]);

    ctx.set_fonts(fonts);
}

/// Where a piece of text sits in the hierarchy.
///
/// Mapped onto egui's named styles so a caller says what a thing *is* rather
/// than what size it should be.
pub mod text {
    use eframe::egui::TextStyle;

    /// A screen or section heading.
    pub const HEADING: TextStyle = TextStyle::Heading;
    /// Ordinary interface text.
    pub const BODY: TextStyle = TextStyle::Body;
    /// A label, a unit, a caption.
    pub const SMALL: TextStyle = TextStyle::Small;
    /// A path, an identity, a version, a build string. Anything the user might
    /// copy into a bug report is monospaced, as the design has it.
    pub const MONO: TextStyle = TextStyle::Monospace;
}

/// Apply the whole theme to a context. Called once at startup.
pub fn apply(ctx: &egui::Context, palette: Palette) {
    let mut style = (*ctx.style()).clone();

    style.text_styles = [
        (TextStyle::Heading, FontId::new(17.0, FontFamily::Proportional)),
        (TextStyle::Body, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Button, FontId::new(13.0, FontFamily::Proportional)),
        (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
        (TextStyle::Monospace, FontId::new(12.0, FontFamily::Monospace)),
    ]
    .into();

    let radius = CornerRadius::same(metric::RADIUS);
    let visuals = &mut style.visuals;
    visuals.dark_mode = palette.ink.r() > palette.bg.r();
    visuals.override_text_color = Some(palette.ink);
    visuals.panel_fill = palette.bg;
    visuals.window_fill = palette.panel;
    visuals.extreme_bg_color = palette.field;
    visuals.faint_bg_color = palette.row_alt;
    visuals.hyperlink_color = palette.accent_text;
    visuals.window_stroke = Stroke::new(metric::HAIRLINE, palette.line);
    visuals.selection.bg_fill = palette.accent_soft;
    visuals.selection.stroke = Stroke::new(metric::HAIRLINE, palette.accent_text);

    // Widgets, quietest state first. The design draws buttons as flat fills
    // that lift on hover, with no outline until they are interacted with.
    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = palette.bg;
    w.noninteractive.weak_bg_fill = palette.bg;
    w.noninteractive.bg_stroke = Stroke::new(metric::HAIRLINE, palette.line);
    w.noninteractive.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink_2);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = palette.btn;
    w.inactive.weak_bg_fill = palette.btn;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = palette.btn_hover;
    w.hovered.weak_bg_fill = palette.btn_hover;
    w.hovered.bg_stroke = Stroke::new(metric::HAIRLINE, palette.line);
    w.hovered.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.hovered.corner_radius = radius;

    w.active.bg_fill = palette.accent;
    w.active.weak_bg_fill = palette.accent;
    w.active.bg_stroke = Stroke::new(metric::HAIRLINE, palette.accent);
    w.active.fg_stroke = Stroke::new(metric::HAIRLINE, palette.accent_ink);
    w.active.corner_radius = radius;

    w.open.bg_fill = palette.panel_2;
    w.open.weak_bg_fill = palette.panel_2;
    w.open.bg_stroke = Stroke::new(metric::HAIRLINE, palette.line);
    w.open.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.open.corner_radius = radius;

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(metric::TIGHT, metric::TIGHT);
    spacing.button_padding = egui::vec2(10.0, 5.0);
    spacing.menu_margin = egui::Margin::same(6);
    spacing.interact_size.y = 24.0;

    ctx.set_style(style);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Both palettes must define every slot. A `Palette` is a struct precisely
    /// so the compiler enforces that, but the contrast direction is the thing a
    /// typo silently inverts, and an unreadable theme is not obvious from a
    /// diff.
    #[test]
    fn text_contrasts_with_its_background_in_both_themes() {
        for (name, p) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
            let ink = p.ink.r() as i32 + p.ink.g() as i32 + p.ink.b() as i32;
            let bg = p.bg.r() as i32 + p.bg.g() as i32 + p.bg.b() as i32;
            assert!((ink - bg).abs() > 250, "{name}: ink and bg are too close");

            // Secondary and tertiary text step away from the ink, toward the
            // background, and in that order. The design's ink-2 and ink-3 are
            // quieter, never louder.
            let ink_2 = p.ink_2.r() as i32 + p.ink_2.g() as i32 + p.ink_2.b() as i32;
            let ink_3 = p.ink_3.r() as i32 + p.ink_3.g() as i32 + p.ink_3.b() as i32;
            let toward_bg = |v: i32| (v - bg).abs();
            assert!(toward_bg(ink_2) < toward_bg(ink), "{name}: ink-2 is not quieter than ink");
            assert!(toward_bg(ink_3) < toward_bg(ink_2), "{name}: ink-3 is not quieter than ink-2");
        }
    }

    /// Text drawn on the accent has to read against it, which is the one pair a
    /// palette gets wrong by copying a value from the wrong theme.
    #[test]
    fn accent_ink_reads_against_the_accent() {
        for (name, p) in [("dark", Palette::DARK), ("light", Palette::LIGHT)] {
            let accent = p.accent.r() as i32 + p.accent.g() as i32 + p.accent.b() as i32;
            let on = p.accent_ink.r() as i32 + p.accent_ink.g() as i32 + p.accent_ink.b() as i32;
            assert!((accent - on).abs() > 200, "{name}: accent-ink does not read on accent");
        }
    }
}
