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
    self, Color32, CornerRadius, FontData, FontDefinitions, FontFamily, Stroke, TextStyle,
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
    /// Secondary and tertiary text inside something toned. The design warms or
    /// reddens the supporting text of a banner rather than leaving it grey
    /// beside a coloured headline.
    pub ink_2_warm: Color32,
    pub ink_3_warm: Color32,
    pub ink_2_err: Color32,
    pub ink_3_err: Color32,
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
    /// Every other row, over whatever the list sits on.
    pub zebra: Color32,
    /// The wash behind a banner, one per tone.
    pub ok_bg: Color32,
    pub warn_bg: Color32,
    pub err_bg: Color32,
    pub info_bg: Color32,
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
        ink_2_warm: hex(0xc6b6aa),
        ink_3_warm: hex(0x9e9089),
        ink_2_err: hex(0xc9b4b1),
        ink_3_err: hex(0x9d8c8a),
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
        zebra: hexa(0xffffff, 13),
        ok_bg: hexa(0xf2f2f2, 15),
        warn_bg: hexa(0xff5a1f, 26),
        err_bg: hexa(0xff4036, 26),
        info_bg: hexa(0xa9a9a9, 18),
    };

    /// Drawn, and not yet reachable from the interface. Kept here because a
    /// second palette is what proves the first one is not hard-coded anywhere
    /// else, and because leaving it out would invite exactly that.
    pub const LIGHT: Palette = Palette {
        page: hex(0xe6e6e6),
        bg: hex(0xf2f2f2),
        panel: hex(0xfafafa),
        panel_2: hex(0xffffff),
        row: hex(0xf2f2f2),
        row_alt: hex(0xe4e4e4),
        row_hover: hex(0xdcdcdc),
        line: hex(0xc6c6c6),
        line_soft: hex(0xdadada),
        ink: hex(0x171717),
        ink_2: hex(0x3e3e3e),
        ink_3: hex(0x5e5e5e),
        ink_2_warm: hex(0x463c34),
        ink_3_warm: hex(0x665c53),
        ink_2_err: hex(0x453a38),
        ink_3_err: hex(0x655a58),
        accent: hex(0xe8500f),
        accent_text: hex(0x9e3809),
        // Dark ink on the accent in both themes. The accent is the same orange
        // in either, and it is light enough that white on it does not read -
        // which is why the design writes the same value twice rather than
        // flipping this one with the theme.
        accent_ink: hex(0x0a0a0a),
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
        zebra: hexa(0x000000, 11),
        ok_bg: hexa(0x171717, 13),
        warn_bg: hexa(0xe8500f, 26),
        err_bg: hexa(0xbf2a1c, 23),
        info_bg: hexa(0x3e3e3e, 18),
    };
}

/// Measurements, in the same spirit: named once, never typed into a layout.
///
/// These are the design's own numbers, read out of the handoff bundle's inline
/// styles rather than judged by eye. Where the design gives a height it is here
/// as a height, not as a padding that happens to produce one.
///
/// **They are absolute, and they were authored for a window of [`WINDOW`].** A
/// thirty-six pixel row in a window half as tall again is not the same row: the
/// numbers still hold but the density does not, and the list reads sparse
/// against the bundle. Growing the window is the user's to do; opening larger
/// than the design was drawn at is not ours.
pub mod metric {
    /// The window the design is drawn at, and therefore the size this opens.
    pub const WINDOW: [f32; 2] = [820.0, 560.0];

    /// Between two things that belong to one phrase: an icon and its label, a
    /// chip and its count.
    pub const TIGHT: f32 = 6.0;
    /// Between two things that do not.
    pub const GAP: f32 = 12.0;
    /// Between the items of a toolbar, which is tighter than a bar's.
    pub const TOOL_GAP: f32 = 8.0;
    /// Between two members of one group, such as the view switch.
    pub const SNUG: f32 = 2.0;
    /// From the edge of a bar or a row to its content.
    pub const PAD: f32 = 12.0;

