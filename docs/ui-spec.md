# Registry manager - UI requirements

App name: **Orange Registry** (binary `orange-registry`).

This document specifies **what must exist in the UI and how it must behave**. It does
not specify visual design. Layout sketches here are structural hints only; the designer
is free to rearrange, as long as every required element and state below is reachable.

Target toolkit: `eframe` / `egui`, immediate mode, single window. Windows, macOS, Linux.
Self-contained single binary - no installer, no runtime dependency, no bundled Python or
JDK.

---

## 1. What the app does

Bitwig Studio keeps an internal list of every device, modulator and Grid module it
considers native. Each row of that list is a `(UUID, display name, kind, library path)`
tuple compiled into `bitwig.jar`. A custom `.bwdevice` / `.bwmodulator` / `.bwmodule`
file that Bitwig does not know by UUID cannot be recalled reliably in a project.

The app makes a custom document known to a specific Bitwig installation:

1. Reads the document's identity (UUID, name, kind) from the file itself.
2. Prepares the installation once, so that it loads the app's entry list at startup.
3. Neutralises Bitwig's tamper response, which would otherwise fire because the
   installation changed.
4. Places the document where the registered library path resolves.
5. Adds a browser description and search keywords, so the entry is findable by typing.
6. Verifies the result, keeping a backup and an automatic rollback path.

The user-visible unit of work is dropping files onto the window and pressing one button.

### Two kinds of Apply

Step 2 happens **once per installation**, and again after a Bitwig update replaces the
installation. Everything else is ordinary file work. That produces two Apply modes with
genuinely different costs and preconditions, and the design must distinguish them:

| | **Prepare install** | **Update entries** |
| --- | --- | --- |
| When | first run on this install, and after every Bitwig update | every other change: add, remove, rename, edit keywords |
| Touches | the installation itself | only the entry list and document files |
| Bitwig must be closed | **yes** | no |
| Backup written | yes | no |
| Duration | seconds, with steps worth showing | effectively instant |
| Takes effect | next Bitwig launch | next Bitwig launch |

**Update entries must never write into the installation.** That is what lets it skip the
backup, the confirmation and the requirement that Bitwig be closed. To hold that line,
preparation creates the library folders for **all three kinds**, not only the kinds
registered at the time. Otherwise the first modulator added to a device-only install would
have to create a folder inside the installation - a write that can demand authorisation,
in the mode that promises never to block.

The common case after the first session is **Update entries**: no backup, no warnings, no
blocking, no step list. Treating every Apply as a heavyweight, dangerous operation would
misrepresent the app and make routine work feel risky. Treating the install preparation as
routine would hide a real consequence. Both modes need their own weight.

### Vocabulary (use these words in the UI)

| Term | Meaning |
| --- | --- |
| **Core Registry** | Bitwig's internal list inside `bitwig.jar`. The thing the app edits. |
| **Entry** | One row of the Core Registry. |
| **Kind** | `Device`, `Modulator` or `Module`. Bitwig's own three categories. Say "Grid module" in prose, "Module" in badges. |
| **Factory entry** | An entry Bitwig ships with (152 devices, 43 modulators, 233 modules in 6.1). Read-only. |
| **Custom entry** | An entry this app added. |
| **Install** | One Bitwig Studio installation, identified by version plus build revision. |
| **Registered** | Entry is in the Core Registry AND its file resolves. |
| **Staged** | Dropped and validated, not yet written. |
| **Prepare install** | The one-time operation that modifies the installation. Needs Bitwig closed. |
| **Update entries** | Every other change. Cheap, no backup, Bitwig may stay open. |

Do not use the words "patch", "hack" or "crack" in primary UI copy. "Register" and
"Apply" are the verbs. "Patched" is acceptable in the install status detail line.

---

## 2. Window and shell

- Single window. No menu bar. No tab bar for top-level navigation.
- Default size approximately 820 x 560. Minimum approximately 640 x 440.
- Content must stay usable at the minimum size: the list is the only region that grows.
- Three fixed regions, top to bottom:
  1. **Install bar** - which Bitwig this is and what state it is in.
  2. **Entry list** - the working area, grows with the window.
  3. **Action bar** - the single primary action plus status.
