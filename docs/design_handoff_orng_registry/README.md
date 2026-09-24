# Handoff: ORNG Registry — custom content manager for Bitwig Studio

> **Revision 8.** Answers `design-round-4-changes.md`; the replies are in
> `design-round-5-answers.md`, which is the thing to read. New components: `UpdateModal`,
> `RenameDialog`. New props: `elevate` (shield) on ActionBar, CatalogRow, RestoreScreen,
> EntryRow; `menuOpen`/`conflict` on EntryRow; `waiting`/`confirmSave` on Inspector;
> `installed` on SettingsScreen. New tokens `--shadow-menu`, `--shadow-panel`. 39 states.
> Where this README and the answers disagree, the answers win.

> **Revision 7.** Applies `design-round-3-changes.md`, all six items. No design value moved:
> the app root is now `~/.orng/`, the product name is **ORNG Registry**, the guard reads as
> three states with no version string, the stale name-matching provenance rule is gone, the
> review link is a commit, and the compatibility/migration passage revision 6 added has been
> deleted — nothing was ever released, so there is nothing to migrate.

## Naming

The project was renamed **orange-registry → orng**, because `orng.tools` was available as a
domain. One convention settles it everywhere downstream:

| | |
|---|---|
| **User-facing text** | **`ORNG`, uppercase** — "ORNG Registry", "ORNG Catalog". Read as an abbreviation, not a word |
| **Machine identifiers** | **`orng`, lowercase** — binary, paths, packages, domain, repositories |

So: `ORNG Registry` in the About heading, in prose, in documentation and in the window
title; `orng-registry` in About's mono identity line; `~/.orng/` in Settings;
`orng.tools` as the domain; `orng-catalog` in a provenance string.

| | |
|---|---|
| Binary / package name | `orng-registry` |
| App root | `~/.orng/` — entry list `~/.orng/entries.tsv`, backups `~/.orng/backups/<version>-<short revision>/` |
| Window title | `ORNG Registry` (Inter 11px/500, centred; the strip stands in for native chrome — do not build it) |
| About's mono identity line | `0.9.2 · orng-registry · macOS arm64` — lowercase, correct as drawn |
| Catalog repository | `orng-catalog` |
| Domain | `orng.tools` |

**The root is `~/.orng/`, not `~/.orng/`.** `.orng` is compiled into the Java class
preparation injects into `bitwig.jar`, which reads the entry list at every launch from a
path it holds as a literal — so the root is not a preference the UI may vary. It is also
deliberately not named for the app: it holds backups of the **installation**, which are
Bitwig's files, so one root named for the project holds both.

**There is nothing to migrate.** Nothing has ever been released, so no machine holds an
`~/.orange-registry/` directory and no installation was ever prepared by an
`orange-registry` build. There is no compatibility state to draw and no first-launch
migration screen.

> **Revision 5.** Adds the ORNG Catalog feature (per `ui-spec-catalog.md`), the
> `Local`/`Catalog` switch, a single-row install bar, and dot-free status treatment.
> Supersedes the two-row install bar, the `ViewSwitch` strip and the "Entries" name
> described in earlier revisions. See **For this audit round** immediately below for what
> changed and where the risk sits.

## Overview

ORNG Registry (binary `orng-registry`) is a desktop utility that registers user-made
devices, modulators and Grid modules with a Bitwig Studio installation, so that projects
recall them reliably. It reads each document's identity (UUID), records it, modifies the
installation so it loads that entry list at startup, places the documents in the user
library, and writes descriptions and search keywords so the devices are findable in
Bitwig's browser.

This bundle documents the **UI** for that app: **39 states** across two primary views
(`Local` and `Catalog`), three full-window secondary screens, two modals and a drag
overlay, in dark and light appearance.

**This revision incorporates the implementation review** (`design-review.md`): the backup
figure, diagnostics contents, About's detected-installation rows, the removal default, the
derived step count, the gated UUID action, factory-row treatment and the new Unknown build
state all reflect what the library can actually report.

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

## Applied from round three

`design-round-2-changes.md`, all five items:

1. **Three paths corrected** — entry list `~/.orng/entries.tsv` (outside the
   installation, so a Bitwig update cannot replace it, and tab-separated because the injected
   class parses it with `split("\t")` at every launch); backups
   `~/.orng/backups/<version>-<short revision>/`; archive
   `Contents/Java/bitwig.jar`, not `Contents/Resources`.
2. **The transaction is five steps, reordered** so nothing in the installation changes until
   the patched archive verifies. Cancel copy and the failure message follow from it.
3. **Provenance keys on UUID**, not display name.
4. **Kept as drawn**, pending index and entry-list additions: the per-item link to the merged
   change, and the two version fields the update modal needs.
5. **No change** — the design's assumption that preparation skips the link step under `Copy`
   placement was right, and the implementation now matches it.

## Still invented

Everything else in the data is now confirmed by the implementation team: 35 MB backup, 34 MB
archive, 428 factory entries as 152 / 43 / 233, and `6.1 (94a90411)` as the build-string
shape. What remains fictional is the sample content itself — the nine catalog items, their
authors and review references, the eleven local entries, and the three backup timestamps.

## For the previous audit round

