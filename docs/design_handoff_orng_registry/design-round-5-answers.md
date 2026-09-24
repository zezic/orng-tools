# ORNG Registry: answers for round five

Replies to `design-round-4-changes.md`, in its order. Every drawn answer is a state in the
picker of `ORNG Registry.dc.html` (39 states now) and a prop on the component named.

---

## Part A

### A1. The update modal

A correction first: revision seven's shell did draw it (`catalogupdateconfirm`, inlined in the
root). It is now a component, **`UpdateModal.dc.html`**, and the shell mounts it. Both themes
come through the shell's tokens, like every other component.

- **Windows without rights:** `elevate` puts the shield before `Update`.
- **Bitwig open:** the modal says so, once, in a neutral strip: `Bitwig Studio is open, so it
  keeps playing 2.0.3 until you restart it.` (`bitwigOpen`; state *Update · Bitwig open*).
  Afterwards the row reads `Pending restart`. The two agree.
- **The detail panel's `Update…`** opens the same modal. The ellipsis marks that it opens a
  dialog.
- The lead now names the kind (`this modulator`, `this module`) where it said `device` for
  every kind.

### A2. The row's overflow menu

Keep it. It holds the actions that do not depend on the row's state (state *Row menu*;
`EntryRow` `menuOpen`):

| Item | When |
|---|---|
| `Rename…` | Every row except `Pending removal`. Disabled on catalog-sourced rows, with the tooltip `A catalog item keeps the catalog's name` |
| `Copy UUID` | Every row. The only route to the UUID once the narrow grid drops that column |
| `Show in catalog` | Catalog-sourced rows only. Opens the Catalog view with the item's detail |

There is no `More` on `Rejected` rows (nothing to act on) or factory rows. The menu uses the
install bar menu's surface, row height and shadow (`--shadow-menu`). The pointer leaving the
row closes it.

### A3. Rights on Windows

Yes to all five as written. One refinement and the placement rule:

- **The shield leads.** It goes before the words. Where a control already leads with a glyph,
  the shield replaces that glyph: `Restore this backup` drops its clock (`RestoreScreen`
  `elevate`).
- **Text controls wear it:** the action bar primary, the plan's `Prepare installation`, the
  catalog row's `Install`, `Update` and `Retry`, `Update` in the modal, `Save` in the
  inspector banner, `Rename` for a registered entry, and `Restore this backup`. The prop is
  `elevate` in every case.
- **Glyph-only row controls do not.** A 22px control has no room for a second glyph, and a
  shield that only shows on hover is not a convention anyone reads. `Locate` keeps its glyph,
  and its tooltip becomes `Locate file · asks Windows for administrator rights`. Staging
  controls (`Remove`, `Undo`, `Assign new UUID`) do not elevate, because nothing is written
  until Apply, and Apply wears the shield.
- **The inspector's Save question** sits in the panel's footer, not over the fields, so the
  words it saves stay visible. While it is open the close control is dimmed, with
  `Save or cancel the changes first` on hover. That is the same treatment as A6.
- **Declined:** neutral tone, dismissible, as you wrote it. It is not an error banner.

States *Windows · Local* and *Windows · Catalog*.

### A4. Renaming

`RenameDialog.dc.html`, state *Name conflict*.

- **Where it lives:** the row's overflow menu, the inspector's action list (`Rename…`, above
  `Reveal file`), and the conflict row. The name field in the inspector stays read-only.
- **The conflict row:** a name conflict and an identity conflict are now told apart
  (`EntryRow` `conflict: "name" | "uuid"`). A name conflict reads
  `Name already used by a registered device` and offers the pencil (`ph-pencil-simple`,
  accent, the same weight as `Locate`) in place of the fingerprint. A UUID conflict keeps
  `Assign new UUID`.
- **The dialog:** a field; `A registered device is already called DISPERSER.` in
  `--err-text` with an error outline while the name is taken; the reason for the rule; and
  when it is written. `Rename` is disabled until the name is free, with
  `Choose a name no other entry uses` on hover.