- Settings, About and Restore are secondary and live behind one overflow control in the
  install bar. They must not consume permanent layout space.
- Light and dark appearance both required, plus a "follow system" mode.

---

## 3. Region 1: install bar

Always visible. Answers "which Bitwig am I about to change, and is it safe right now".

Required elements:

- **Install name and version.** e.g. `Bitwig Studio 6.1` with the short build revision
  available (inline or on hover).
- **Install path**, truncated, full path on hover.
- **Change install** control. Opens a native folder picker. Must accept an install root
  the app failed to auto-detect.
- **Registry state badge**, exactly one of:
  - `Stock` - no custom entries, jar unmodified.
  - `Registered (N)` - N custom entries present and verified.
  - `Needs re-apply` - the app has custom entries on record but this jar is stock again
    (normal after a Bitwig update).
  - `Unknown build` - the app could not locate the registry structure in this jar.
  - `Modified elsewhere` - jar was changed by something that is not this app.
- **Tamper guard state**, one of `Armed`, `Disarmed`, `Unknown`. Secondary weight; it is
  diagnostic, not a user decision.
- **Backup indicator**: presence and date of the most recent backup, linking to Restore.
  Before the first preparation there is no backup, so this needs a second form - say so
  or omit the segment. It must not render an empty or placeholder date.
- **Overflow control** for: Settings, Restore backup, Open backups folder, About.

### Blocking banner

Only relevant to the **Prepare install** mode. When that mode is pending and Bitwig
Studio or its audio engine is running, a banner replaces or overlays the action bar:
`Quit Bitwig Studio before preparing the installation.` with a `Check again` control.
While it shows, the primary action is disabled. Everything else stays browsable.

In **Update entries** mode this banner must not appear. Bitwig may stay open; the change
is picked up the next time it launches.

---

## 4. Region 2: entry list

One flat, scrollable list. No master-detail split, no tree.

### Above the list

- **Search field**, filters by name and by UUID substring.
- **Kind filter**, three toggles: Devices, Modulators, Modules. Multi-select, all on by
  default. Show the count per kind.
- **Show factory entries** toggle. Off by default. When on, factory entries appear
  visually recessed and are not selectable for editing.

### The row

One row must carry, in reading order:

1. **Kind badge** - `DEVICE` / `MODULATOR` / `MODULE`. Three distinguishable colours.
   Colour alone must not be the only differentiator; keep the text.
2. **Display name** - the name shown in Bitwig's browser. Editable in place for custom
   entries.
3. **UUID** - monospace, may be truncated to first segment plus ellipsis. Full value on
   hover. Click copies.
4. **Status chip** - see the state table below.
5. **Row actions**, revealed on hover or always visible if the design allows:
   `Reveal file`, `Remove`, and for staged or conflicting rows `Assign new UUID`.

`Assign new UUID` must be offered **only** before an entry has been registered. A UUID is
how a saved project finds the device; reassigning one that is already in use silently
breaks every project that references it. Wherever the action appears - row, inspector,
menu - it carries the same restriction.

The registered library path (`devices/My Devices/NAME.bwdevice`) is detail, not primary.
Put it on hover or in an expanded row. It must be reachable somewhere.

An expanded row, or an equivalent detail surface, must also carry two editable text
fields for custom entries:

- **Description** - the one-line text Bitwig shows under the device in the browser.
- **Search keywords** - space-separated words that find the device when typed into the
  browser. The app proposes keywords derived from the name; the user may edit them.

These are cheap to write and are what makes a registered device feel native, so they are
required, not optional. They are secondary in weight: a user who never opens the detail
surface still gets working defaults.

### Row states