Revision 4. Since the last audit the app gained the **ORNG Catalog** (a second primary
view, ten new states, four new components) and lost a good deal of chrome. The corrections
from `design-review.md` are all applied and are marked as decisions 9–12 below.

**Where the risk is concentrated.** Ten review rounds on this revision found the same defect
class over and over: *a surface asserting something the data does not support.* Every one was
a hardcoded value that survived next to real data — a panel that described one entry while
showing another's provenance, band counts that counted a different set from the rows beneath
them, a filter that highlighted itself without filtering, a confirmation naming the wrong
device, a diagnostics report claiming every anchor resolved on the screen that exists because
they did not. If you audit one thing, audit that: **for each stated fact, ask which input it
derives from, and what happens when that input changes.**

Specific claims worth testing against the real implementation:

1. **The two views must agree.** A Local entry is catalog-sourced iff a catalog item **with
   the same UUID** reports `Installed`, `Superseded` or `Update available` — never a name
   match; names are allowed to collide in the repository, and a name collision in the
   Local list is a `Conflict` the user resolves with Rename. The rule is stated in full under *What the Local view gains*.
2. **Per-kind library folders.** The design states `devices/My Devices/*.bwdevice`,
   `modulators/My Modulators/*.bwmodulator`, `modules/My Modules/*.bwmodule`. Confirm those
   are the real paths and the real extensions.
3. **Status gating** is tabulated under the inspector. Confirm the remedies are right —
   particularly that `Missing file` offers Locate and not Reveal, and that a staged document
   has an identity but no library path yet.
4. **The catalog sample is nine items** and every count in the window derives from it. The
   real catalog will hold hundreds; check that nothing in the spec assumes a small index
   (the facet counts, the "N of M shown" summary, the single-file index fetch).
5. **Version reporting** is one format, `6.1` + `94a90411`, suppressed entirely when
   resolution fails. Confirm the library can always produce both, and that `—` is the right
   thing to show when it cannot.
6. **Update vs supersede.** The design assumes the repository guarantees that an update keeps
   the parameter set, and that anything else is published under a new identity. The entire
   distinction — one confirmation, one offer — rests on that guarantee holding.
7. **Numbers that are still invented:** the nine apply steps and their metadata (35 MB backup,
   428 factory entries, 6 files placed), the diagnostics report's jar size, the three backup
   sizes, and the catalog items themselves. Correct them or confirm them.

Open questions the design answered by choosing, and would revisit on request: where the
`Local`/`Catalog` switch lives (decision 13), whether a catalog row needs its inline
description (14), and how loudly `Superseded` should read (15).

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
- A 30px title strip at the top of the prototype (`ORNG Registry`, centred, three dots at
  left) stands in for **native window chrome** — do not build it, use the platform's.
- **Left gutter: 12px.** Everything aligns to it: install-bar content, toolbar, list rows,
  section bands, action bar, and all three secondary screens.
- **There are no secondary alignment columns.** Earlier revisions had one at 25px (status
  text, after a dot) and one at 46px (a row-index column); removing the dots and the index
  put every left edge on the 12px gutter, including the action-bar summary and its note.
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
neutral's luminance and pulls hue toward its background. **They are only for tinted grounds**
— on a neutral surface they read as a hue with no cause, and the app's own grading rule
applies instead: `--ink-2` for emphasis, `--accent-text` only when a decision is pending.

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
- Tone dots: 6px circles, **banners only** — the lists, install bar and action bar have none
- Row action buttons: 22×22px hit area, 16px icon
- All Phosphor icons: **16px, duotone** — sized uniformly so they align on the pixel grid at
  1× displays. Large display glyphs use Phosphor **light**: 54px (empty states), 56px (drag
  overlay), 28px (About mark).
- Modal shadow: `0 30px 80px rgba(0,0,0,.85)` dark, `0 26px 60px rgba(0,0,0,.22)` light
- Menu shadow `--shadow-menu`: `0 18px 40px rgba(0,0,0,.7)` dark, `0 12px 28px rgba(0,0,0,.16)` light
- Panel shadow `--shadow-panel`: `-18px 0 40px rgba(0,0,0,.5)` dark, `-14px 0 32px rgba(0,0,0,.12)` light

## Screens / views

### 1. Entry list (main view)

**Purpose:** see what is registered, stage new documents, fix problems, apply changes.

**Install bar** (top, `--panel`, two rows):
The install bar is **one 42px row** (9px padding), left to right:

- `Local` / `Catalog` switch — two tabs, active one filled `--btn` with `--ink` text at 500,
  inactive `--ink-3`. This is the app's only top-level navigation, and it exists because the
  app has two peer activities: manage what is registered, and discover what is available.
- Installation name (13.5px/600), build hash (mono 10px, `--ink-2`, tooltip `Build <hash>`),
  full path (mono 10.5px, `--ink-3`, flexes and truncates).
- **Registry state, shown only when it is not `Registered`** — `Stock`, `Needs re-apply`
  (`--accent-text`), `Unknown build`, `Modified elsewhere` (`--err-text`). When the
  installation is simply registered the chip says nothing, so it is omitted.
- **Catalog view only:** index freshness (`--ink-3`, or `--accent-text` when stale) and a
  refresh control — icon-only when current, a labelled `Refresh` button when stale.
