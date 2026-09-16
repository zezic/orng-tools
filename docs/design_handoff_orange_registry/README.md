# Handoff: Orange Registry — custom content manager for Bitwig Studio

## Overview

Orange Registry (binary `orange-registry`) is a desktop utility that registers user-made
devices, modulators and Grid modules with a Bitwig Studio installation, so that projects
recall them reliably. It reads each document's identity (UUID), records it, modifies the
installation so it loads that entry list at startup, places the documents in the user
library, and writes descriptions and search keywords so the devices are findable in
Bitwig's browser.

This bundle documents the **UI** for that app: 19 states across one main window and three
full-window secondary screens, in dark and light appearance.

The design deliberately answers a handful of questions the spec left open. Those decisions
are recorded in **Design decisions** below; they are the parts most worth validating
against planned functionality, because each one encodes an assumption about behaviour.

## About the design files

The files in this bundle are **design references written in HTML** — prototypes that show
intended look and behaviour. They are **not production code to copy**. The app ships as a
native desktop binary (the working assumption in the spec is an immediate-mode Rust/egui
app), so the task is to **recreate these designs in the target environment** using its own
layout and styling primitives.

Two consequences worth stating plainly:

- The HTML uses flexbox, CSS grid, CSS custom properties, `position:sticky` and web fonts.
  None of that is a requirement — they are how the intent was expressed. The intent is the
  measurements, tones, type sizes and states documented here.
- The prototype is a **state gallery**, not a working app. A picker above the window
  switches between 19 predefined states. There is no real file I/O, no real installation
  detection, and the transitions between states are not the app's real transitions. Do not
  infer navigation from how the gallery switches states.

## Fidelity

**High-fidelity.** Colours, type sizes, spacing, surface levels and interaction states are
final and exact, and every value in this README was measured on the rendered design rather
than read off the source. Recreate the UI to these values, subject to one caveat: the
design was authored at CSS pixel sizes at a 820×560 logical window. Font sizes carry
fractional values (10.5px, 11.5px, 12.5px) that came from web-font metrics; round them to
whatever the target toolkit's type scale supports rather than reproducing the fraction.

Not final: the exact copy of long explanatory sentences (accurate in substance, but not
written by a technical writer), and icon choices beyond those named below.

## Window and layout frame

- Logical window **820 × 560**, resizable is untested — every state was designed at this size.
- Window corner radius 12px; no window border (the shell separates from the desktop by tone and shadow).
- A 30px title strip at the top of the prototype (`orange-registry`, centred, three dots at
  left) stands in for **native window chrome** — do not build it, use the platform's.
- **Left gutter: 12px.** Everything aligns to it: install-bar content, toolbar, list rows,
  section bands, action bar, and all three secondary screens.
- Secondary alignment columns: **25px** (status text, after a 5px dot + 8px gap) and
  **46px** (nothing lands here any more since the row index was removed; noted only because
  the install-bar title and kind column used to share it).
- The list body reserves **15px** for a vertical scrollbar; right-edge content
  (section counts, row action buttons) therefore sits 27px from the window's right edge
  while bars sit at 12px. Verify against the target toolkit's scrollbar metrics.

### Vertical stack of the main window (top to bottom)

| Region | Height | Surface |
|---|---|---|
| Install bar | ~70px (two rows, 11px/9px padding) | `--panel` |
| List toolbar | 42px | `--panel` |
| Section band (`Pending`) | 26px, `position:sticky` | `--row-alt` |
| Staged rows | 36px each | `--row` |
| Section band (`Registered`) | 26px, sticky | `--row-alt` |
| Registered rows | 36px each | `--row` |
| Banner (conditional) | content height, 12px padding | tone-tinted |
| Action bar | min 52px | `--panel` |

The detail inspector, when open, is a **272px** panel on the right side of the region
between the toolbar and the action bar (it does not overlay the install bar or action bar).

## Design tokens

Two themes. Every neutral is **equal-RGB** — no hue in any neutral surface or ink. Orange
is the only chroma, plus a red reserved for failure.

### Surfaces (dark → light)