| Status | Meaning | Required affordance |
| --- | --- | --- |
| `Staged` | Dropped, validated, not written | included in the next Apply |
| `Registered` | In the registry, file resolves | Remove |
| `Pending restart` | Written, but Bitwig is open and has not reloaded yet | none, informational |
| `Missing file` | In the registry, file not found on disk | Locate file, or Remove |
| `Changed` | File on disk no longer matches what was registered | Re-apply |
| `Conflict` | UUID or name already used by another entry | Assign new UUID, or Cancel |
| `Rejected` | Dropped file the app cannot use | reason text, Dismiss |
| `Pending removal` | Marked for removal in the next Apply | Undo |
| `Factory` | Shipped with Bitwig | none, read-only |

`Rejected` rows must state the reason in plain language, for example
`Not a Bitwig device document`, `Saved by a newer Bitwig version`,
`Already registered with a different name`.

### Empty states

Three distinct ones are required:

- **No install found.** Headline plus a `Locate Bitwig Studio...` button. The list region
  is given over entirely to this. Mention that the app looked in the default locations.
- **Install found, nothing registered.** This is the primary onboarding surface: the drop
  invitation. It must name the three accepted extensions and say that Bitwig must be
  closed.
- **Build not recognised.** An installation is selected but the app could not locate what
  it needs inside it. Nothing can be listed and nothing can be applied, so the list region
  is given over to this state. It must name what could not be found in plain language, and
  offer `Copy diagnostics` as the primary action plus `Change install`.

  This is the state a user reaches the morning after a Bitwig release, so it is the one
  most likely to be seen by someone who is not expecting it. It is not an error the user
  caused and the copy must not read like one. The `Unknown build` badge alone is not
  enough: a badge with an empty list below it reads as a broken app.
- **Filter matches nothing.** Minor; offer `Clear filters`.

---

## 5. Drag and drop

- The **entire window** is a drop target, not a sub-region.
- On drag-over with at least one acceptable file, show a full-window overlay state. It
  must communicate accept versus reject before the drop, based on file extension.
- Accepted: `.bwdevice`, `.bwmodulator`, `.bwmodule`. Also accept a dropped **folder**
  and take the acceptable files inside it, one level deep at minimum.
- On drop, every file becomes a row immediately, then resolves asynchronously to
  `Staged`, `Conflict` or `Rejected`. Reading a document is fast; do not block the UI, and
  do not show a modal for it.
- Dropping a file that is already registered updates that row to `Changed` rather than
  creating a duplicate.
- A keyboard and screen-reader accessible alternative is required: an `Add files...`
  control that opens a native file picker. Drag and drop must never be the only path.

---

## 6. Region 3: action bar and the Apply flow

One primary button. Its label states the pending work and which mode it will run:
`Prepare installation` when the install still needs it, otherwise `Apply 2 changes`. It
is disabled with a stated reason when nothing is pending, when the build is unrecognised,
or when Bitwig is running **and** the pending work needs Prepare install.

When both are pending - a fresh install with dropped files waiting - it is one press.
Prepare runs first, entries follow. Do not make the user press twice.

### 6.1 Plan confirmation

**Update entries mode: no confirmation.** Files are copied and a list is rewritten. It is
undoable, touches nothing of Bitwig's, and asking would train the user to click through
the dialog that does matter. Apply immediately and report in the action bar.

**Prepare install mode: confirm.** Show what will happen, as a short list, not prose:

- That the installation will be modified, and that a backup is written first, with the
  backup location. The backup is `bitwig.jar` plus the description bundles, roughly 35 MB.
  It is not a copy of the installation, and must not be described as one.
- Entries to be registered, and entries to be removed.
- Files copied, and the destination.
- Library links created - under `Copy documents into the installation` there are none, and
  the line is omitted rather than shown empty.
- That this must be repeated after a Bitwig update.

**Removal is not confirmed here.** Entries are almost always removed in Update mode, which
applies directly, so a choice that lives only in this dialog is unreachable in the common
case. Whether the document file is deleted along with the entry belongs on the removal
action itself, or as a stated default in Settings that the row action names. Wherever it
lives, the default is **keep the file**.

Confirmation is a single explicit action. Do not require typed confirmation; the backup
plus rollback plus Restore make that ceremony unnecessary. Cancel must be equally
available.