- `Change install` filled button with `ph-folder-open`, then the overflow control.

**One version format throughout:** the title carries the version (`Bitwig Studio 6.1`), the
chip carries the build hash (`94a90411`). The library reports `6.1 (94a90411)`; there is no
`rev`-timestamp form. When resolution fails the title drops to `Bitwig Studio` and the chip
is suppressed — that state means the installation could not be read, so it claims no version.

An earlier revision had a second row carrying the registered count, tamper-guard state and
backup date. It was removed: the count duplicated the list's own `Registered` band (and
disagreed with it), the guard is an implementation detail with no user action attached, and
the backup date was only a route to Restore. Guard and backup now appear in **Settings** —
the diagnostics report and the Backups row — where they are actually consulted, and both
carry a no-backup form for a stock installation.

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
| 4 | 130px | Status label (colour-graded, no dot) |
| 5 | 84px | Action buttons, right-aligned, revealed on hover |

Inside column 2 the **name takes shrink priority** (`flex: 0 1 auto`) over the reason text
(`flex: 0 200 auto`) — otherwise a long reason squeezes a short name to four characters.

Narrow mode (inspector open, 4 columns: 66 / 1fr / 116 / 76): the UUID column and the
reason text are both dropped. The status label still names the problem and the inspector
carries the detail.

**Statuses.** There are **no status dots anywhere** — in the lists, the install bar or the
action bar. A dot beside a label repeats what the label says, and an orange dot made healthy
states read as alerts. Colour alone carries the grade, and orange is reserved for states
that want a decision:

| Status | Colour | Meaning |
|---|---|---|
| `Registered` | `--ink-3` | Live in the installation |
| `Factory` | `--ink-3` | Bitwig's own entry, read-only |
| `Pending removal` | `--ink-3` | Queued for removal; name gets `line-through` |
| `Rejected` | `--ink-3` | Not a Bitwig document; name dims to `--ink-2`, and **kind and UUID both render as an em dash** — the file was never read, so it has neither |
| `Staged` | `--ink-2` | Dropped, not yet applied |
| `Pending restart` | `--ink-2` | Applied; Bitwig must relaunch to see it |
| `Changed` | `--accent-text` | File on disk differs from the record |
| `Update available` | `--accent-text` | Catalog has a newer version of this identity |
| `Missing file` | `--err-text` | Document not found in the user library |
| `Conflict` | `--err-text` | UUID already in use |

Grey means settled or inert, neutral means in flight with nothing to decide, orange means a
decision is waiting, red means broken. Banners keep a 6px tone dot, because there the colour
*is* the message and no status label is doing the work.

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
| `Factory` | transparent | `--ink-3` | Bitwig's own entry, read-only — see note below |

