# ORNG Registry: requests and questions for round five

For the designer. A work list, not a review - the reasoning behind each item is in
`docs/design-review.md` (round 3) if you want it, but you should not need it.

Revision seven is built. Every screen, component and state it draws is in the app, measured
against the bundle, except the few listed under *Not drawn, on purpose* at the end. What is
left is of three kinds: **things the bundle does not draw that the app needs**, **words and
rules the app had to decide without you**, and **a handful of values the bundle states two
ways or not at all**. Nothing you drew needs redrawing unless an answer below says so.

Line numbers are revision seven's.

---

## Part A - to draw

### A1. The update modal, and so `Update` on a catalog row

`README.md:802-808` says exactly what it carries: the item and the target version in the
title (`Update BREATH FOLLOWER to 2.1.0?`), `installed 2.0.3 -> catalog 2.1.0` in mono
under the lead, and the sentence that matters - projects that already use the device will
use the new version. **No component draws it**, and neither does the full-window mockup.

Until it exists the row says `Update available` in the accent and offers no press. An
update without its confirmation would replace the file under every open project, which is
the one thing you put a dialog in front of, so the app does not do a smaller version of it.

Asked for: the modal, as a component, in both themes, with its two presses. Three things
it will meet that the README does not mention:

- **Windows without rights.** The press will raise Windows' consent dialog, so it wears the
  shield (A3).
- **Bitwig open.** An update is entries work like an install, so Bitwig may stay open, and
  the row then reads `Pending restart` until Bitwig is started again. Does the modal say
  so, or is that the row's job afterwards?
- **The detail panel.** `CatalogDetail.dc.html:109` gives the updatable state an `Update...`
  primary, which the app does not draw either. Does it open the same modal?

### A2. What the row's overflow menu holds

`EntryRow.dc.html:60` draws `ph-dots-three-vertical` on every row in every state, and
`README.md:407` lists it last among the row's actions. **Nothing in the bundle says what it
opens.** The five conditional actions are already on the row, and the narrow grid drops the
identity column rather than a control, so it is not obviously the overflow for a squeezed
row either.

The app does not draw it: a control that opens nothing is worse than no control. Asked
for: the menu's contents and one drawn state of it open, or a decision to drop the control.

### A3. Rights on Windows: the shield, and where else it goes

An installation under `Program Files` is not this account's to write, and **every change
reaches inside it** - preparing writes the archive, and every entry change rewrites the
description bundles in its `localization` folder. The bundle draws no state for any of this.
What the app does now, all of it ours:

- **No dialog of our own.** The work is handed to a second copy of the app that Windows
  starts with the rights, and the user answers Windows' consent dialog. Our dialog in front
  of it would be two dialogs for one question.
- **A shield on the press.** A press that will end in the consent dialog wears `ph-shield`,
  light, before its words, at the size and ink of the arrow after them - Windows' own
  convention for a control that elevates. It is on the action bar's primary in both modes
  and on the plan's `Prepare installation`. Nowhere where nothing will ask.
- **An inspector edit waits for a press.** Every other edit is written when a field is left;
  here that would raise the consent dialog because the pointer left a text box. So the words
  wait behind a warn banner: `Save the changes to <name>?`, body `<installation> is not
  writable by this account, so saving asks Windows for administrator rights. The
  description and keywords Bitwig shows are kept inside the installation.`, and two presses,
  `Cancel` and `Save`, with no dismiss mark.