### 6.2 Progress

**Update entries** is effectively instant. No step list, no modal, no progress bar. A
transient line in the action bar is enough. If it somehow takes longer than a moment,
degrade to an indeterminate indicator in place - never a dialog.

**Prepare install** is a transaction with ordered, nameable steps. Show them as a step
list with the current one active, not a bare spinner:

1. Back up the archive and the description bundles
2. Prepare the installation
3. Verify
4. Activate
5. Link library folders

**Nothing in the installation changes until step 4.** The patched archive is written beside
the original under a temporary name, loaded under Bitwig's own JVM to verify it, and moved
into place by a single rename. A failure at any earlier step leaves an installation that was
never touched.

That is why placing documents and writing descriptions are **not** in this list. They are
Update entries work: they touch no part of the archive, cannot disturb the tamper seal, and
so have no business inside the transaction that can fail. Reading the Core Registry is not a
step either - it happens while the plan is computed, before the user has agreed to anything.

Step 5 does not run under the `Copy documents into the installation` placement setting, and
the plan states which steps it will run. Draw the full list and render a step that will not
run as skipped; do not label a run "step N of 5" when it will take four.

The UI must not be frozen during this. Cancel is allowed up to the activation step; after
activation the operation is atomic and cancel is hidden.

### 6.3 Result

The next instruction depends on whether Bitwig is currently running:

- **Success, Bitwig closed**: `Start Bitwig Studio. Your devices are in the browser.`
- **Success, Bitwig running**: `Restart Bitwig Studio to see your changes.` This case only
  arises in Update entries mode, and stating it is required - a user who added a device
  and cannot find it in an open Bitwig will otherwise assume the app failed.
- **Failure**: state which step failed, in one sentence, plus what it means for the
  installation, plus a `Copy details` control that yields a technical report for a bug
  report. Never show a raw Rust error as the primary message.

  The honest sentence is `Your installation was not changed.` - not "restored from backup".
  A failure before activation never touched it, so there is nothing to restore and saying
  otherwise implies a repair that did not happen. Restore stays what it is: a thing the
  user asks for, to undo a preparation that succeeded.

Success in Update entries mode is a transient confirmation in the action bar, not a
persistent panel. Success in Prepare install mode is a persistent, dismissible summary.

---

## 7. Re-apply after a Bitwig update

This is the second most common session after the first one, and it deserves a designed
state rather than a fallback.

When the app opens and finds that the install has custom entries on record but a stock
installation, the install bar shows `Needs re-apply` and the primary action becomes
`Prepare installation`. This is the Prepare install mode again, with all of its weight:
Bitwig must be closed, a backup is written, the step list shows.

No re-dropping of files is required, and the entries themselves are untouched. The app
keeps its own record of registered identities, so the same UUIDs come back and existing
projects keep working. The list should therefore **not** mark every row as broken - the
entries are fine; the installation is what needs work. Communicate this once, at the
install bar, not once per row.

The app must persist that record per install, keyed by install path, surviving
replacement of the installation.

---

## 8. Removal and restore

- **Remove** on a registered entry marks it `Pending removal`; it takes effect on the
  next Apply. Removal takes the entry out of the registry. Whether the document file is
  also deleted must be an explicit choice in the confirmation step, defaulting to
  **keep the file**.
- **Restore backup** is a distinct, deliberate action behind the overflow control. It
  lists available backups with version, build and date, and restores the jar wholesale.
  It must warn that projects using custom devices will not recall them afterwards.

---

## 9. Settings

Small, one screen, no tabs. Required:

- Bitwig install path (with auto-detect reset).
- User library path (auto-detected per platform, overridable).
- Backup folder, with a control to open it.
- `Place documents in the user library and link them` versus `Copy documents into the
  installation`. Default to the first: documents then survive Bitwig updates and only the
  jar patch has to be re-applied.