- **When it is written:** for a staged document, nothing is written until Apply runs. For a
  registered entry it is written on the press, like any inspector edit, so it wears the shield
  on Windows.
- **Catalog-sourced entries cannot be renamed.** An update replaces the document by UUID and
  would bring back the catalog's name.

README corrected in both places: the app never renames on its own; a name collision is a
`Conflict` the user resolves with Rename.

### A5. Settings with no installation

Yes, as you describe it. Drawn as state *Settings · no install* (`SettingsScreen`
`installed: false`): install and backup paths read `—` in `--ink-3`, `Browse` stays, the user
library still shows if it was found, and the report reads
`install    not found · searched /Applications, ~/Applications`, with `—` on every line that
depends on an installation.

### A6. The inspector while it cannot write

A footer in the same place as the Save question, neutral tone:
`Saved when the registration finishes` / `Your changes wait here until the other work is
over, so the panel stays open until then.` The close control is dimmed, with
`Your changes are saved when the registration finishes` on hover. `Inspector` `waiting`
takes the name of the work (`the preparation`, `the registration`, `the download`). State
*Inspector waiting*.

### A7. A drop during a run

Yes, no drop target. The platform's own "no drop" cursor is the refusal, and it needs no
words.

---

## Part B

### B1. Catalog action bar

- **Fetching:** yes on the bar, and it also goes on the row. `Fetching…` in `--ink-2`, with no
  action, becomes the eighth published row state (`CatalogRow` `status: "Fetching"`). The row
  is where the user pressed, so that is where the answer appears first.
- Local primary disabled with `In progress`, label unchanged: **yes.**
- `Catalog · N installed`: **yes.**
- Stale after a week: **yes.**
- Age format: **yes**, including `370 days ago`.
- Registration target gone: **yes**, as worded.

### B2. The hidden-entries note

It was meant to count **pending work out of sight**, because Apply acts on rows the user
cannot see. The sentence becomes `N changes hidden by the current filter`, with N counting the
pending changes the filter hides. Show it whenever that number is above zero, not only when
the list is empty. In the preparing mode the cost note (`a backup is written first`) keeps the
line, because the plan confirmation lists every change, hidden or not.

### B3. The plan confirmation

- All four additions: **yes**, as worded.
- The re-preparation line: add one, after the registrations:
  `6 entries already registered keep their UUIDs; their description bundles are written again.`
- The press: the outline was not meant. It is 32 tall with no border, like the action bar. The
  shell is corrected.

### B4. `Browse all`

Dropped. `Clear filters` alone, as you drew it.

---

## Part C

- **13.5px tracking:** `-0.015em` everywhere. The three screen headers are corrected.
- **Install bar on `noinstall`:** 42. The shell now uses `9px 12px`.
- **`--shadow`:** `.85`. Every component is corrected.
- **Light shadows, now tokens in both themes:**

  | Token | Dark | Light |
  |---|---|---|
  | `--shadow-menu` | `0 18px 40px rgba(0,0,0,.7)` | `0 12px 28px rgba(0,0,0,.16)` |
  | `--shadow-panel` | `-18px 0 40px rgba(0,0,0,.5)` | `-14px 0 32px rgba(0,0,0,.12)` |

  The light values are drawn values, not ratios. The spread tightens as well as the alpha,
  because a wide pale shadow on `#f2f2f2` reads as a smudge.

---

## Not drawn, on purpose

- **No `Cancel` in progress:** fine. The note under the steps must then stop mentioning
  Cancel. Use `Nothing in the installation changes until the patched archive verifies.` up to
  Activate, and the existing Activate sentence without its first clause after that.
- **Factory entries:** fine.
- **No `Licences`:** fine for now. A public release must ship the notices for Inter,
  Iosevka and Phosphor, so it comes back before 1.0.
- **No quiet refresh on entering the Catalog view:** agreed. `ui-spec-catalog.md` §7 is
  corrected to launch and `Refresh` only.
- **Update modal, overflow menu, `Browse all`:** answered above.