`Changed` and `Missing file` are kept **separate** (per the spec's open question): same
remedy, different cause, and the cause is what the user needs in order to act.

**Factory rows are not dimmed.** An earlier revision put the whole row at 50% opacity; that
measured 4.34:1 on the secondary cells and no usable opacity cleared the floor. Read-only is
signalled instead by the absent hover response, the `Factory` status itself and the
separate `Factory` band it sits under. Those cells measure 6.53:1.

**Row actions** (hover-revealed, plus a persistent overflow): `ph-folder-open` reveal ·
`ph-trash` remove (label "Cancel" for staged/conflict) · `ph-fingerprint` assign new UUID
(staged/conflict only) · `ph-file-magnifying-glass` locate (missing file only, accent
coloured) · `ph-arrow-u-up-left` undo (pending removal only) · `ph-dots-three-vertical`.

**Section bands** (26px, sticky, `--row-alt`, 11px/500 at the gutter, right-aligned mono
count). Three of them, in DOM order: `Pending` in `--accent-text`, `Registered` in
`--ink-2`, and `Factory` in `--ink-3` — the last present only while factory entries are
shown. Staged rows are **pinned above** registered rows rather than sorted in.

Two rules that are easy to get wrong and were both bugs during design:

- **Every band count describes the rows beneath it**, not a global figure. The `Registered`
  count excludes factory rows (they have their own band) and is not the install bar's
  registration count — those legitimately differ, since an entry queued for removal is still
  registered until Apply runs.
- **Band `z-index` must ascend in DOM order** (Pending 1, Registered 2, Factory 3). They are
  sticky siblings in one scroll container, so with descending or equal z-index the first band
  stays pinned forever and ends up labelling rows from a later section.

**Action bar** (min 52px, `--panel`): summary (11.5px, tone-coloured) with an optional
second line note (10.5px, `--ink-3`), both starting on the 12px gutter; primary button right.
No dot. Summary tone colours: neutral and ok `--ink-2`, warn `--accent-text`, error
`--err-text`.

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
- `Registered library path` — mono, wraps. **Folder and extension derive from the kind**,
  since each kind has its own library folder: `devices/My Devices/<name>.bwdevice`,
  `modulators/My Modulators/<name>.bwmodulator`,
  `modules/My Modules/<name>.bwmodule`. Reporting everything as a `.bwdevice` contradicts
  the row's own kind column, the accepted-extensions line on the onboarding state, and the
  per-kind folder creation that `design-review.md` item 6 requires.
- For a `Rejected` entry, **UUID and library path are replaced by an `Identity` row**
  reading "Not read. This file is not a Bitwig document, so it has no UUID and nothing was
  placed in the library." A rejected file has neither, so showing an all-zeros UUID and a
  path states two things that are not true
- `Placement` — pill: `linked` (`ph-link`, ok), `copied` (`ph-copy-simple`, info), `unresolved` (`ph-link-break`, err). **Shown only for entries that have actually been placed** — a `Staged` or `Conflict` document has not been placed at all yet (its path row says "Will be placed at"), so reporting a placement would state a settled fact that is not one, and would contradict the current placement setting whenever they differ. Registered entries keep their real historical value, which legitimately varies per entry.
- separator
- Actions: `ph-folder-open` Reveal file · `ph-fingerprint` Assign new UUID… (**staged and
  conflicting entries only**, same restriction as the row) · `ph-trash` Remove entry (in
  `--err`, tooltip "The document file is kept")

A UUID is how a saved project finds a device. Reassigning one that is already registered
breaks every project referencing it silently, so the restriction holds wherever the action
appears.

**The panel's status rules are the row's status rules**, not a second set. All of it derives
from the entry's status:

| Status | Path row | Actions |
|---|---|---|
| `Registered`, `Pending restart`, `Changed`, `Update available` | `Registered library path` | Reveal file · Remove entry |
| `Staged`, `Conflict` | **`Will be placed at`**, in `--ink-3`, plus "Nothing is written until Apply runs." | Assign new UUID… · Remove entry |
| `Missing file` | `Registered library path` | **`Locate file…`** (in `--accent-text`) · Remove entry — no Reveal, because the file cannot be found |
| `Rejected` | replaced by the `Identity` row; **Description and Search keywords are dropped too**; no `Placement` | Remove entry |

Getting this wrong is not cosmetic: during design the panel offered `Reveal file` on a
missing file (the one action that cannot work) while omitting `Locate file` (the one that
fixes it), and reported a "registered library path" for documents that Apply had not yet
placed.

Description and keywords are the reason this panel exists — they are what make a registered
device feel native in Bitwig's browser, so they are editable here with proposed defaults.

### 3. Empty states

All three centre in the list region, `box-sizing: border-box`, 20px/34px padding. The two
full-page variants carry **11px corner crop marks** inset 16px, in `--line`.

- **Nothing registered** — `ph-light ph-tray-arrow-down` 54px accent; "Drop a device here
  to register it"; body naming what the app does and that Bitwig must be closed for the
  first run; the three accepted extensions in mono separated by `·`; then an aside —
  "Nothing of your own yet? The catalog has devices, modulators and Grid modules you can
  install in one click." — and **`Browse the catalog` as the accent primary** with
  `Add files…` secondary; footnote "A backup is written before anything is changed".

  The catalog is the call to action rather than Add files: a user on this screen has nothing
  of their own to add, so the friendlier path is the one that gives them something.
- **No install found** — `ph-light ph-folder-dashed` 54px `--ink-3`; lists the three
  searched locations; `Locate Bitwig Studio…` primary + `Copy diagnostics` secondary;
  footnote "The installation root contains bitwig.jar". The install bar degrades to
  "No installation selected" in `--ink-3`.
- **Unknown build** — `ph-light ph-question` 54px `--ink-3`, crop marks; "This Bitwig
  installation could not be read"; body explaining that the app finds what it needs by
  structure rather than by version number, that this installation is arranged in a way it
  does not recognise, that this usually means a new Bitwig release, and that **nothing has
  been changed**; `Copy diagnostics` primary + `Change install…` secondary; footnote "The
  diagnostics report names what was looked for and what was found". The install bar shows
  the `Unknown build` badge with no version, and the action bar reads "Installation not
  recognised" with the primary disabled.

  This is the state a user reaches the morning after a Bitwig release. It is nobody's
  fault and it is most likely to be met by someone not expecting it, so the copy must not
  read like an error the user caused. It is a recognised condition, not a crash.
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
in `--accent-text`); then a closing note that a Bitwig update resets the installation.
Footer: `Cancel` filled + `Prepare installation` accent.

The modal carries **no** delete-the-document checkbox. Removal is Update-mode work, which
applies without confirmation, so a checkbox here would be unreachable in exactly the case
it was written for; the choice lives in Settings instead.

**Only prepare mode confirms.** Update mode applies directly.

### 7. Apply progress (modal, 436px)

Header: accent dot · "Preparing the installation" · sub-line "Step N of <total> · <step name>".
Optional authorisation notice (`--accent-soft` ground, warm-tinted body, no side bar).
Then nine steps, 27px each: mono ordinal · 5px state dot · label (600 when active) ·
right-aligned mono meta. Footer: 2px progress track (`--line` with an accent fill) ·
percentage in mono · `Cancel`.

The five steps: **Back up the archive and the description bundles** (35 MB) · **Prepare the
installation** · **Verify** · **Activate** · **Link library folders**.

Only `Contents/Java/bitwig.jar` (34 MB) and the three description bundles are copied — never
the ~957 MB installation, and the backup goes to
`~/.orng/backups/<version>-<short revision>/`, outside the installation so a
Bitwig update cannot reach it.