- Appearance: System / Light / Dark.
- About: app version, the detected Bitwig version and build, and the disclaimer that this
  is unofficial and not affiliated with Bitwig GmbH.

  There is **no list of supported Bitwig builds** and the UI must not imply one. The app
  locates what it needs by structure, so an unseen release usually works and a changed one
  fails loudly. "Verified against \<versions\>" would describe a different app and would go
  stale the day it shipped. Report what was detected and whether it resolved.
- Diagnostics: a read-only report of what the app found in the current installation, with
  a `Copy` control. This is what a user sends when a new Bitwig build is not recognised,
  so it must be reachable without the app being in a working state.

  It can report: install path, Bitwig version and build revision, jar path, which anchors
  resolved and which did not, tamper guard state, the entry list location and its count,
  and the placement strategy. Real values for the paths are in the appendix; a mockup that
  invents one teaches a user to look in the wrong place. It **cannot** report the user's Bitwig licence tier. That is
  runtime state assembled at startup and nothing static in the installation exposes it, so
  no licence line can be filled in.

---

## 10. Copy requirements

- Short sentences. One idea per sentence. No exclamation marks.
- Never blame the user. `Bitwig Studio is still running.` not `You must quit Bitwig!`
- State consequences before the action, not after.
- Required verbatim concepts, wording may be polished:
  - `Quit Bitwig Studio before preparing the installation.`
  - `A backup is written before anything is changed.`
  - `A Bitwig update resets the installation. Prepare it again afterwards. Your
    registered devices are kept.`
  - `Restart Bitwig Studio to see your changes.`
  - `Not affiliated with or endorsed by Bitwig GmbH.`
- No emoji, anywhere. Icons are fine, emoji are not.

---

## 11. Non-goals

Out of scope for this UI. Do not design surfaces for them:

- Editing device contents, panels, DSP graphs or Nitro code.
- Browsing or previewing factory device internals.
- The donor-UUID method that avoids patching the jar.
- Multi-install management in one view. One selected install at a time.
- Any network feature, account, update check or telemetry.

---

## 12. Structural sketch

Hint only.

```
+--------------------------------------------------------------------+
| Bitwig Studio 6.1   /Applications/Bitwig Studio.app     [Change] [.]|
| Registered (6)      Guard: disarmed      Backup: 2026-09-14         |
+--------------------------------------------------------------------+
| [Search............]  [Devices 5] [Modulators 1] [Modules 0]  [ ]Fa |
+--------------------------------------------------------------------+
| DEVICE      DISPERSER            21882ab0...  Registered        ... |
| DEVICE      CURVECOMP            7c41d0b2...  Registered        ... |
| DEVICE      WAVESHAPER           9a10ff3e...  Staged            ... |
| MODULATOR   SHAPER               c0de4419...  Missing file      ... |
|                                                                    |
+--------------------------------------------------------------------+
| 1 to add, 1 to fix                              [ Apply 2 changes ] |
+--------------------------------------------------------------------+

Same window, install not prepared yet:

+--------------------------------------------------------------------+
| Bitwig Studio 6.1   /Applications/Bitwig Studio.app     [Change] [.]|
| Stock               Guard: armed         No backup yet              |
+--------------------------------------------------------------------+
| ...                                                                |
+--------------------------------------------------------------------+
| Quit Bitwig Studio first             [Check again] [ Prepare ... ]  |
+--------------------------------------------------------------------+
```

---

## 13. Open questions for the designer

1. Should `Staged` rows sit in a distinct section pinned to the top of the list, or mix
   into the list sorted with everything else? Recommendation: pinned to the top, since
   they are the pending work.
2. Row actions always visible, or on hover? Hover is cleaner but hurts discoverability
   and touch. Recommendation: `Remove` on hover, status chip always.
3. How much of the plan confirmation belongs inline in the action bar versus in a modal?
   A modal is safer for a destructive-adjacent action; inline is less interruptive.
4. `Changed` and `Missing file` are different problems with the same fix. Is one status
   with two reason texts better than two statuses?