| Token | Dark | Light | Used for |
|---|---|---|---|
| `--page` | `#000000` | `#e6e6e6` | Ground behind the window (gallery only) |
| `--bg` | `#0a0a0a` | `#f2f2f2` | Window body, list ground, modal header/footer bands |
| `--row` | `#0a0a0a` | `#f2f2f2` | List row default |
| `--panel` | `#161616` | `#fafafa` | Install bar, toolbar, action bar, cards, inspector header |
| `--row-alt` | `#1e1e1e` | `#e4e4e4` | Sticky section bands |
| `--panel-2` | `#1e1e1e` | `#ffffff` | Raised surfaces: inspector body, modal body |
| `--row-hover` | `#262626` | `#dcdcdc` | Row hover |
| `--btn` | `#282828` | `#e2e2e2` | Filled secondary controls, chips, hover targets |
| `--btn-hover` | `#333333` | `#d8d8d8` | Control hover |
| `--field` | `#000000` | `#ececec` | Input wells (search, inspector fields, path fields, report block) |

Surface steps are load-bearing: **there are almost no borders in this design.** Separation
comes from tone. The steps were tuned to stay perceptible — controls sit 18/255 above their
panel, the modal header band 20/255, section bands 20/255 (dark) and 14/255 (light),
row hover 28/255 (dark) and 22/255 (light). If the target toolkit renders these flat
colours differently, preserve the *steps*, not the literal hex.

### Ink

| Token | Dark | Light | Used for |
|---|---|---|---|
| `--ink` | `#f2f2f2` | `#171717` | Primary text: entry names, titles, headings |
| `--ink-2` | `#bcbcbc` | `#3e3e3e` | Secondary text, field labels, banner body |
| `--ink-3` | `#949494` | `#5e5e5e` | Tertiary: kind, UUID, reason, counts, paths, metadata |

Tinted variants exist so that grey text never sits on a coloured ground. Each holds the
neutral's luminance and pulls hue toward its background:

| Token | Dark | Light |
|---|---|---|
| `--ink-2-warm` | `#c6b6aa` | `#463c34` |
| `--ink-3-warm` | `#9e9089` | `#665c53` |
| `--ink-2-err` | `#c9b4b1` | `#453a38` |
| `--ink-3-err` | `#9d8c8a` | `#655a58` |

Applied to: banner body copy, authorisation notice, Restore warning body, and the secondary
content of any accent-tinted row (selected entry row's kind/reason/UUID, selected backup's
metadata, selected placement description).

### Accent and semantic

| Token | Dark | Light | Notes |
|---|---|---|---|
| `--accent` | `#ff5a1f` | `#e8500f` | **Fills only** — primary button, active chip, checked box, keyword pills |
| `--accent-text` | `#ff7a45` | `#9e3809` | **Text/outline only** — the fill orange cannot pass 4.5:1 as small text |
| `--accent-ink` | `#0a0a0a` | `#0a0a0a` | Text on an accent fill — near-black in both themes |
| `--accent-soft` | `rgba(255,90,31,.10)` | `rgba(232,80,15,.10)` | Tinted rows, warning grounds |
| `--err` | `#ff4036` | `#bf2a1c` | Error dots, destructive outline |
| `--err-text` | `#ff6259` | `#a32316` | Error text on tinted grounds |
| `--zebra` | `rgba(255,255,255,.05)` | `rgba(0,0,0,.045)` | Backup list banding only |
| `--scrim` | `rgba(0,0,0,.74)` | `rgba(255,255,255,.66)` | Overlay wash — **light theme's scrim is a light wash** |
| `--line` | `#242424` | `#c6c6c6` | Retained for the few semantic borders and the drag frame |

The accent split is the single most important token rule: **one orange for fills, a
different orange for text.** Using the fill orange as small text fails contrast, and using
the text orange as a fill looks muddy.

### Type

- **Inter** — all human-language text. Weights 400 / 500 / 600 (no 700 in the UI).
- **Iosevka Extended** — machine data only, always `letter-spacing: -0.05em`.
  Bundled in this handoff as `Iosevka-Extended.ttf` / `Iosevka-ExtendedBold.ttf`.

Mono is confined to: UUIDs, filesystem paths, build/revision strings, byte sizes, counts,
the diagnostics report, backup metadata, version strings, and the binary name. Everything a
human reads as language is Inter — including statuses, kind labels, filter labels, section
headers and buttons. This boundary is deliberate and worth preserving.

| Role | Size | Weight | Letter-spacing |
|---|---|---|---|
| Window/screen title | 13.5px | 600 | -0.02em |
| Entry name | 12.5px | 500 | -0.005em |
| Primary button | 12px | 600 | -0.01em |
| Body / secondary | 11.5px | 400 | normal |
| Status, kind, filters, section headers | 11px | 400 (500 for section headers) | normal |
| Field labels | 10.5px | 600 | normal |
| Tertiary, notes | 10.5px | 400 | normal |
| Mono data | 10–11px | 400 | -0.05em |