**The order is the property the transaction exists to provide.** The patched archive is
written beside the original, verified under Bitwig's own JVM, and moved into place by a
single rename. Nothing in the installation changes until it verifies, so a failure anywhere
earlier leaves an installation that was never touched. Two consequences the copy must carry:

- **Cancel is genuinely clean** up to Activate — "Nothing in the installation has changed
  yet, so Cancel leaves it untouched: there is nothing to undo." An earlier draft ordered
  document placement before Verify, which meant a cancelled run had already copied the
  user's documents and rewritten Bitwig's description bundles.
- **A failure restores nothing**, because nothing was touched: "Your installation was not
  changed." Restore stays what it is on its own screen — something the user asks for, to undo
  a preparation that succeeded.

Placing documents and writing descriptions are **not in the transaction** — they are *Update
entries* work, touching no part of the archive and unable to disturb the tamper seal. Reading
the registry is not a step either: it happens while the plan is computed, before the user has
agreed to anything.

**The step count is derived, not fixed:** "Link library folders" does not run under the *copy
documents into the installation* placement setting (a linked folder would resolve straight
back out into the user library, making the setting meaningless), and renders with an em-dash
ordinal and "not run" while the header and percentage fall back to four steps.

**Cancel is offered up to Activate, and removed at it.** Up to that point the note reads
"Nothing in the installation has changed yet, so Cancel leaves it untouched — there is
nothing to undo"; at Activate the control is **gone** and the note reads "The verified
archive has been moved into place, so there is nothing left to cancel. Undoing a completed
preparation means Restore." A live Cancel button beside a caption saying cancelling is no
longer possible is worse than no button.

Update mode shows **none** of this — it is a transient line in the action bar.

### 8. Settings (full window)

Header 44px: `ph-arrow-left` + `Local` back control · `ph-gear-six` · "Settings".
Body, 12px gutter, max 620px, sections 15px apart, each with a 10.5px/600 `--ink-2` label:

- **Paths** — three rows on a `128px / 1fr / auto / auto` grid: Bitwig install, User
  library, Backups. Value in a mono `--field` well; `Browse` filled button and a
  `ph-arrow-counter-clockwise` reset on the first two. The Backups row instead carries
  `Restore…` (with `ph-clock-counter-clockwise`), or the text "No backup yet" when the
  installation has never been prepared.
- **Document placement** — two radio rows (`ph-radio-button` / `ph-circle`), the selected
  one on an accent-tinted ground with warm-tinted description ink. Link (documents survive
  a Bitwig update) vs copy (an update removes them).

  **This setting is app state, not screen state**, and three surfaces derive from it: the
  diagnostics `placement` line, the plan's placement line, and whether the transaction runs
  four steps or five. Held in the root and passed down, like the kind filters and the install
  filter — a placement control that only tints its own row is the defect this design hit
  three times in review.
- **Appearance** — System / Light / Dark segmented control on a `--field` ground with
  `ph-desktop` / `ph-sun` / `ph-moon`.
- **Removing entries** — a `ph-square` checkbox "Also delete the document file", **off by
  default**, with the consequence spelled out: removing an entry unregisters it and leaves
  the document in the library. This is the only home for that choice (see decision 9).

  Like placement, this is **app state held in the root**, and both remove affordances name
  the setting in force: the row's trash tooltip reads "Remove entry · the document file is
  kept" or "…is deleted too", and the inspector's Remove matches. A fixed tooltip would
  promise the opposite of the setting half the time.
- **Diagnostics** — one line of explanation, `Copy report` button, and a mono report block
  in a `--field` well: install path, version and build, archive path and size, which anchors
  resolved, **guard state** (see below), **backup date and size**, entry-list location and count,
  placement strategy, factory breakdown.

  **Every variable line derives from the installation the screen was opened from** — none of
  it is fixed text. From a prepared install: `anchors registry ok · descriptions ok · library
  ok`, `guard disarmed`, `backup 14 Sep 2026 · 35.1 MB`,
  `entries … · 6 entries`. From a stock one: `guard armed · installation not prepared`,
  `backup none yet`, `entries … · none recorded`. From `Unknown build`:
  `version —  (unresolved)`, `archive … not read`, `anchors registry NOT LOCATED`, `guard unknown · guard site not recognised · preparation refuses`, `factory not read · the registry anchor was not located`. Version, build, archive size and the factory breakdown all come through the registry anchor, so when it does not resolve the report says so rather than restating the last known figures — the install bar suppresses the version in the same state (decision 11), and the two surfaces must not disagree. This matters more here than
  anywhere else in the app — the report is the artefact a user sends to get a new Bitwig
  build supported, and `Copy diagnostics` is the primary action of the state that most
  needs it, so a report that claims every anchor resolved would deny the failure it exists
  to describe. Guard and backup moved here when the install bar
  lost its second row; both derive from the installation, so a stock one reports
  `guard armed · installation not prepared` and `backup none yet`, and the Backups row
  reads "No backup yet" instead of offering `Restore…`. There is deliberately **no licence line** — the entitlement tier is assembled
  at Bitwig startup and nothing static in the installation exposes it.