5. The two Apply modes: one button that changes label and weight, or two visibly distinct
   affordances? One button keeps the app to a single primary action, but the same control
   then sometimes opens a confirmation and sometimes acts immediately, which can feel
   unpredictable. Recommendation: one button, with the mode named in its label so the
   difference is stated before the press.
6. `Needs re-apply` after a Bitwig update: how loud? It is the app's second most common
   state and nothing is broken - the entries are safe and the fix is one press. Loud
   enough to be found, calm enough not to alarm.

---

## Appendix: technical constraints the design must respect

- The three kinds are fixed by Bitwig: `DEVICE`, `MODULATOR`, `MODULE`. There is no
  fourth, and a document's kind is determined by its file, never chosen by the user.
- A document's UUID comes from the file. The app may assign a new one, which rewrites the
  document. This is destructive to projects already using the old identity, so it must be
  an explicit per-row action, never automatic.
- Registered library paths resolve inside the installation's own `Library` folder. The
  default strategy links that location to the user library, so the placement status of an
  entry can be `linked`, `copied` or `unresolved`. The list must be able to express
  `unresolved` - that is the `Missing file` status.
- Modifying the installation always requires disarming the tamper guard as well. These are
  not two user-facing options; they are one operation.
- The installation is prepared once and then reads the app's entry list at startup. That
  is why adding or removing a device afterwards is cheap and does not require closing
  Bitwig, and why a Bitwig update costs one preparation rather than one operation per
  registered device. The entry list is the app's own durable data; the installation holds
  nothing that needs preserving across an update.
- Changes are read by Bitwig at launch. Nothing the app does affects a running Bitwig, and
  nothing it does can take effect without a restart. Every success message has to account
  for this.
- Browser description and search keywords are **not** part of the jar. They live in
  plain properties files under the installation's `Resources/localization` folder, keyed
  by the entry's name. Writing them requires no bytecode work and does not disturb the
  tamper seal. They are therefore always applied, and the design does not need a
  "registered but unsearchable" partial state.
- Bitwig checks a UUID against a license entitlement grant map, separately per kind. A full
  Bitwig Studio license sets an "everything allowed" flag and the check passes for custom
  UUIDs untouched. A restricted edition (16-Track, Essentials, Producer, demo) does not,
  and an unknown UUID then fails, both when filtering library content and when
  instantiating the device. The app adds the UUID to that grant map, which grants exactly
  that one UUID and nothing else. It is not a user choice, and the design needs no
  control for it.
- Writing into the installation directory may require elevated rights. The design needs a
  state for "authorisation requested" during Apply on macOS and Linux. Only preparation
  writes there, so only preparation can reach that state.
- The backup is `bitwig.jar` (about 34 MB) plus the three description bundles. The
  installation as a whole is roughly a gigabyte and is never copied.
- **The paths the UI shows are these, and not approximations of them.** A mockup that
  invents one teaches a user to look where nothing is.

  | | |
  | --- | --- |
  | Entry list | `~/.orange-registry/entries.tsv` |
  | Backups | `~/.orange-registry/backups/<version>-<short revision>/` |
  | The archive | `Contents/Java/bitwig.jar` on macOS |
  | Description bundles | `Contents/Resources/localization/` on macOS |

  Both of the app's own paths are outside the installation, and that is the point: a Bitwig
  update replaces the installation wholesale, and it would take the entry list and the
  backup of the thing it replaced with it.

  The entry list is tab separated, not JSON. The class injected into the archive reads it at
  every launch, and `split("\t")` needs no parser, no dependency and no error handling worth
  the name. The extension is load-bearing and should be drawn as it is.

  `Contents/Java` and `Contents/Resources` are different places: the archive is in the
  former, the browser descriptions in the latter.
- Descriptions and keywords written to the base bundle reach every language. Bitwig loads
  the base file first and lets the locale-specific file overwrite it key by key, so a key
  that exists only in the base file survives. One file per kind is enough; there is no
  per-language work and no per-language state to show.
- Reading the factory entries means parsing a class out of a 30k-entry archive, which
  takes on the order of a second. The `Factory` toggle must not stall the window.