    /// The bar naming the installation, across the top of both views.
    pub const INSTALL_BAR: f32 = 42.0;
    /// Search, kind filters, the factory toggle and `Add files...`.
    pub const TOOLBAR: f32 = 42.0;
    /// The summary and the one primary action.
    pub const ACTION_BAR: f32 = 52.0;
    /// One entry.
    pub const ROW: f32 = 36.0;
    /// A heading dividing the list into pending, registered and factory.
    pub const SECTION: f32 = 26.0;
    /// A control inside a bar: a kind chip, a search field, `Add files...`.
    pub const CONTROL: f32 = 26.0;
    /// A control in the install bar, which the design draws two pixels shorter
    /// than the toolbar's: the view tabs and the overflow.
    pub const TAB: f32 = 24.0;
    /// The overflow control, which is wider than it is tall and is the one
    /// control the design gives both measurements for.
    pub const OVERFLOW: f32 = 26.0;
    /// The primary action, which is taller than anything beside it.
    pub const ACTION: f32 = 32.0;
    /// A control drawn as an outline rather than as a fill, which the design
    /// uses for the one thing a banner offers.
    pub const OUTLINED: f32 = 28.0;

    /// Corner of a control. The design rounds by three, not by six: at these
    /// sizes a six-pixel radius reads as a pill rather than as a soft corner.
    pub const RADIUS: u8 = 3;
    /// Every separating line in the design is one pixel. Named because it is a
    /// decision, and because an untyped `1.0` is ambiguous to the compiler here.
    pub const HAIRLINE: f32 = 1.0;

    /// How wide the columns of an entry row are. The design lays a row out as a
    /// grid rather than as a flow, so the identities and the statuses line up
    /// down the list instead of wandering with the length of each name.
    pub const KIND_COLUMN: f32 = 66.0;
    pub const UUID_COLUMN: f32 = 106.0;
    pub const STATUS_COLUMN: f32 = 130.0;
    /// The row's own controls, at its right end. Reserved whether or not the
    /// pointer is over the row, because the design hides them rather than
    /// removing them: a column that appears on hover would move every other
    /// column under the pointer.
    pub const ACTIONS_COLUMN: f32 = 84.0;
    /// The author and the version of a catalog item, which take the place of
    /// the identity and the status a registered row carries.
    pub const AUTHOR_COLUMN: f32 = 116.0;
    pub const VERSION_COLUMN: f32 = 56.0;
    pub const CATALOG_STATUS_COLUMN: f32 = 142.0;
    pub const CATALOG_ACTIONS_COLUMN: f32 = 92.0;
    /// A catalog row carries a description under the name, so it is taller than
    /// a registered one.
    pub const CATALOG_ROW: f32 = 48.0;
    /// The mark beside a banner's headline.
    pub const DOT: f32 = 6.0;
    /// The search field, measured across the whole box - the glyph, the gap and
    /// the text - because that is what the bundle's is measured across.
    pub const SEARCH_FIELD: f32 = 216.0;
}

/// Where a piece of text sits in the hierarchy.
///
/// Named by role rather than by size, so a caller says what a thing *is*. The
/// sizes are the design's, which uses more of them than egui has named styles
/// for, so these are [`FontId`]s rather than `TextStyle`s - the five egui styles
/// are still set, for the controls this does not draw by hand.
///
/// `emphasis` is the design's 500 and 600 weights. It is a second family rather
/// than a flag, because that is how a font is chosen, and it needs the context
/// to ask whether the family is bound yet: fonts installed during a pass are not
/// available until the next one, and asking for an unbound family panics.
pub mod font {
    use eframe::egui::{self, FontFamily, FontId};

    /// The installation's name, the loudest thing in the window.
    pub const INSTALL_TITLE: f32 = 13.5;
    /// An entry's display name.
    pub const ROW_NAME: f32 = 12.5;
    /// The primary action.
    pub const ACTION: f32 = 12.0;
    /// A tab, a menu item, a search field, the action bar's summary.
    pub const CONTROL: f32 = 11.5;
    /// A chip, a badge, a status, a section heading.
    pub const CHIP: f32 = 11.0;
    /// A reason, a note, anything explaining the line above it.
    pub const NOTE: f32 = 10.5;
    /// A path, an identity, a version: anything a user might copy.
    pub const MONO: f32 = 10.5;
    /// A count or a build revision, which sit beside text rather than in it.
    pub const MONO_TIGHT: f32 = 10.0;
    /// An empty state's headline.
    pub const HEADING: f32 = 16.5;
    /// The headline of a dialog, which is a smaller surface than a screen.
    pub const DIALOG_TITLE: f32 = 14.0;
    /// The headline of a minor empty state, which is a sentence and not a
    /// heading.
    pub const SUBHEADING: f32 = 13.0;
    /// An icon, which the design sizes with the text it sits beside.
    pub const ICON: f32 = 16.0;
    /// The one icon a screen is built around.
    pub const ICON_LARGE: f32 = 54.0;
    /// The icon of a minor empty state.
    pub const ICON_SMALL: f32 = 36.0;

