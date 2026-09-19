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
    /// Under the overflow menu, which floats over the window without a scrim
    /// and needs an edge of its own to sit on.
    ///
    /// The design writes this one as a literal rather than as a token, so it
    /// states only the dark value: `rgba(0,0,0,.7)`. The light value is the
    /// same shadow under the ratio the design's own two `--shadow` tokens
    /// state between the themes, .85 to .22, which is the nearest thing the
    /// bundle says about how a shadow behaves in light.
    pub menu_shadow: Color32,
    /// Under the inspector, which slides over the right of the list rather than
    /// beside it. Written as a literal in the design too, and given the light
    /// value the same way [`Palette::menu_shadow`] is.
    pub panel_shadow: Color32,
    /// The row the inspector is about. The design writes this one as a literal
    /// as well - the accent at nine per cent, which is a shade softer than the
    /// `--accent-soft` token beside it - so that a selected row and a hovered
    /// row cannot be confused for each other.
    pub row_selected: Color32,
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
        menu_shadow: hexa(0x000000, 179),
        panel_shadow: hexa(0x000000, 128),
        row_selected: hexa(0xff5a1f, 23),
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
        menu_shadow: hexa(0x000000, 46),
        panel_shadow: hexa(0x000000, 33),
        row_selected: hexa(0xe8500f, 23),
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
    /// A control in an empty state. The design draws both of the pair at the
    /// primary's height, so an empty state does not borrow a bar's 26-tall
    /// button and stand it beside a 32-tall one.
    pub const EMPTY_CONTROL: f32 = 32.0;
    /// From an empty-state control's edge to its label. The action is padded
    /// wider than the alternative beside it, and the design states both.
    pub const EMPTY_ACTION_PAD: f32 = 14.0;
    pub const EMPTY_ALT_PAD: f32 = 12.0;
    /// A control drawn as an outline rather than as a fill, which the design
    /// uses for the one thing a banner offers.
    pub const OUTLINED: f32 = 28.0;

    /// The overflow menu, measured off the bundle with the menu forced open.
    ///
    /// The design states `min-width:190px` on a box that is not
    /// `border-box`, so 190 is the content and the menu draws 200 across: four
    /// of padding and one of line on each side. Both numbers are here because
    /// the items are laid out against the first and the anchor against the
    /// second.
    pub const MENU: f32 = 190.0;
    pub const MENU_MARGIN: i8 = 4;
    /// One line of it. Taller than its text, because what sets the height is
    /// the sixteen-pixel icon beside the text with six above and below.
    pub const MENU_ITEM: f32 = 28.0;
    pub const MENU_PAD_X: f32 = 8.0;
    pub const MENU_PAD_Y: f32 = 6.0;
    /// From a menu item's icon to its label. Wider than a bar's gap: a menu is
    /// read down a column rather than scanned across a row.
    pub const MENU_GAP: f32 = 9.0;
    /// The rule between groups of items, inset from the menu's own padding.
    pub const MENU_RULE_INSET: f32 = 2.0;
    pub const MENU_RULE_GAP: f32 = 4.0;
    /// Below the overflow control, and past its right edge: the design hangs
    /// the menu off the bar rather than off the control, so its right edge is
    /// eight from the window where the control's is twelve.
    pub const MENU_DROP: f32 = 5.0;
    pub const MENU_OVERHANG: f32 = 4.0;
    /// The menu's shadow, `0 18px 40px` in the design.
    pub const MENU_SHADOW_DROP: i8 = 18;
    pub const MENU_SHADOW_BLUR: u8 = 40;

    /// Corner of a control. The design rounds by three, not by six: at these
    /// sizes a six-pixel radius reads as a pill rather than as a soft corner.
    pub const RADIUS: u8 = 3;
    /// Corner of a surface that floats over the window. The menu is the one
    /// thing the design rounds by six, and it is not a control.
    pub const MENU_RADIUS: u8 = 6;
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
    /// The same two columns beside the inspector, where the identity has gone
    /// and the row has 272 fewer pixels to spend. The design shortens these
    /// rather than letting the name take the whole difference.
    pub const NARROW_STATUS_COLUMN: f32 = 116.0;
    pub const NARROW_ACTIONS_COLUMN: f32 = 76.0;
    /// The author and the version of a catalog item, which take the place of
    /// the identity and the status a registered row carries.
    pub const AUTHOR_COLUMN: f32 = 116.0;
    pub const VERSION_COLUMN: f32 = 56.0;
    pub const CATALOG_STATUS_COLUMN: f32 = 142.0;
    pub const CATALOG_ACTIONS_COLUMN: f32 = 92.0;
    /// The same two beside the detail panel, where the author and the version
    /// have gone the way the identity goes on a registered row.
    pub const NARROW_CATALOG_STATUS_COLUMN: f32 = 108.0;
    pub const NARROW_CATALOG_ACTIONS_COLUMN: f32 = 122.0;
    /// A catalog row carries a description under the name, so it is taller than
    /// a registered one.
    pub const CATALOG_ROW: f32 = 48.0;
    /// A panel over the right of the list: the inspector in Local, the item's
    /// detail in Catalog. Everything under here is shared by both, read off
    /// `Inspector.dc.html` and `CatalogDetail.dc.html` - which agree on all
    /// three - and off the shell probed with the panel open.
    pub const ASIDE: f32 = 272.0;
    /// Its header, which names what the panel is about and closes it.
    pub const ASIDE_HEADER: f32 = 40.0;
    /// From the edge of the panel's body to its fields. Wider down than across,
    /// which is the design's own pair and not a symmetric margin.
    pub const ASIDE_PAD_Y: f32 = 13.0;
    /// Between two groups of fields.
    pub const BETWEEN_GROUPS: f32 = 14.0;
    /// Between a field's label and the field, which is looser than the same gap
    /// in the block of facts below: a field is a thing to fill in and the facts
    /// are a thing to read.
    pub const UNDER_A_FIELD_LABEL: f32 = 5.0;
    pub const UNDER_A_FACT: f32 = 3.0;
    /// Between two facts.
    pub const BETWEEN_FACTS: f32 = 7.0;
    /// A field holding one line.
    pub const FIELD: f32 = 27.0;
    pub const FIELD_PAD: f32 = 8.0;
    /// Corner of a field, which the design rounds by four where it rounds a
    /// control by three.
    pub const FIELD_RADIUS: u8 = 4;
    /// A field holding a sentence, at its shortest: it grows with what is
    /// written in it.
    pub const PARAGRAPH: f32 = 44.0;
    pub const PARAGRAPH_PAD_Y: f32 = 6.0;
    /// The box the search keywords sit in, and the chips inside it. The box's
    /// height is the design's own `min-height`, which is on the content: a
    /// single row of chips is shorter than that and the box does not shrink
    /// to it.
    pub const KEYWORDS: f32 = 27.0;
    pub const KEYWORDS_PAD_X: f32 = 6.0;
    pub const KEYWORDS_PAD_Y: f32 = 5.0;
    pub const KEYWORD: f32 = 18.0;
    pub const KEYWORD_PAD_X: f32 = 6.0;
    pub const BETWEEN_KEYWORDS: f32 = 4.0;
    /// Inside a chip, from its icon to its word.
    pub const ALONG_A_CHIP: f32 = 4.0;
    /// The chip stating where a document actually is. One pixel taller than a
    /// keyword, because the design pads it by one and a half rather than by
    /// two, around a sixteen-pixel icon rather than around text.
    pub const PLACEMENT: f32 = 19.0;
    pub const PLACEMENT_PAD_X: f32 = 6.0;
    /// One line of the inspector's action list. Taller than its text for the
    /// same reason a menu item is: the sixteen-pixel icon beside it.
    pub const PANEL_ACTION: f32 = 26.0;
    pub const PANEL_ACTION_PAD_X: f32 = 7.0;
    /// From a panel action's icon to its label.
    pub const ALONG_A_PANEL_ACTION: f32 = 7.0;
    /// The catalog's detail panel, which is the inspector's opposite number and
    /// is measured a shade tighter: its groups are 13 apart where the
    /// inspector's are 14, and the row of an icon and its line is 8 where the
    /// inspector's source row is 7. Read off the shell probed with it open.
    pub const BETWEEN_DETAIL_GROUPS: f32 = 13.0;
    pub const ALONG_A_DETAIL_LINE: f32 = 8.0;
    /// The panel that says a newer device has replaced this one: a wash, and a
    /// control to go and look at it.
    pub const NOTICE_PAD_X: f32 = 11.0;
    pub const NOTICE_PAD_Y: f32 = 10.0;
    pub const IN_A_NOTICE: f32 = 6.0;
    /// A control inside one, which is shorter than a bar's.
    pub const NOTICE_CONTROL: f32 = 26.0;
    pub const NOTICE_CONTROL_PAD: f32 = 10.0;
    /// The inspector's shadow, `-18px 0 40px` in the design: cast to the left,
    /// because the panel is over the list rather than beside it.
    pub const PANEL_SHADOW_REACH: i8 = -18;
    pub const PANEL_SHADOW_BLUR: u8 = 40;

    /// A full-window surface behind the overflow: Settings, Restore, About.
    ///
    /// Each is `width:100%; height:100%` on the page colour with a header of its
    /// own, so it replaces the install bar and the action bar as well as the
    /// page. The header is two pixels taller than the install bar it stands in
    /// for, which is the design's own number and not a rounding: it carries a
    /// 24-tall control with ten above and below.
    pub const SCREEN_HEADER: f32 = 44.0;
    /// Between the header's own items: the back control, the rule, the icon and
    /// the title. Wider than a toolbar's eight and tighter than a bar's twelve.
    pub const SCREEN_HEADER_GAP: f32 = 9.0;
    /// The rule between what you came from and where you are.
    pub const SCREEN_RULE: f32 = 16.0;
    /// The back control: a 16-pixel arrow, the view you came from, and padding
    /// that is not symmetric - the design pulls the left side in to six so the
    /// arrow sits where the window's own edge padding puts everything else.
    pub const BACK: f32 = 24.0;
    pub const BACK_PAD_LEFT: f32 = 6.0;
    pub const BACK_PAD_RIGHT: f32 = 9.0;
    /// From the arrow to the word.
    pub const ALONG_A_BACK: f32 = 6.0;

    /// Inside a screen: from the window's edge to the column of groups, and the
    /// air above and below it. The design's own asymmetric trio, and the bottom
    /// is deeper than the top so a scrolled-to-the-end column does not end
    /// against the sill.
    pub const SCREEN_PAD_X: f32 = 12.0;
    pub const SCREEN_PAD_TOP: f32 = 16.0;
    pub const SCREEN_PAD_BOTTOM: f32 = 20.0;
    /// How wide the column of groups ever gets. A `max-width`, so a wider window
    /// leaves the settings where they are rather than stretching a path field
    /// across it.
    pub const SCREEN_COLUMN: f32 = 620.0;
    /// Between two groups of settings. A third value for `rule`, beside the
    /// inspector's 14 and the catalog detail's 13.
    pub const BETWEEN_SETTINGS_GROUPS: f32 = 15.0;
    /// Between a group's heading and the box under it.
    pub const UNDER_A_GROUP_HEADING: f32 = 7.0;
    /// A group's box: the panel colour, the field radius, and padding the design
    /// varies by what is in it - eleven around rows of controls, five around a
    /// list of choices that have a wash of their own.
    pub const GROUP_PAD_X: f32 = 12.0;
    pub const GROUP_PAD_Y: f32 = 11.0;
    pub const CHOICES_PAD: f32 = 5.0;
    /// Between two rows inside a group, and between two choices - which touch,
    /// because a choice is a band of colour and two bands need only a seam.
    pub const BETWEEN_GROUP_ROWS: f32 = 9.0;
    pub const BETWEEN_CHOICES: f32 = 1.0;

    /// The label column of a path row, which is fixed so the three paths line up
    /// down the group rather than each starting after its own word.
    pub const PATH_LABEL_COLUMN: f32 = 128.0;
    /// Between the cells of one.
    pub const ALONG_A_PATH_ROW: f32 = 8.0;
    /// The box a path is written in. Shorter than a field in the inspector,
    /// because it is a value to read rather than a box to type in.
    pub const PATH_FIELD: f32 = 23.0;
    pub const PATH_FIELD_PAD_X: f32 = 8.0;
    /// A control inside a group: `Browse`, `Restore...`, `Copy report`. Two
    /// pixels shorter than a bar's, around the same 16-pixel icon.
    pub const GROUP_CONTROL: f32 = 24.0;
    pub const GROUP_CONTROL_PAD_X: f32 = 8.0;
    /// The one that carries a wider label, which the design pads by nine.
    pub const WIDE_CONTROL_PAD_X: f32 = 9.0;
    /// From a group control's icon to its label.
    pub const ALONG_A_GROUP_CONTROL: f32 = 5.0;
    /// The control that puts a path back to what was auto-detected: an icon and
    /// nothing else, and the one box in the design that is not square.
    pub const RESET: [f32; 2] = [26.0, 25.0];

    /// One choice in a list of them: a radio mark, a line, and a sentence under
    /// it. Padding the design states across the box, which is why the height is
    /// what the content comes to rather than a number.
    pub const CHOICE_PAD_X: f32 = 9.0;
    pub const CHOICE_PAD_Y: f32 = 8.0;
    /// From the mark to the words. The mark is nudged one down, so that a
    /// 16-pixel glyph sits on the line of the 11.5 text beside it rather than
    /// above it.
    pub const ALONG_A_CHOICE: f32 = 8.0;
    pub const CHOICE_MARK_DROP: f32 = 1.0;
    /// Between a choice's line and the sentence explaining it.
    pub const UNDER_A_CHOICE: f32 = 2.0;
    /// The row a checkbox makes, which is a choice without the list around it:
    /// the group's own padding, and the same gap from mark to words that a list
    /// of choices uses plus the one the box would have added.
    pub const ALONG_A_CHECK: f32 = 9.0;

    /// The appearance switch: three segments in a well of the page colour, which
    /// is what says they are one control rather than three buttons.
    pub const SEGMENTS: f32 = 28.0;
    pub const SEGMENTS_PAD: f32 = 2.0;
    pub const SEGMENT: f32 = 24.0;
    pub const SEGMENT_PAD_X: f32 = 10.0;
    pub const BETWEEN_SEGMENTS: f32 = 2.0;
    /// From a segment's icon to its word.
    pub const ALONG_A_SEGMENT: f32 = 5.0;
    /// The row it sits in, which is shallower than a group of rows: the switch
    /// carries its own height and needs no air of its own.
    pub const SWITCH_PAD_Y: f32 = 9.0;

    /// The diagnostics report: a block of monospaced lines meant to be copied
    /// whole. How far apart they sit is leading rather than a margin, and is
    /// [`Leading::Reporting`](super::font::Leading::Reporting).
    pub const REPORT_PAD_X: f32 = 9.0;
    pub const REPORT_PAD_Y: f32 = 8.0;
    /// Between the sentence saying what the report is for and the report.
    pub const ABOVE_A_REPORT: f32 = 8.0;

    /// The row at the foot of Settings that leads to About: one line, so it is
    /// shallower than a group and has no heading over it.
    pub const SCREEN_LINK: f32 = 34.0;
    pub const ALONG_A_SCREEN_LINK: f32 = 8.0;

    /// The mark beside a banner's headline.
    pub const DOT: f32 = 6.0;
    /// The search field, measured across the whole box - the glyph, the gap and
    /// the text - because that is what the bundle's is measured across. The
    /// design writes these as a `max-width` and a `min-width` on the text area
    /// inside, at 200 and 64, plus eight of padding on each side.
    pub const SEARCH_FIELD: f32 = 216.0;
    pub const SEARCH_FLOOR: f32 = 80.0;
    /// A kind chip's padding, which the design tightens beside the inspector.
    pub const CHIP_PAD: f32 = 9.0;
    pub const NARROW_CHIP_PAD: f32 = 7.0;
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
    /// What is written in a field. The same size as [`ACTION`], which is why
    /// the two track alike, and a different job: one is pressed and the other
    /// is read.
    pub const FIELD_VALUE: f32 = 12.0;
    /// A reason, a note, anything explaining the line above it.
    pub const NOTE: f32 = 10.5;
    /// The smallest run the design writes: the sentence under a field saying
    /// what Bitwig does with it, and the word inside a placement chip.
    pub const FOOTNOTE: f32 = 10.0;
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

    /// How tightly the design sets its monospaced runs. Every Iosevka run in
    /// the bundle carries this, at every size, without exception - 41 of them.
    const MONO_EM: f32 = -0.05;

    /// How much taller than its size the design sets a run that wraps.
    ///
    /// Leading rather than a margin, and therefore here rather than in `metric`.
    /// A gap between two labels is something a layout puts in; this is inside one
    /// run of text and only the run can carry it.
    ///
    /// **Not derivable from the size, which is why this is not [`tracking`].**
    /// The bundle writes six different `line-height` values, and the same size
    /// takes different ones: 11.5px is set on 1.55 in the catalog detail's
    /// description and on nothing at all where it is one line in a row. So the
    /// call site names what the run *is* and this names the number - the same
    /// division `metric` makes everywhere else.
    ///
    /// Only the roles this window draws are here. A seventh value in the bundle
    /// that nothing draws yet is not a variant until something draws it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub enum Leading {
        /// A sentence explaining the line above it, inside a group.
        Explaining,
        /// The sentence inside a notice - the catalog detail's superseded block.
        Noticing,
        /// A paragraph of the author's own prose, which is the longest wrapping
        /// run the window has and the one where the leading compounds most.
        Describing,
        /// The diagnostics block, looser still, so that a column of monospaced
        /// lines reads across as well as down.
        Reporting,
    }

    impl Leading {
        /// The bundle's own number, one per role.
        const fn ratio(self) -> f32 {
            match self {
                // `Inspector.dc.html`, `SettingsScreen.dc.html`.
                Leading::Explaining => 1.45,
                // `CatalogDetail.dc.html:33`.
                Leading::Noticing => 1.5,
                // `CatalogDetail.dc.html:46`.
                Leading::Describing => 1.55,
                // `SettingsScreen.dc.html`, the report block.
                Leading::Reporting => 1.65,
            }
        }

        /// The line box for a run of this size, in points.
        pub const fn over(self, size: f32) -> f32 {
            size * self.ratio()
        }
    }

    /// A run that wraps, set on the design's leading for what it is.
    ///
    /// Beside [`run`] rather than a flag on it, because leading is not a property
    /// of the face - see [`Leading`]. A run that does not wrap wants none of
    /// this and calls `run`.
    pub fn wrapping(text: impl Into<String>, size: f32, leading: Leading) -> egui::RichText {
        run(text, plain(size)).line_height(Some(leading.over(size)))
    }

    /// The tracking the design states for a run in this face at this size, in
    /// points.
    ///
    /// **Derived from the font rather than repeated at the call site.** Both
    /// the family and the size are already in a [`FontId`], so the one place
    /// that knows what a run is set in is also the place that can say how
    /// tightly - which is why `run` below takes the font and nothing else.
    ///
    /// The design is not consistent about the proportional face and there is no
    /// formula to find: it tracks 12 at -0.01em, 12.5 at -0.005, 13.5 at
    /// -0.015, 14 and 16.5 at -0.02, and leaves 11, 11.5 and 13 alone entirely.
    /// So these are transcribed, and a size the design says nothing about is
    /// set with nothing rather than interpolated into.
    pub fn tracking(font: &FontId) -> f32 {
        let same = |a: f32, b: f32| (a - b).abs() < f32::EPSILON;
        if font.family == FontFamily::Monospace {
            return MONO_EM * font.size;
        }
        // The icon face is a set of glyphs rather than a run of letters, and
        // the design tracks none of them.
        if !matches!(font.family, FontFamily::Proportional | FontFamily::Name(_))
            || same(font.size, ICON)
        {
            return 0.0;
        }
        let em = if same(font.size, ACTION) {
            -0.01
        } else if same(font.size, ROW_NAME) {
            -0.005
        } else if same(font.size, INSTALL_TITLE) {
            -0.015
        } else if same(font.size, DIALOG_TITLE) || same(font.size, HEADING) {
            -0.02
        } else {
            0.0
        };
        em * font.size
    }

    /// One run of a [`LayoutJob`](egui::text::LayoutJob), in the given face.
    ///
    /// The counterpart to `run` for the places that build a job by hand, so
    /// those cannot lose the tracking either. Spread it and override what else
    /// the run needs: `TextFormat { color, valign, ..font::format(f) }`.
    pub fn format(font: FontId) -> egui::TextFormat {
        egui::TextFormat {
            extra_letter_spacing: tracking(&font),
            font_id: font,
            ..Default::default()
        }
    }

    /// A run of text in the given face, set as the design sets it.
    ///
    /// Use this rather than `RichText::new(..).font(..)`: the tracking then
    /// cannot be left off, because there is no call site left that chooses a
    /// font without also getting one.
    pub fn run(text: impl Into<String>, font: FontId) -> egui::RichText {
        let tracking = tracking(&font);
        egui::RichText::new(text).font(font).extra_letter_spacing(tracking)
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

    // **Labels here are not selectable text, and that is what makes a row
    // clickable.** egui's default adds `Sense::click_and_drag()` to every
    // label so it can be dragged over and copied, which puts a click target on
    // top of whatever the label was drawn inside. A row senses its own click
    // and the name on it is a label, so a press on the one thing in a row
    // anybody aims at landed on the label and went nowhere - while a press on
    // the empty half of the same row opened the inspector. Nothing in this
    // window is selectable text: what can be copied says so and copies on a
    // click, which is a smaller promise kept properly.
    style.interaction.selectable_labels = false;

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

    /// Each leading is the bundle's own ratio, and the line boxes it makes at
    /// the sizes drawn on it are the bundle's own pixels.
    ///
    /// Each side is written as the bundle writes it - the size and the ratio,
    /// multiplied - rather than as the decimal they come to. `11.5 * 1.55` is
    /// 17.824999 in `f32` and not 17.825, and a test that had to know that would
    /// be testing the arithmetic rather than the design.
    #[test]
    fn every_leading_is_the_one_the_bundle_states() {
        use font::Leading;
        assert_eq!(Leading::Explaining.over(font::NOTE), 10.5 * 1.45);
        assert_eq!(Leading::Noticing.over(font::NOTE), 10.5 * 1.5, "CatalogDetail.dc.html:33");
        assert_eq!(
            Leading::Describing.over(font::CONTROL),
            11.5 * 1.55,
            "CatalogDetail.dc.html:46"
        );
        assert_eq!(Leading::Reporting.over(font::MONO_TIGHT), 10.0 * 1.65);

        // The two the design deliberately sets apart. A description is the
        // author's own prose and the design gives it more air than the sentence
        // inside a notice, so collapsing the two onto one number would be a
        // quiet loss rather than a visible one.
        assert!(
            Leading::Describing.over(font::CONTROL) > Leading::Noticing.over(font::CONTROL),
            "a description is set looser than a notice"
        );
    }
}