- **The tamper guard is three states, and nothing is written into the installation.** The
  guard is a bytecode edit: it compiles to a value load, a comparison and a branch, and
  disarming replaces the load with a zero so the comparison always takes the normal path.
  The state is read back by reading that instruction — there is no marker, no version and no
  name, so the app cannot know who disarmed an installation or with what, and the diagnostics
  line reads `guard disarmed` and nothing more. It is also not how "already prepared" is
  decided.

  | State | Line | Meaning |
  |---|---|---|
  | `armed` | `armed · installation not prepared` | Untouched — the normal state of an installation that was never prepared |
  | `disarmed` | `disarmed` | Prepared |
  | `unknown` | `unknown · guard site not recognised · preparation refuses` | The guard site is present but not in a shape the app recognises |

  `unknown` is the state with a consequence and must be drawn as itself: preparation
  **refuses** on it rather than editing blind. It is what a Bitwig release that changed the
  guard looks like, so it is the state a user meets on upgrade day — Settings reporting
  `armed` there would send them to press Prepare and collect an unexplained refusal. In the
  prototype it is the `Settings · unknown guard` state, reached from the picker.

- **About** row — clickable card with `ph-info`, version in mono, `ph-caret-right`.

One screen, no tabs. Diagnostics live here because that is what a user sends when a new
Bitwig build is not recognised.

### 9. Restore backup (full window)

**Two forms.** With no backup — a stock installation that has never been prepared — the
region carries an empty state: `ph-light ph-archive` 54px, "Nothing to restore yet", a body
explaining that a backup is written just before the installation is prepared, a note that
backups hold the jar and description bundles (~35 MB, not the installation), a footer reading
"No backup exists for this installation." and an inert primary. The warning card and the list
appear only when a backup exists. No placeholder date is ever shown.

Header adds an `Open backups folder` button. A warning card (`--warn-bg`, warm-tinted body,
`ph-warning`): "Restoring removes every registration from this installation." plus the
consequence — projects will not recall custom devices afterwards, though documents and the
app's own record are kept. Then the backup list: radio rows with date (12px/500), a
`Latest` accent-tinted pill on the newest, mono metadata in the `6.1 (94a90411) · jar +
description bundles` form, mono size at right (~33–35 MB, not the installation size); selected row
accent-tinted with warm-tinted metadata; alternate rows carry `--zebra`. Footer: a mono-free
note, `Cancel`, and `Restore this backup` as an **outlined** `--err` button — deliberately
not a filled one, so the destructive action does not compete with the accent and so its
label keeps contrast in both themes.

### 10. About (full window)

**Version, Build and Resolution all derive from the installation**, like the Settings
report. When resolution fails, Version and Build render as `—` and Resolution reads
"Anchors not located" in `--err-text` — About must not report a confident version in the
state where the install bar deliberately suppresses one.

52px accent-filled mark with `ph-light ph-package` in `--accent-ink`; product name
(19px/600) and a mono line `0.9.2 · orng-registry · macOS arm64`; two sentences of
description, the second stating that the app finds what it needs by structure rather than
by version; a **Detected installation** card with three rows — **Version** (`6.1`),
**Build** (`94a90411`) and **Resolution**, a dot plus "All anchors located" in `--accent`
or "Anchors not located" in `--err`; a disclaimer card with `ph-scales` stating no
affiliation with Bitwig GmbH and that the tool modifies a local installation at the user's
discretion; `Copy diagnostics` and `Licences` buttons.

There is **no "verified against" or supported-builds row, and the UI must not imply one.**
The app locates what it needs by structure, which is what lets an unseen Bitwig release
work on day one and a genuinely changed one fail loudly instead of mispatching. The
Resolution row is how that is reported, and it has both a success and a failure form.

## The Catalog view

**ORNG Catalog** is a public, curated repository of community devices, modulators and Grid
modules that the app can install directly. Two facts shape the design: items are 20–30 KB so
installing is effectively instant and needs no download manager, and **an item is executable
DSP** — the repository's review process is the only trust boundary, so the author is visible
before installing, not buried in a detail panel.

### Toolbar (42px)