    pub fn plain(size: f32) -> FontId {
        FontId::new(size, FontFamily::Proportional)
    }

    pub fn mono(size: f32) -> FontId {
        FontId::new(size, FontFamily::Monospace)
    }

    /// The design's heavier weight, when this frame can draw it.
    ///
    /// A named family is bound from the pass *after* `set_fonts`, and this
    /// application installs its fonts while it is being built - which under the
    /// render harness happens inside a pass. Asking for a family that is not
    /// bound panics, so the answer is the regular face until it is.
    pub fn emphasis(ctx: &egui::Context, size: f32) -> FontId {
        named(ctx, super::emphasis(), size)
    }

    /// An icon from the design's set, at the size of the text it sits beside.
    pub fn icon(ctx: &egui::Context, size: f32) -> FontId {
        named(ctx, super::icons(), size)
    }

    fn named(ctx: &egui::Context, family: FontFamily, size: f32) -> FontId {
        let bound = ctx.fonts(|fonts| fonts.families().contains(&family));
        if bound { FontId::new(size, family) } else { plain(size) }
    }
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
    const INTER_MEDIUM: &[u8] = include_bytes!("../../../assets/fonts/InterDisplay-Medium.ttf");
    const MONO: &[u8] = include_bytes!("../../../assets/fonts/subset-Iosevka-Regular-Extended.ttf");
    const MONO_MEDIUM: &[u8] =
        include_bytes!("../../../assets/fonts/subset-Iosevka-Medium-Extended.ttf");

    let mut fonts = FontDefinitions::default();
    let faces = [
        ("inter", INTER),
        ("inter-medium", INTER_MEDIUM),
        ("iosevka", MONO),
        ("iosevka-medium", MONO_MEDIUM),
    ];
    for (name, bytes) in faces {
        fonts.font_data.insert(name.to_owned(), Arc::new(FontData::from_static(bytes)));
    }

    let family = |fonts: &mut FontDefinitions, key: FontFamily, faces: &[&str]| {
        let list = fonts.families.entry(key).or_default();
        for (at, face) in faces.iter().enumerate() {
            list.insert(at, (*face).to_owned());
        }
    };
    family(&mut fonts, FontFamily::Proportional, &["inter"]);
    family(&mut fonts, FontFamily::Monospace, &["iosevka", "inter"]);
    // The design sets its headings and its primary action in a heavier weight,
    // and a weight is a face, so it is a third family. A `FontFamily::Name` is
    // not bound until the pass after `set_fonts`, and this runs while the
    // application is being built - which under the render harness is inside a
    // pass. Anything asking for it in that pass would panic, which is exactly
    // what the empty state once did, so `font::emphasis` asks first.
    family(&mut fonts, FontFamily::Name(EMPHASIS.into()), &["inter-medium", "iosevka-medium"]);

    // The design's own icon set, drawn as text. Phosphor is MIT licensed, and
    // Light is the weight the design asks for by name on its largest icons. It
    // draws the small ones in `duotone`, which is two overlapping glyphs in two
    // colours and has no single-colour font to be drawn from; light is the
    // nearest single weight there is.
    //
    // A family of its own, with nothing in front of it, and that is the whole
    // point. Phosphor lives in the Private Use Area - and so does InterDisplay,
    // which carries 745 glyphs there for its stylistic alternates. Adding the
    // icons to the proportional family as a fallback puts them behind Inter,
    // which then answers for every codepoint the two happen to share: the
    // folder became an r-acute, the overflow an o-ogonek, and the tray an
    // e-circumflex, while the magnifying glass and the tick came out right
    // because Inter does not claim those two.
    let icons = egui_phosphor::Variant::Light.font_bytes();
    fonts.font_data.insert("phosphor".to_owned(), Arc::new(FontData::from_static(icons)));
    family(&mut fonts, FontFamily::Name(ICONS.into()), &["phosphor"]);