Minimum text size is 10px, all of it non-essential mono data; all text was verified at
**≥4.5:1** against its actual composited background in both themes.

### Radii and other values

- Buttons, chips, fields, badges: **3px**
- Cards, wells, segmented controls: **4px**
- Modals: **8px**; window: **12px**
- Status dots: 5px circles (6px in banners)
- Row action buttons: 22×22px hit area, 16px icon
- All Phosphor icons: **16px, duotone** — sized uniformly so they align on the pixel grid at
  1× displays. Large display glyphs use Phosphor **light**: 54px (empty states), 56px (drag
  overlay), 28px (About mark).
- Modal shadow: `0 30px 80px rgba(0,0,0,.85)` dark, `0 26px 60px rgba(0,0,0,.22)` light

## Screens / views

### 1. Entry list (main view)

**Purpose:** see what is registered, stage new documents, fix problems, apply changes.

**Install bar** (top, `--panel`, two rows):
- Row 1: installation name (13.5px/600) · build string (mono 10px, `--ink-2`) · full path
  (mono 10.5px, `--ink-3`, truncates with ellipsis, flexes) · `Change install` filled button
  with `ph-folder-open` · overflow button with `ph-dots-three-outline-vertical`.
- Row 2 (9px below, starting at the 12px gutter): status dot + registry state (11px) ·
  `Guard <armed|disarmed|unknown>` · `·` · `Backup <date>` (clickable, opens Restore).
- Registry states and their dot colours: `Registered <n>` accent dot / `--ink-2` text;
  `Stock` `--ink-3` dot; `Needs re-apply` accent dot and accent text; `Unknown build`
  `--ink-3`; `Modified elsewhere` `--err`.
- Overflow menu (190px, `--panel-2`, 6px radius, shadow): Settings · Restore backup… ·
  Open backups folder · **separator** · About Orange Registry.

**Toolbar** (42px, `--panel`): search well (`--field`, max 200px, flexes down to 64px,
`ph-magnifying-glass`, placeholder "Search name or UUID") · three kind filters
(`Devices <n>`, `Modulators <n>`, `Modules <n>` — label in Inter, count in mono `--ink-3`;
active = `--btn` fill, inactive = transparent with `--ink-3` text) · flexible gap ·
`Factory` checkbox (`ph-check-square`/`ph-square`, accent when checked) · `Add files…`
filled button with `ph-file-plus`.

The filters carry **no icons** — earlier icon versions implied meanings the glyphs did not
actually have. If space is tight, drop other labels before these.

**Row** (36px, 6-column grid, 12px gaps):

| Column | Width | Content |
|---|---|---|
| 1 | 66px | Kind — `Device` / `Modulator` / `Module`, 11px, `--ink-3` |
| 2 | `minmax(0,1fr)` | Name (12.5px/500, `--ink`) + reason (10.5px, `--ink-3`) |
| 3 | 106px | UUID first segment, mono, + `ph-copy` at 50% opacity |
| 4 | 130px | Status dot + status label |
| 5 | 84px | Action buttons, right-aligned, revealed on hover |

Inside column 2 the **name takes shrink priority** (`flex: 0 1 auto`) over the reason text
(`flex: 0 200 auto`) — otherwise a long reason squeezes a short name to four characters.

Narrow mode (inspector open, 4 columns: 66 / 1fr / 116 / 76): the UUID column and the
reason text are both dropped. The status label still names the problem and the inspector
carries the detail.

**Statuses** (dot colour / text colour):

| Status | Dot | Text | Meaning |
|---|---|---|---|
| `Staged` | accent | `--accent-text` | Dropped, not yet applied |
| `Registered` | `--ink-2` | `--ink-2` | Live in the installation |
| `Pending restart` | accent | `--ink-2` | Applied; Bitwig must relaunch to see it |
| `Changed` | accent | `--accent-text` | File on disk differs from the record |
| `Missing file` | `--err` | `--err` | Document not found in the user library |
| `Conflict` | `--err` | `--err` | UUID already in use |
| `Rejected` | `--err` | `--ink-3` | Not a Bitwig document; name also dims to `--ink-2` |
| `Pending removal` | `--ink-3` | `--ink-3` | Queued for removal; name gets `line-through` |
| `Factory` | transparent | `--ink-3` | Bitwig's own entry, read-only, whole row at 50% opacity |