- **Declining is not a failure.** Dismissing Windows' dialog reports `administrator rights
  were declined, so nothing was changed`.
- **Where the platform cannot ask** (macOS, Linux), the press is refused by a banner that
  says `This installation is not yours to change.` and names the directory. No control.

Asked for: a yes or a correction on each. And **where the shield goes on the small
controls** that elevate the same way and do not wear it yet - a catalog row's `Install`
(`CatalogRow.dc.html:98`), a row's `Locate` (`ph-file-magnifying-glass`), and the Restore
screen's `Restore this backup` (`RestoreScreen.dc.html:81`). A row control is one glyph
already; whether the shield replaces it, sits beside it, or only shows on hover is yours.

### A4. Renaming, and a name that is already taken

`README.md:132` and `:855` say the app renames entries when their names collide. **It does
not, and today nothing can.** Bitwig's browser is flat and matches on the name, so a dropped
document whose name another entry already holds is a `Conflict`. The only remedy the row
offers is `Assign new UUID`, which settles an identity collision and not a name one - so a
name conflict has no way out from the window.

Renaming is also why the inspector's name field is read-only: Bitwig keys an entry's
description bundle by its display name, so a rename rewrites the document's own name as
well. It is a feature rather than a field.

Asked for: a rename, drawn - where it lives (the conflict row, the inspector, the overflow
from A2), what it says, and what the conflict row offers once it exists. And the two README
sentences corrected to whichever it is.

### A5. Settings on a machine with no installation

The install bar's overflow reaches Settings in every state, including "no Bitwig Studio
found" and "this build could not be read", and `SettingsScreen.dc.html` draws one machine
with every path resolved. The app draws the rows anyway, writes `-` where there is nothing
to say so the groups keep their height, and the diagnostics block states what there is - the
root that was refused and why, or the places searched. It is the state somebody is most
likely to be in when they open this screen. Asked for: the screen drawn in that state, or a
yes to the above.

### A6. The inspector while its words cannot be written yet

New since revision seven. An edit made while other work is running - a preparation, a
registration, a catalog download - waits in the panel until the work is over, as it waits
behind the `Save` question in A3. Closing the panel or clicking another row then would lose
the words, so **the panel does not close**. Nothing on screen says why: the close control
simply does nothing until the work finishes.

Asked for: what the panel shows in that state. A line under the fields, the close control
disabled with a reason on hover, or something else.

### A7. A drop while work is running

A drop while a run is in flight is refused, because the press takes the pending rows as they
stand. Rather than draw the drag-over overlay (`Drop to stage 1 document`) and then ignore
the drop, the app draws **no drop target at all** while a run is going. The other way would
be an overlay saying why, which needs words the bundle does not have. Asked for: a yes, or
those words.

---

## Part B - words and rules we decided without you

Each is drawn; each wants a yes or a correction.

### B1. The catalog's action bar

- **A download in flight.** Between pressing `Install` and the registration there is a
  second or two the seven row states have no word for. The bar says `Fetching <item>` in the
  neutral tone. If it belongs on the row, that is an eighth published state.
- **While that download is out, the Local primary is disabled**, with `In progress` on its
  hover, and its label keeps saying what it would do (`Apply 2 changes`), because nothing is
  being applied yet.
- **The `Installed` filter's count.** `All` and `Updatable` are captioned
  (`ORNG Registry.dc.html:433`, `:439`); the third reads `Catalog`, your middle dot, then
  `N installed` - the same sentence with the same substitution.
- **When an index stops being current.** A week. You draw 20 minutes as current and 12 days
  as stale; the window checks on every launch, so a week without success is a machine off
  the network rather than a quiet catalog.
- **How an age is written.** The largest whole unit, nothing below a minute, `just now`
  under one, and days as the largest unit - so a year offline reads `370 days ago`.
- **An install whose installation went away** while it downloaded is a failed install whose
  detail reads `<item> was fetched, and there is no installation to register it in`.

### B2. The Local bar's hidden-entries note

`ORNG Registry.dc.html:490` draws `5 entries hidden by the current filter` under a search
that matches nothing. The note is a literal there, so two things are ours: it is drawn only
once the filter has left **nothing** on screen, and the number counts **the rows the list
would have drawn**. Your sample's `5` is the count in `Apply 5 changes` on the same bar,
not the list's ten. If it was meant to count pending work out of sight, it is a different
sentence. And in the preparing mode it replaces the note that says a backup is written
first, for as long as the filter is empty - should the cost outrank the count there?

### B3. The plan confirmation

`ORNG Registry.dc.html:223-252` draws the plan over one scenario; the app draws it over
whatever the press will do, and four things it says are not in your scenario:

- **A backup that already exists is kept**: `The archive and the description bundles are
  already backed up in ..., and the patch is built from that copy.`
- **A removal names what happens to its file**, in its control's words: `1 entry removed:
  OLD REVERB. The document file is kept.`
- **The links are counted**: `3 library links created inside the installation's Library
  folder, once the archive is in place`, or `The installation's library folders are already
  linked to the user library.` Preparation links all three kinds whatever is registered.
- **The entries already on record get no line**, as in your scenario. A re-preparation also
  rewrites their description bundles; if the plan should say so, it is one line.

And the press: yours has a one-pixel accent outline on the accent fill, 34 tall; the app
draws the action bar's 32. Say if the outline is meant.

### B4. `Browse all` beside `Clear filters`

`EmptyState.dc.html:97` draws both in the catalog's no-match state, and the shell's two
handlers do the same thing (`ORNG Registry.dc.html:770` and `:773`, resolved at `:637`). The
app draws `Clear filters` alone. If `Browse all` is meant to differ - keep the search and
drop the filters, say - tell us what it does, and it is drawn.

---

## Part C - values stated twice, or not at all

- **13.5px is tracked two ways.** `-0.015em` on the install bar's title
  (`InstallBar.dc.html:29`, `ORNG Registry.dc.html:90`), `-0.02em` on the three screen
  headers (`SettingsScreen.dc.html:30`, `RestoreScreen.dc.html:30`,
  `AboutScreen.dc.html:30`). The app tracks by face and size, so both draw `-0.015em`. If
  they are meant to differ, the cleanest answer is a different size for one of them.
- **The install bar is 50 tall on `noinstall`**, where the shell inlines a copy of the bar
  at `padding:13px 12px` (`ORNG Registry.dc.html:85`) instead of the component's `9px`. The
  app draws 42 everywhere. Say if the 50 is meant.
- **`--shadow` is `.8` in the components and `.85` in the shell and `EmptyState`** (and
  `README.md:303`). The app uses `.85`. Which?
- **The light values of two shadows.** The overflow menu's `rgba(0,0,0,.7)` and the
  inspector's shadow at `.5` are written as literals, dark only. The app derives light by
  the ratio of your two `--shadow` tokens (`.85` to `.22`), giving `.18` and `.13`. A
  measured value would be better than a derived one.

---

## Not drawn, on purpose

So they are not taken for omissions. Say if any of these is wrong.

- **`Cancel` in the progress dialog.** Nothing can cancel a preparation yet, so the dialog
  is drawn without it - the shape you draw from Activate onwards.
- **`Show factory entries`, and the factory section under it.** Decided against for now;
  the entry list has nine states rather than ten, and the Local toolbar four boxes of five.
- **`Licences` on About.** Nothing in the build carries the text it would show. `Copy
  diagnostics` is drawn alone.
- **A quiet refresh on entering the Catalog view** (`ui-spec-catalog.md:201`). The window
  checks on every launch and on `Refresh`; a second trigger would fetch twice for one look.
- **The update modal, the overflow menu and `Browse all`**, until A1, A2 and B4 are answered.