    ctx.set_fonts(fonts);
}

/// The heavier of the two weights the design uses.
const EMPHASIS: &str = "emphasis";

/// The design's icon set, alone in a family so that nothing can answer for it.
const ICONS: &str = "icons";

/// That set as a family, for the one call that has to name it.
pub fn icons() -> FontFamily {
    FontFamily::Name(ICONS.into())
}

/// That weight as a family, for the one call that has to name it.
pub fn emphasis() -> FontFamily {
    FontFamily::Name(EMPHASIS.into())
}

/// Apply the whole theme to a context. Called once at startup.
pub fn apply(ctx: &egui::Context, palette: Palette) {
    // Every theme egui keeps, not just the current one. This application picks
    // its own palette and never follows the system, so leaving the other theme
    // at egui's defaults would mean a control drawn from it arrived in the
    // wrong colours.
    ctx.all_styles_mut(|style| apply_to(style, palette));
}

fn apply_to(style: &mut egui::Style, palette: Palette) {
    // What egui draws for itself - the text inside a `Button`, a tooltip - in
    // the sizes the design gives the same things. Everything this application
    // draws by hand names a `font::` role instead.
    style.text_styles = [
        (TextStyle::Heading, font::plain(font::HEADING)),
        (TextStyle::Body, font::plain(font::CONTROL)),
        (TextStyle::Button, font::plain(font::CONTROL)),
        (TextStyle::Small, font::plain(font::CHIP)),
        (TextStyle::Monospace, font::mono(font::MONO)),
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
    // The search field is the only frame egui draws for this application, and
    // the design gives it a fill and no outline at all.
    w.noninteractive.bg_fill = palette.field;
    w.noninteractive.weak_bg_fill = palette.field;
    w.noninteractive.bg_stroke = Stroke::NONE;
    w.noninteractive.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink_2);
    w.noninteractive.corner_radius = radius;

    w.inactive.bg_fill = palette.btn;
    w.inactive.weak_bg_fill = palette.btn;
    w.inactive.bg_stroke = Stroke::NONE;
    w.inactive.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.inactive.corner_radius = radius;

    w.hovered.bg_fill = palette.btn_hover;
    w.hovered.weak_bg_fill = palette.btn_hover;
    w.hovered.bg_stroke = Stroke::NONE;
    w.hovered.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.hovered.corner_radius = radius;

    w.active.bg_fill = palette.accent;
    w.active.weak_bg_fill = palette.accent;
    w.active.bg_stroke = Stroke::NONE;
    w.active.fg_stroke = Stroke::new(metric::HAIRLINE, palette.accent_ink);
    w.active.corner_radius = radius;

    w.open.bg_fill = palette.panel_2;
    w.open.weak_bg_fill = palette.panel_2;
    w.open.bg_stroke = Stroke::NONE;
    w.open.fg_stroke = Stroke::new(metric::HAIRLINE, palette.ink);
    w.open.corner_radius = radius;

    // **Nothing may change size under the pointer**, and two separate things in
    // egui do.
    //
    // `expansion` draws the frame outside the rect that was allocated, so a
    // hovered control grows without moving its neighbours - a twitch rather than
    // a reflow.
    //
    // The outline is worse, and is why the bar still moved after the first of
    // these was zeroed: a button's inner margin is `button_padding + expansion -
    // bg_stroke.width` (`widget_style.rs`), so a state carrying a one-pixel
    // outline is two pixels narrower than one without, and every control after
    // it on the bar slides over. That the widgets here pass `Stroke::NONE`
    // themselves does not help, because the margin is computed from the style's
    // stroke and not from the one the button draws with.
    //
    // The design has no outline in any state and changes the fill alone, so both
    // are zero everywhere and the question does not arise.
    for state in
        [&mut w.noninteractive, &mut w.inactive, &mut w.hovered, &mut w.active, &mut w.open]
    {
        state.expansion = 0.0;
        state.bg_stroke = Stroke::NONE;
    }

    let spacing = &mut style.spacing;
    spacing.item_spacing = egui::vec2(metric::TIGHT, metric::TIGHT);
    spacing.button_padding = egui::vec2(10.0, 5.0);
    spacing.menu_margin = egui::Margin::same(6);
    spacing.interact_size.y = metric::CONTROL;
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