Parallel to the Local toolbar, with one deliberate difference: **search covers name, author,
description and keywords**, and says so in the placeholder ("Search name, author, description
or keywords", 320px; "Search" when the panel narrows it). Local searches name and UUID —
different fields, because the question is different: *what is this thing I have* versus *is
there a thing that does X*.

Then the three kind filters with facet counts, and an **install filter** as a three-state
segmented control — `All` / `Installed` / `Updatable` — not a checkbox, because
`Updatable` is the state a returning user wants.

### Row (48px)

Six columns: kind (66) · name + one-line description (1fr) · author (116) · version (56,
mono) · status (142) · action (92). Narrow mode drops author and version, which the open
panel is already showing.

A catalog row leads with **what it is and who made it** — the reverse of a Local row, which
leads with kind and identity. The description is inline and clamped to one line: without it,
browsing means opening every item; clamped, rows stay even. The author sits in `--ink-2`,
brighter than version or description, because it is the trust signal.

**There is no UUID in a catalog row.** It identifies a thing you already have; it does not
help you choose one. It lives in the panel.

Actions are `--btn`-filled, never accent — the action bar's primary stays the only accent
fill in the window. `Update` takes `--accent-text`, the two failures `--err-text`.

### Statuses

| Status | Colour | Action |
|---|---|---|
| `Available` | `--ink-3` | `Install` |
| `Installed` | `--ink-2` | none; the name dims to `--ink-2` |
| `Update available` | `--accent-text` | `Update` |
| `Replacement available` | `--ink-2` | `See replacement` |
| `Needs Bitwig <version>` | `--ink-3` | none; states the version it needs |
| `Download failed` | `--err-text` | `Retry` |
| `Verification failed` | `--err-text` | `Copy details` |

### Item detail (272px, same panel as the Local inspector)

Author and version, full description, **Requires Bitwig \<version\> or newer** stated whether
or not it is satisfied, licence, the keywords that become search terms once installed, the
homepage if the author gave one, and **provenance — a link to the commit that published the
item**. That link is what makes review the trust boundary rather than a claim about one.
UUID last, secondary. Primary action, plus `Remove` when installed.

**The provenance link is a commit, not a pull request.** The index is generated from git
history on merge, and a commit is what that history can name; a PR number survives only as
text inside a commit message. Per item the index carries the forty lowercase hex digits of
the publishing commit:

| | |
|---|---|
| Label | `orng-catalog@<first seven characters>` — e.g. `orng-catalog@3f9a1c2` |
| Links to | `https://github.com/zezic/orng-catalog/commit/<the forty>` |

Seven characters occupy about the width the earlier `#412` form did, so the layout does not
move. Both panels render it as `Reviewed in orng-catalog@3f9a1c2` with the full URL on the
element's tooltip (`provenanceUrl` prop). The entry list also records the version an item
was installed at, so the update modal's `installed 2.0.3 → catalog 2.1.0` has both numbers
behind it.

### Install, update, supersede

The plan's placement line **derives from the staged items' kinds** — "1 to devices/My
Devices, 1 to modules/My Modules" — because one kind folder cannot hold documents of two
kinds, and the inspector states the same per-kind destination one panel away.

- **Install** is one click. On a prepared installation it is *Update entries* work: no
  backup, no confirmation, Bitwig may stay open, takes effect next launch.
- **Update confirms**, and installs do not. Bitwig resolves a device by identity, so
  replacing the file changes every project that already uses it. The modal names the item
  and the target version in its title ("Update BREATH FOLLOWER to 2.1.0?"), states the pair
  in mono beneath the lead sentence ("installed 2.0.3 → catalog 2.1.0"), and leads with the
  one sentence that matters — "Projects that already use this device will use the new
  version." — then explains that the catalog only permits updates which keep the parameter
  set, so those projects still load. An updatable item therefore needs **two** version
  fields in the data: the installed one and the catalog one.
- **Supersede** is an offer, not a debt. An incompatible revision is published as a **new
  item with a new identity**; the old one stays published. The row reads
  `Replacement available` in neutral grey with no badge — quieter than `Update available` —
  and the panel carries the counterintuitive part: a newer version exists as a separate
  device, installing it leaves your projects alone, both can be installed at once.

  **Both items must exist in the catalog, and the offer must navigate.** The design shows
  `CURVECOMP` (installed, 0.9.0, `Replacement available`) and `CURVECOMP II` (2.0.0,
  `Available`, its own UUID, keywords and review reference) as separate rows, and
  `See CURVECOMP II` in the panel opens that item. Naming a replacement the catalog does not
  list makes the coexistence claim unverifiable — which is the one thing this state has to
  get across.

### Network and trust states

- **Offline with a cached index** is a degraded state, not an error: **no banner**. The age
  is stated next to the install path ("Catalog from 12 days ago"), turning `--accent-text`
  and growing a labelled `Refresh` once stale. Browsing and installing cached items work.
- **Never fetched** gets a full-region empty state explaining that the index is one small
  file and caches once fetched, with `Try again`.
- **Download failed** is ordinary: per-item, `Retry`.
- **Verification failed is a trust event and must not read like a network error.** The file
  does not match the hash the catalog states, so the install is **refused**, the row holds
  the failure, and the only action is `Copy details` — deliberately **not** `Retry`. The
  banner says nothing was written to the library or the installation. The design shows both
  failures in one view so they visibly differ.

### What the Local view gains

- **Provenance per entry**: a `ph-duotone ph-package` marker on catalog-sourced rows
  (tooltip `ORNG Catalog · <version>`) and a **Source** block in the inspector with the
  review link. Local-file entries show `Local file`.

  **The sample data must not collide on UUID.** Only four pairs share an identity, each a
  local entry and its own catalog item: DISPERSER, CURVECOMP, BREATH FOLLOWER and
  SLEW LIMITER. Three accidental collisions in an earlier revision (a Device sharing a
  Modulator's UUID, and so on) were invisible only because the colliding catalog items were
  not installed — one status flip would have attributed another author's version and review
  link to an unrelated entry, and a staged document duplicating a catalog identity should
  have rendered as `Conflict` rather than a clean `Staged`. If you regenerate fixtures,
  check uniqueness rather than eyeballing them.

  **Provenance is derived, and keyed on UUID.** An entry is catalog-sourced when a catalog
  item **with the same UUID** reports an installed-ish status (`Installed`, `Superseded` or
  `Update available`) — so the two views cannot disagree about what is installed. It must not
  key on the display name: names are allowed to collide in the repository, and a collision in the
  Local list is a `Conflict` the user resolves with Rename — the name is not the identity. A UUID is exact, is
  already in both records, and cannot be renamed — name matching would eventually mark the
  wrong row, and would do it first to the user who had just renamed one. Building
  this from a hand-maintained name list means Local and Catalog drift apart the first time a
  catalog status changes, which is exactly what happened during design: the catalog claimed
  an item was installed that the Local list did not contain, and a superseded item lost its
  marker.

  Likewise the **inspector must be given the entry that was clicked** — name, kind, UUID,
  status, description, keywords, library path, placement and provenance. If it falls back to
  defaults it will show one entry's identity beside another's provenance, and the
  staged-only `Assign new UUID` gating will evaluate against the wrong row. (Note the
  attribute cannot be called `name` at the mount — that names the component to import.)
- `Update available` as an additional status.

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
  whole window and returns via the `Local` back control.
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

Added by the implementation review:

9. **The delete-the-document choice lives in Settings**, not on the apply confirmation,
   because removal is Update-mode work and never confirms. Default: keep the file — the
   document is the user's own work, the registration is not. The row action names the
   default in its tooltip.
10. **The step count is derived** rather than fixed at nine, because "Link library folders"
    does not run under copy placement.
11. **Version reporting is a single format** (`6.1` + `94a90411`) and is **suppressed
    entirely when resolution fails** — a confident version on an unreadable installation
    is worse than none.
12. **`Unknown build` is a designed, full-region state**, not an empty list under a grey
    badge.

Added by the catalog round:

13. **Two top-level views, one two-item switch** (`Local` / `Catalog`), living in the install
    bar rather than its own strip. The app has two peer activities; a switch is the smallest
    affordance that admits it. The toolbar could not host it — with search, three kind
    filters and the install filter, the catalog toolbar already needs ~844px of 796.
14. **A catalog row is not an entry row.** It leads with what the item is and who made it,
    carries a clamped one-line description, and omits the UUID entirely.
15. **Supersede reads as an offer**, in neutral grey, quieter than an update.
16. **Updates confirm; installs do not** — an update reaches backwards into saved projects.
17. **Verification failure is a trust event**, refusing the install and offering only
    `Copy details`, never `Retry`.
18. **No status dots**, and orange is reserved for states awaiting a decision.
19. **Every overflow screen reports the installation it was opened from**, never fixed
    text: Settings' four variable report lines, About's Version / Build / Resolution, and
    whether Restore has anything to list. Three separate rounds of review caught one of
    these each — if a screen states an installation fact, that fact has to arrive as a prop.
20. **The install bar is one row.** Registered count, guard state and backup date were
    removed from it — duplicated, non-actionable and menu-reachable respectively. Guard and
    backup moved to Settings, with a no-backup form for stock installations.

One behavioural invariant the design depends on, confirmed by the review: preparation
creates the library folders for **all three kinds**, whether or not anything of that kind
is registered. Without it, registering the first entry of a new kind would write into the
installation during Update mode — the mode that promises never to block and never to
require Bitwig closed. The design needs nothing for this beyond the invariant holding.

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
| `ORNG Registry.dc.html` | Root: window shell, all 39 states, state picker, theme toggle, drag overlay, plan/progress modals, banner |
| `UpdateModal.dc.html` | Catalog update confirmation: versions, Bitwig-open note, shield |
| `RenameDialog.dc.html` | Rename with the name-taken error; staged vs registered write note |
| `EntryRow.dc.html` | One Local row: 10 statuses, 3 kinds, hover/selected/factory/narrow, catalog provenance marker |
| `InstallBar.dc.html` | Single-row install bar: Local/Catalog switch, conditional state chip, catalog freshness, overflow menu |
| `ListToolbar.dc.html` | Local toolbar: search, kind filters with counts, factory toggle, Add files |
| `ActionBar.dc.html` | Summary, note, primary action, both apply modes |
| `Inspector.dc.html` | 272px Local detail panel, including the Source block |
| `CatalogRow.dc.html` | One catalog row: 7 statuses, inline description, author, version, action |
| `CatalogToolbar.dc.html` | Catalog toolbar: wide-scope search, kind facets, All/Installed/Updatable |
| `CatalogDetail.dc.html` | 272px catalog item panel, including provenance and the supersede block |
| `EmptyState.dc.html` | Six variants: nothing registered, no install, unknown build, no match, catalog never fetched, catalog no results |
| `SettingsScreen.dc.html` | Settings, including diagnostics, guard and backup state |
| `RestoreScreen.dc.html` | Restore backup |
| `AboutScreen.dc.html` | About |
| `support.js` | Prototype runtime — **not part of the design**, required only to open the HTML |

Open `ORNG Registry.dc.html` and use the picker above the window to reach any of the 39 states; the
`Dark`/`Light` toggle beside the title switches appearance. The child files are the same
components in isolation, each with editable properties.

Note: the child components define only the **dark** palette. The light theme is declared in
the root file, so a child opened on its own always renders dark.