`Changed` and `Missing file` are kept **separate** (per the spec's open question): same
remedy, different cause, and the cause is what the user needs in order to act.

**Row actions** (hover-revealed, plus a persistent overflow): `ph-folder-open` reveal ·
`ph-trash` remove (label "Cancel" for staged/conflict) · `ph-fingerprint` assign new UUID
(staged/conflict only) · `ph-file-magnifying-glass` locate (missing file only, accent
coloured) · `ph-arrow-u-up-left` undo (pending removal only) · `ph-dots-three-vertical`.

**Section bands** (26px, sticky, `--row-alt`): `Pending` in `--accent-text` /
`Registered` in `--ink-2`, both 11px/500 at the gutter, with a right-aligned mono count.
Staged rows are **pinned above** registered rows rather than sorted in.

**Action bar** (min 52px, `--panel`): status dot + summary (11.5px, tone-coloured) with an
optional second line note (10.5px, `--ink-3`, aligned to the 25px column); primary button
right. Summary tones: neutral `--ink-3` dot, warn accent, error `--err`, ok `--ink`.

**Primary button:** accent fill, `--accent-ink` text, 32px tall, 12px/600, label + trailing
icon (`ph-check` for update mode, `ph-arrow-right` for prepare mode). Disabled = `--btn`
fill, `--ink-3` text, no border, with the reason in a tooltip.

### 2. Detail inspector (272px slide-over)

Header (40px, `--panel`): kind label · entry name (12.5px/600, truncates) · `ph-x` close.
Body (`--panel-2`, 12px gutter, 14px between groups, scrolls):

- `Display name` — well, 27px tall, 12px text
- `Description` — well, min 44px, 11.5px/1.45, helper: "Shown under the device in Bitwig's browser."
- `Search keywords` — wrapping well of accent-filled pills (10.5px, `--accent-ink`, with a `×`), helper: "Proposed from the name."
- separator
- `UUID` — mono, `word-break: break-all`, with `ph-copy`
- `Registered library path` — mono, wraps
- `Placement` — pill: `linked` (`ph-link`, ok), `copied` (`ph-copy-simple`, info), `unresolved` (`ph-link-break`, err)
- separator
- Actions: `ph-folder-open` Reveal file · `ph-fingerprint` Assign new UUID… · `ph-trash` Remove entry (in `--err`)

Description and keywords are the reason this panel exists — they are what make a registered
device feel native in Bitwig's browser, so they are editable here with proposed defaults.

### 3. Empty states

All three centre in the list region, `box-sizing: border-box`, 20px/34px padding. The two
full-page variants carry **11px corner crop marks** inset 16px, in `--line`.

- **Nothing registered** — `ph-light ph-tray-arrow-down` 54px accent; "Drop a device here
  to register it"; body naming what the app does and that Bitwig must be closed for the
  first run; the three accepted extensions in mono separated by `·`; `Add files…` primary;
  footnote "A backup is written before anything is changed".
- **No install found** — `ph-light ph-folder-dashed` 54px `--ink-3`; lists the three
  searched locations; `Locate Bitwig Studio…` primary + `Copy diagnostics` secondary;
  footnote "The installation root contains bitwig.jar". The install bar degrades to
  "No installation selected" in `--ink-3`.
- **No match** — `ph-light ph-funnel-x` 36px; 13px title; `Clear filters` secondary. The
  toolbar stays in place above it.

### 4. Drag-over overlay

The **whole window** is the drop target. `--scrim` wash; 1px dashed accent frame inset 12px;
12px accent corner marks inset 20px; `ph-light ph-tray-arrow-down` 56px; title
"Drop to stage N documents" (17px/600); then a per-file list on `--panel` rows (7px/10px
padding, 1px apart): an accent mono ordinal + the filename in mono. Rejected files get a
transparent row, `——` in place of the ordinal, `line-through` on the name, and `ignored` at
the right. A dropped folder shows a file count. Accepted extensions repeat in mono at the
bottom.

Accept/reject is resolved and shown **before** the drop. A dropped folder is read one level
deep.

### 5. Banner (conditional, above the action bar)

Full-width, 12px padding, tinted ground (`--warn-bg` / `--err-bg` / `--ok-bg`), no border
and **no accent side-bar**: a 6px tone dot, title (12px/600, `--ink`), body (10.5px/1.55 in
the matching tinted ink), optional outlined button in the tone colour, optional `ph-x`
dismiss. Five uses:

| State | Tone | Title | Notes |
|---|---|---|---|
| Bitwig running | warn | "Quit Bitwig Studio before preparing the installation." | `Check again`; blocks prepare mode only |
| Needs re-apply | warn | "A Bitwig update reset this installation." | `What changed?`; states entries are kept |
| Success | ok | "Start Bitwig Studio. Your devices are in the browser." | dismissible |
| Restart needed | ok | "Restart Bitwig Studio to see your changes." | dismissible |
| Failure | err | "Verification failed. The Core Registry did not match what was written." | `Copy details`, dismissible |

### 6. Plan confirmation (modal, 476px)

`--panel-2` body with `--bg` header and footer bands. Header: accent dot · title
"Prepare this installation" · `Plan` label right. Body: one framing sentence; then the plan
as numbered lines (mono ordinal + 11px/1.5 text, the installation-modifying line's ordinal
in `--accent-text`); then a `ph-square` checkbox "Also delete the document file of removed
entries" with a sub-note; then a closing note that a Bitwig update resets the installation.
Footer: `Cancel` filled + `Prepare installation` accent.

**Only prepare mode confirms.** Update mode applies directly.

### 7. Apply progress (modal, 436px)

Header: accent dot · "Preparing the installation" · sub-line "Step N of 9 · <step name>".
Optional authorisation notice (`--accent-soft` ground, warm-tinted body, no side bar).
Then nine steps, 27px each: mono ordinal · 5px state dot · label (600 when active) ·
right-aligned mono meta. Footer: 2px progress track (`--line` with an accent fill) ·
percentage in mono · `Cancel`.

The nine steps: Back up the installation (238 MB) · Read the Core Registry (428 entries) ·
Prepare the installation · Neutralise the tamper guard · Place documents (6 files) ·
Link library folders · Write descriptions and search keywords · Verify · Activate.

Cancel is offered up to Activate. Update mode shows **none** of this — it is a transient
line in the action bar.

### 8. Settings (full window)

Header 44px: `ph-arrow-left` + `Entries` back control · `ph-gear-six` · "Settings".
Body, 12px gutter, max 620px, sections 15px apart, each with a 10.5px/600 `--ink-2` label:

- **Paths** — three rows on a `128px / 1fr / auto / auto` grid: Bitwig install, User
  library, Backups. Value in a mono `--field` well; `Browse`/`Open` filled button;
  `ph-arrow-counter-clockwise` reset (absent for Backups).
- **Document placement** — two radio rows (`ph-radio-button` / `ph-circle`), the selected
  one on an accent-tinted ground with warm-tinted description ink. Link (documents survive
  a Bitwig update) vs copy (an update removes them).
- **Appearance** — System / Light / Dark segmented control on a `--field` ground with
  `ph-desktop` / `ph-sun` / `ph-moon`.
- **Diagnostics** — one line of explanation, `Copy report` button, and a mono report block
  in a `--field` well (8 lines: install, version, jar, registry, guard, locale, license,
  entries).
- **About** row — clickable card with `ph-info`, version in mono, `ph-caret-right`.

One screen, no tabs. Diagnostics live here because that is what a user sends when a new
Bitwig build is not recognised.

### 9. Restore backup (full window)

Header adds an `Open backups folder` button. A warning card (`--warn-bg`, warm-tinted body,
`ph-warning`): "Restoring removes every registration from this installation." plus the
consequence — projects will not recall custom devices afterwards, though documents and the
app's own record are kept. Then the backup list: radio rows with date (12px/500), a
`Latest` accent-tinted pill on the newest, mono metadata, mono size at right; selected row
accent-tinted with warm-tinted metadata; alternate rows carry `--zebra`. Footer: a mono-free
note, `Cancel`, and `Restore this backup` as an **outlined** `--err` button — deliberately
not a filled one, so the destructive action does not compete with the accent and so its
label keeps contrast in both themes.

### 10. About (full window)

52px accent-filled mark with `ph-light ph-package` in `--accent-ink`; product name
(19px/600) and a mono line `0.9.2 · orange-registry · macOS arm64`; one sentence of
description; a **Detected installation** card (Version / Build / Verified against, labels
in Inter, values in mono); a disclaimer card with `ph-scales` stating no affiliation with
Bitwig GmbH and that the tool modifies a local installation at the user's discretion;
`Copy diagnostics` and `Licences` buttons.

## Interactions & behaviour

- **Row hover** reveals the action cluster and lifts the row to `--row-hover`. Factory rows
  do not respond. Every row responds identically — no banding, because a zebra stripe and a
  hover tone cannot both be legible in the same ramp.
- **Row click** opens the inspector, which narrows the row grid (see above).
- **Two apply modes, one button** whose label names the mode:
  - *Update entries* — cheap, no confirmation, Bitwig may stay open. Action-bar note says
    so. Produces `Pending restart` statuses and the restart banner.
  - *Prepare install* — modifies the installation. Confirms first, writes a backup, runs
    the nine-step transaction, requires Bitwig closed.
- **Overflow menu** is the only route to Settings, Restore and About; each replaces the
  whole window and returns via the `Entries` back control.
- **Drag** resolves accept/reject per file before the drop.
- Animations were **not specified** in the prototype. Suggested: no transition on hover
  tone (instant), and a short fade for the drag overlay. Nothing in the design depends on
  motion.

## State management

Per installation: detected path, version, build/revision, registry state (one of the five
listed), tamper-guard state, latest backup timestamp.

Per entry: kind, display name, UUID, library path, placement (linked/copied/unresolved),
description, keywords, status, and a reason string for the statuses that carry one.

Session/UI: search query, three kind filters, factory-visible flag, selected entry,
inspector open, overflow menu open, pending-apply mode, transaction step and result,
appearance preference.

Transitions worth validating: staged → registered (via apply); registered → pending restart
(update mode while Bitwig runs); registered → needs re-apply (Bitwig updated underneath);
staged → conflict (UUID collision found); any → failure (verification fails, backup
restored, nothing changed).

## Design decisions to validate

These are the design's answers to the spec's open questions — the parts most likely to need
a functional check:

1. **One Apply button, mode in the label** rather than two affordances.
2. **`Changed` and `Missing file` kept as separate statuses** rather than merged.
3. **Needs re-apply is a full-width banner**, and no row is marked broken — the entries are
   fine, the installation is not.
4. **Staged rows pinned above** the registered list rather than sorted into it.
5. **Update mode never blocks on Bitwig running**, so the restart banner is mandatory —
   otherwise a user who cannot find the device assumes the app failed.
6. **Secondary screens replace the window**; no tabs, no modals for Settings/Restore/About.
7. **The destructive restore is outlined, not filled** — one filled colour per view.
8. Row indices and decorative ordinals were **removed**: a positional number is not an
   identity, and the UUID already identifies a row.

## Assets

- **Phosphor Icons** 2.1.1, duotone (all 16px UI icons) and light (large display glyphs),
  loaded from `unpkg.com/@phosphor-icons/web`. Named glyphs are listed per screen above.
- **Inter** via Google Fonts, weights 400/500/600.
- **Iosevka Extended** — user-supplied TTFs, bundled here as `Iosevka-Extended.ttf` and
  `Iosevka-ExtendedBold.ttf` (only the 400 weight is actually used). Note this is the
  *Extended* width variant, which is **not** available from public CDNs — the plain-width
  Iosevka on npm/jsDelivr is a different, narrower face.
- No images, illustrations or raster assets. No Anthropic brand assets.

## Files

Design source in this bundle. Each is a self-contained HTML file that opens directly in a
browser; child files also render standalone.

| File | Contents |
|---|---|
| `Orange Registry.dc.html` | Root: window shell, all 19 states, state picker, theme toggle, drag overlay, both modals, banner |
| `EntryRow.dc.html` | One list row: 9 statuses, 3 kinds, hover/selected/factory/narrow variants |
| `InstallBar.dc.html` | Install bar: 5 registry states, guard, backup, overflow menu |
| `ListToolbar.dc.html` | Search, kind filters, factory toggle, Add files |
| `ActionBar.dc.html` | Summary, note, primary/secondary actions, both apply modes |
| `Inspector.dc.html` | 272px detail panel |
| `EmptyState.dc.html` | Three empty variants |
| `SettingsScreen.dc.html` | Settings |
| `RestoreScreen.dc.html` | Restore backup |
| `AboutScreen.dc.html` | About |
| `support.js` | Prototype runtime — **not part of the design**, required only to open the HTML |

Open `Orange Registry.dc.html` and use the picker above the window to reach any state; the
`Dark`/`Light` toggle beside the title switches appearance. The child files are the same
components in isolation, each with editable properties.

Note: the child components define only the **dark** palette. The light theme is declared in
the root file, so a child opened on its own always renders dark.
