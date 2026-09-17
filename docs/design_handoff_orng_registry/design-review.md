# Design review: Orange Registry

Validation of the design handoff against the app's actual behaviour and against the
Bitwig internals it depends on. Everything below was checked against a real Bitwig Studio
6.1 installation and against the library that now implements this, not against the spec.

The design is accepted. Nine items need changes: four are factually wrong, three are
behavioural holes, one state is missing, and one is a contrast check. `ui-spec.md` has
been updated to match, so it and this document agree.

---

## Confirmed correct

Listed because each was a genuine risk, not to pad the review.

- **"Read the Core Registry (428 entries)"** is exact: 152 devices, 43 modulators,
  233 Grid modules.
- **Descriptions and keywords reach non-English users.** This was the single biggest risk
  in the design. Bitwig ships six localised copies of each description bundle and every
  one carries a complete translation of all 304 keys, so a reasonable assumption would be
  that a key written only to the base file is invisible to a German or Japanese user. It
  is not: the loader builds a narrowing list of candidate files and merges them
  base-first, letting the locale-specific file overwrite key by key. A key that exists
  only in the base file survives in every language. Writing one file per kind is enough,
  and there is no per-language state for the UI to represent.
- **`linked` / `copied` / `unresolved`** matches the implementation exactly.
- **Per-entry state** in "State management" is completely covered.
- **Design decisions 1 to 8** are all compatible. Decision 3 (needs-re-apply as a banner
  with no row marked broken) and decision 5 (the restart banner is mandatory) match the
  architecture particularly well - after a Bitwig update the entries genuinely are
  untouched, and nothing the app does can affect a running Bitwig.

One gap the design exposed on our side: nothing in the library reported the Bitwig
version or build. That is now implemented and yields `6.1 (94a90411)`, so the install bar
and About screen can be filled in as designed.

---

## Factually wrong

### 1. "Back up the installation (238 MB)"

`bitwig.jar` is **34 MB**. The whole installation is about 957 MB and is never copied. The
backup is the jar plus three description bundles, so roughly 35 MB.

The number matters beyond accuracy: 238 MB implies a slow, disk-heavy operation and makes
the progress modal look necessary for reasons it is not. Change the figure, and keep the
label honest about what is copied.

### 2. Diagnostics line `license`

We cannot report it. Bitwig assembles the user's entitlement at startup from licence
records; nothing static in the installation exposes the tier. There is no value to put in
that line.

Replace it with something we can actually produce. The report can carry: install path,
version and build revision, jar path, which anchors resolved and which did not, tamper
guard state, entry list location and count, placement strategy.

### 3. About: "Verified against"

There is no list of supported builds, and the UI must not imply one. The app locates what
it needs by structure rather than by pinned names, which is what lets an unseen Bitwig
release work on day one and a genuinely changed one fail loudly instead of silently
mispatching. A "verified against" line describes a different app, and would be stale the
day it shipped.

Report the detected version and build, and whether resolution succeeded.

### 4. Install bar: `Backup <date>` before the first preparation

On a `Stock` installation no backup exists yet. That segment needs a second form, or must
be omitted in that state. It must not show an empty or placeholder date.

---

## Behavioural holes

### 5. Removal has no reachable confirmation

"Also delete the document file of removed entries" lives only in the prepare modal. But
removing an entry is Update-mode work, and Update mode applies directly with no
confirmation - by design decision 1, which is correct. So as drawn, a user who removes an
entry is never asked, and the checkbox is unreachable in the case it was written for.

Move the choice onto the removal action itself, or make it a stated default in Settings
that the row action names. Either way the default is **keep the file**: the document is
the user's own work and the registration is not.

### 6. Update mode can still touch the installation

Registering the first entry of a *new kind* needs a folder created inside the installation
(`Library/modulators/My Modulators` on a device-only install). That is a write into the
application bundle, which can demand authorisation - in the mode that promises never to
block and never to need Bitwig closed.

Fix at the behaviour level, not the design level: preparation creates the folders for all
three kinds, whether or not anything of that kind is registered yet. Update mode then
provably never writes into the installation and the promise holds. The design needs no
change beyond knowing the invariant is real.

### 7. "Step N of 9" is not always nine

Step 6, "Link library folders", does not run under the `Copy documents into the
installation` placement setting. Either derive the count, or keep nine steps and render
that one as skipped. Do not label a run "step N of 9" when it will take eight.

### 8. "Assign new UUID" is gated on the row but not in the inspector

The row has it right: staged and conflicting entries only. The inspector lists it
unconditionally in its Actions group.

A UUID is how a saved project finds a device. Reassigning one that is already registered
silently breaks every project referencing it, with no error and no way back except
restoring the old identity by hand. The restriction has to hold wherever the action
appears.

---

## Missing state

### 9. `Unknown build` has a badge but no screen

When the app cannot locate what it needs inside an installation, it cannot list entries
and cannot apply anything. The badge is drawn, but the list region below it has no
designed state - so it renders as an empty list under a grey badge, which reads as a
broken app rather than a recognised condition.

This needs a full-region state alongside "No install found": what could not be found, in
plain language, with `Copy diagnostics` as the primary action and `Change install`
alongside.

Worth designing carefully rather than treating as an edge case. It is the state a user
reaches the morning after a Bitwig release, it is nobody's fault, and it is the one most
likely to be met by someone who was not expecting it. The copy should not read like an
error the user caused.

---

## To verify

### 10. Factory rows at 50% opacity

The README states all text was verified at 4.5:1 against its composited background.
`--ink-3` at 50% opacity on `--row` is the one place that looks likely to fall under. Worth
re-measuring; if it does, dimming the row less and relying on the transparent status dot
and absent hover to signal read-only would carry the same meaning.

---

## Implementation notes

Not design changes. Recorded so the cost is known when this is rebuilt in egui.

- **Phosphor duotone is two stacked glyphs** at different opacities, not a single
  character. In an immediate-mode toolkit that is two draw passes per icon or an SVG
  pipeline. It is the largest single piece of work implied by the visual spec.
- **Inter has to be bundled** as well as Iosevka - the binary is self-contained and cannot
  assume a system font. Both faces are OFL, so bundling is fine.
- Sticky section bands, the 15px scrollbar reserve and hover-revealed row actions are all
  manual in immediate mode. None is difficult; all are hand-written.
- Resizing is listed as untested. The narrow four-column row variant already supplies the
  reflow rule, so a minimum window size plus that one breakpoint covers it.
- Reading the 428 factory entries means parsing a class out of a 30,000-entry archive,
  about a second. The `Factory` toggle has to do that off the UI thread.
- Fractional type sizes (10.5, 11.5, 12.5) round to the toolkit's scale, as the handoff
  already allows.
