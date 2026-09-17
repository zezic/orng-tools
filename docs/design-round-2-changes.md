# Orng Registry: changes for round three

For the designer. A work list, not a review - the reasoning behind each item is in
`design-review.md` if you want it, but you should not need it to make these changes.

Round two is accepted. All ten items from the first round are answered, several better than
they were asked: factory rows are not dimmed at all rather than dimmed less, and the skipped
apply step renders as "not run" instead of quietly changing the count. The Catalog view is
right in the places that were hardest to get right - refusing an install on a hash mismatch
with `Copy details` and deliberately no `Retry`, install not confirming while update does,
and insisting both items of a supersede pair exist in the catalog so the offer can navigate.

Six changes. Four are small, one is a restructure, and one is a rename that
arrived after round two was drawn.

---

## 1. Three paths are wrong

All three are drawn values with no source in the spec, which was our omission - the real
ones are now in `ui-spec.md`, appendix. They are:

| | |
| --- | --- |
| Entry list | `~/.orng/entries.tsv` |
| Backups | `~/.orng/backups/<version>-<short revision>/` |
| The archive | `Contents/Java/bitwig.jar` |

### Entry list

`SettingsScreen.dc.html:155-156` reads `Library/orange-registry.json`.

Two things are wrong and the first one matters. `Library/` is inside the Bitwig
installation, and a Bitwig update replaces that directory wholesale - the entry list lives
outside it precisely so an update cannot touch it. That is what makes "a Bitwig update costs
one preparation" true instead of "one preparation and your registrations are gone".

And it is not JSON. The format is tab separated because the class injected into the archive
reads it at every launch, and `split("\t")` needs no parser and no dependency. The extension
is load-bearing; please draw `.tsv`.

### Backups

`SettingsScreen.dc.html:53` and `Orange Registry.dc.html:387` read
`~/Library/Application Support/OrangeRegistry/backups`.

Same root as the entry list, for the same reason: a backup of the installation must not live
inside the thing it is a backup of. One directory per Bitwig build, named for it.

### The archive, in diagnostics

`SettingsScreen.dc.html:184` reads `jar  Contents/Resources/bitwig.jar`. It is
`Contents/Java/bitwig.jar`.

`Contents/Resources` is a real directory - it holds the factory `Library` and the
description bundles - which makes this wrong in a way that looks right. It also sits in the
block whose whole purpose is being copied into a bug report, so the wrong path gets repeated
by the user to whoever is helping them.

---

## 2. The apply steps: nine become five, and three of them leave

`Orange Registry.dc.html:375-385`, and `README.md` lines 85, 497, 501 and 724.

The step list as drawn places documents (5), links folders (6) and writes descriptions (7)
**before** Verify (8) and Activate (9). The transaction does not work that way, and the
difference is the property it exists to provide.

Nothing in the installation changes until the patched archive verifies. It is written beside
the original under a temporary name, loaded under Bitwig's own JVM, and moved into place by
a single rename. A failure anywhere earlier leaves an installation that was never touched.

The list is:

1. Back up the archive and the description bundles
2. Prepare the installation
3. Verify
4. Activate
5. Link library folders

Placing documents and writing descriptions are *Update entries* work - they touch no part of
the archive and cannot disturb the tamper seal, which is exactly why they are not inside the
transaction that can fail. "Read the Core Registry" is not a step either: it happens while
the plan is computed, before the confirmation the user has not yet agreed to.

Three consequences for the drawing:

- **Cancel up to Activate now means something clean.** As drawn it meant a cancelled run had
  already copied the user's documents and rewritten Bitwig's description bundles. Under the
  real order there is nothing to undo, and the copy can say so plainly.
- **The failure message changes.** `Your installation was restored from backup` should be
  `Your installation was not changed.` Nothing was restored, because nothing was touched.
  Restore stays what it already is on its own screen: something the user asks for, to undo a
  preparation that succeeded.
- **The skipped-step machinery you built still applies**, to step 5 instead of step 6. It is
  skipped under `Copy documents into the installation`, for the reason in item 5 below. The
  plan states which steps it will run, so you do not have to derive it.

---

## 3. Provenance matches on UUID, not on display name

`README.md` around line 700, in "What the Local view gains".

The rule that Local and Catalog must not disagree about what is installed is right, and the
diagnosis of how they drifted during design is right. The key is wrong. An entry is
currently described as catalog-sourced when "a catalog item of the same name" reports an
installed status.

Display names are allowed to collide in the repository, and the app renames entries when
they do, precisely because the name is not the identity. A UUID is exact, is already in both
records, and cannot be renamed. Name matching will eventually mark the wrong row - and it
will do it first to the user who hit the rename path, who is least able to explain what
happened.

One sentence. Nothing drawn changes.

---

## 4. Two things you drew are not yet in the data. Keep them

Flagged only so you do not simplify them away on the assumption that they are unsupported.
Both are ours to build and both are agreed.

- **The link to the merged change**, in the item detail panel. The index did not carry a
  per-item reference to it; it is being added. Keep the panel as drawn. (It has a single
  index-wide revision, which names the commit the index was built from - that traces the
  index, not the item. Please do not fall back to showing that instead.)
- **Two version fields on an updatable item**, which the update modal needs for
  "installed 2.0.3 -> catalog 2.1.0". The entry list did not record the installed version;
  it is being added. Keep the modal as drawn.

  This is also why the name-versus-UUID question above is not cosmetic: the installed version
  has to be recorded against the identity, and then the same record answers both questions.

---

## 5. One thing you assumed about us was right, and we have fixed it

No change needed; recorded so the two documents agree.

The design assumed preparation skips the link step under `Copy` placement. It did not - it
linked all three kinds unconditionally, which made the copy setting a choice that made no
difference. A registered path resolves inside the installation's `Library`, so a linked
folder resolved straight back out into the user library and a document "copied into the
installation" landed in exactly the file the linked strategy would have used.

It now skips, as you had it.

---

## 6. The project is now called Orng

This landed after you drew round two, so nothing about it is your error - but it touches
every screen, and it is better done in one pass than discovered later.

`orng.tools` is the project's domain and the name follows it. The product names become
**Orng Registry** and **Orng Catalog**, and the binary is `orng-registry`.

The paths in item 1 already reflect this, so drawing them from that table gets both changes
at once:

| | |
| --- | --- |
| Entry list | `~/.orng/entries.tsv` |
| Backups | `~/.orng/backups/<version>-<short revision>/` |

Two consequences worth naming:

- **The app's directory is now one root, `~/.orng/`,** holding the entry list and the
  backups. The old name said "registry" while also holding backups of the *installation*,
  which are not the registry's. Settings shows a backup folder inside it rather than a
  separate location.
- **The provenance string changes shape.** `orange-catalog #455` becomes an `orng-catalog`
  reference - though per item 4 that field is being added to the index, so the exact form is
  ours to give you rather than yours to invent. Draw it as a link with a short label.

The name Bitwig is unchanged everywhere it appears, and so is anything describing Bitwig's
own files: `Contents/Java/bitwig.jar`, the `Library` folder, the description bundles.

---

## Not changes

- The numbers are right now. 35 MB for the backup, 34 MB for the archive, 428 factory
  entries as 152 devices, 43 modulators and 233 Grid modules, and `6.1 (94a90411)` as the
  shape of a build string.
- The Catalog statuses map cleanly onto what the index and the entry list can answer, with
  the two exceptions in item 4.
- `Requires Bitwig <version> or newer` is carried per item in the index and can be stated
  whether or not it is satisfied, as you have it.
- Search covering author in the Catalog and UUID in Local is a real distinction and worth
  the asymmetry.
