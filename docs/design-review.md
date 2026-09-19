# Design review: ORNG Registry

Validation of the design handoff against the app's actual behaviour and against the
Bitwig internals it depends on. Everything below was checked against a real Bitwig Studio
6.1 installation and against the library that implements this, not against the spec.

Round 1 is closed: all ten items were addressed, and several were answered better than
they were asked. Round 2 adds the Catalog view, and raises eight items - three factual,
one that contradicts a safety property of the transaction, three that depend on data no
format currently carries, and one that was our bug rather than the design's and is now
fixed. Round 3 collects what building the window against revision 7 turned up, and its
last two items are open questions for the designer rather than findings.

---

## Round 1: closed

Recorded compactly because the reasoning is in git, not because it did not matter.

| | Item | How it was answered |
| --- | --- | --- |
| 1 | Backup stated as 238 MB | 35 MB, and named as the jar plus description bundles rather than the installation |
| 2 | Diagnostics line `license` | Gone, with a note saying why the tier cannot be reported |
| 3 | About: "Verified against" | Gone; no supported-builds row, and the README says the UI must not imply one |
| 4 | `Backup <date>` before the first preparation | "No backup yet" replaces the date and the `Restore` offer |
| 5 | Removal had no reachable confirmation | Moved onto the removal action, defaulting to keeping the file |
| 6 | Update mode could still touch the installation | Invariant now holds in the library; design unchanged, as expected |
| 7 | "Step N of 9" is not always nine | Count derived, and the skipped step renders as "not run" |
| 8 | `Assign new UUID` ungated in the inspector | Gated on `showAssign`, matching the row |
| 9 | `Unknown build` had a badge but no screen | Full-region state, and the copy does not read like the user's fault |
| 10 | Factory rows at 50% opacity | Not dimmed at all now, which is the better answer |

---

## Round 2: factually wrong

### 1. The entry list is in the wrong place, and the place matters

`SettingsScreen` shows `Library/orange-registry.json`. It is `~/.orng/entries.tsv`.

The path is not a detail here. Inside the installation's `Library` the list would be
destroyed by every Bitwig update - and "a Bitwig update costs one preparation, not one
operation per registered item" is the promise the whole architecture is built to keep. The
list lives outside the installation precisely so an update cannot touch it.

The extension is also load-bearing. The format is tab separated because the class injected
into the archive reads it at every launch, and `split("\t")` needs no parser, no dependency
and no error handling worth the name. It is not JSON and should not be drawn as JSON.

### 2. The backups path is wrong

`~/Library/Application Support/OrangeRegistry/backups` appears in Settings, in the plan
confirmation and on the Restore screen. It is `~/.orng/backups/`, one directory
per Bitwig build, named `<version>-<short revision>`.

Same root as the entry list, for the same reason: a backup of the installation must not be
inside the thing it is a backup of.

### 3. Diagnostics names the wrong jar path

The report reads `jar  Contents/Resources/bitwig.jar`. It is `Contents/Java/bitwig.jar`.
`Contents/Resources` is where `Library/` and `localization/` live, so the path is not
merely wrong but plausibly wrong.

This one is worth more than its size: the diagnostics block exists to be copied into a bug
report, so a wrong path there is repeated by the user to whoever is helping them.

---

## Round 2: contradicts the transaction

### 4. Three of the nine steps run before the installation is safe to touch

The step list places documents (5), links library folders (6) and writes descriptions (7)
**before** Verify (8) and Activate (9). The transaction as built does not work that way,
and the difference is the property it exists to provide.

Nothing in the installation changes until the patched archive verifies. It is written
beside the original under a temporary name, loaded under Bitwig's own JVM, and moved into
place by a single rename. A failure anywhere earlier leaves an installation that was never
touched, rather than one that has to be repaired.

So the transaction is five steps:

1. Back up the archive and the description bundles
2. Prepare the installation (write the patched archive beside the original)
3. Verify
4. Activate (one rename, and the first moment anything changes)
5. Link library folders - skipped under `Copy` placement, per item 8

A plan now states which of these it will run, so the drawn list can show a skipped step as
not run without the app deciding for itself which one that is.

Placing documents and writing descriptions are *Update entries* work. They touch no part of
the archive and cannot disturb the tamper seal, which is exactly why they are not inside the
transaction that can fail. "Read the Core Registry" is not a step either: it happens while
the plan is computed, before the confirmation the user has not yet agreed to.

The drawn order has a consequence the design will not want: with "Cancel is allowed up to
Activate", a cancelled run has already copied the user's documents into place and rewritten
Bitwig's own description bundles. Under the real order there is nothing to undo.

---

## Round 2: depends on data nothing carries

These are ours to fix or to rule out, not the designer's. They are here because the design
is drawing values that no format currently produces, and it should not be redrawn twice.

### 5. Per-item provenance has nowhere to live

The item panel offers "a link to the merged change in the repository", and the mock carries
`orange-catalog #455`. The index entry carries uuid, kind, name, author, slug, version,
minimum Bitwig, licence, description, keywords, path, digest, size, homepage and supersedes.
There is no per-item reference to the change that merged it.

The index has a single `revision`, the commit the whole index was generated from. That
traces the index, not the item.

The claim is worth keeping - review is the only trust boundary, and saying so is the point -
but it costs a schema field. Either the index gains one per item, or the panel says
something it can actually show.

### 6. `Update available` cannot be computed from what is recorded

The update modal states "installed 2.0.3 -> catalog 2.1.0", and the README correctly notes
that an updatable item needs two version fields. Only one of them exists. A registration
records uuid, kind, name, library path, description and keywords. Not a version, and not
where it came from.

Comparing the document against the catalog's digest would only say "this differs from the
catalog's current file". It cannot separate `Update available` from `Changed`, which the
design is right to keep apart - different cause, different remedy.

The entry list has to gain columns. That is cheap and safe: the injected class already reads
the first four and ignores anything after them, so older prepared installations keep working
against a longer list. Accepted; noted so the design knows the requirement was real.

### 7. Derive provenance from the UUID, not the name

The fix for the Local and Catalog views drifting apart is right, and its key is wrong. An
entry is treated as catalog-sourced when "a catalog item of the same name" reports an
installed status.

Display names are explicitly allowed to collide in the repository, and the app renames
entries when they do, precisely because the name is not the identity. A UUID is exact, is
already in both records, and cannot be renamed. Matching on name will eventually mark the
wrong row - and it will do it first to the user who hit the rename path, who is the one
least able to explain what happened.

---

## Round 2: our bug, found by the design (fixed)

### 8. "Link library folders is skipped under Copy placement" is correct, and the library did not do it

The design assumes preparation skips the link step when placement is `Copy`. It was right,
and we were wrong: preparation linked all three kinds unconditionally.

The consequence is worse than a redundant step. `Copy` writes to the registered library
path, which resolves *through* the link into the user library - so with the links in place,
`Copy` and `Link` put the document in the same file, and the `Copied` placement state is
unreachable. The setting has no effect.

Fixed: preparation now takes the placement strategy, and a plan states which steps it will
run, so a step list drawn before the run cannot promise one that will be skipped. No design
change was needed - the design already had it right.

---

## Confirmed correct, round 2

Listed because each was a genuine risk.

- **The Catalog trust model.** Refusing an install on a digest mismatch, holding the failure
  on the row, and offering `Copy details` rather than `Retry` is exactly right. A hash
  mismatch is not a transient network condition and must not be retryable.
- **Install does not confirm and update does.** This matches how identity works: installing
  a new UUID cannot affect an existing project, and replacing the file behind one can.
- **Supersede as a separate item with a new identity**, both published, the offer navigating
  to a row that actually exists. The insistence that both items be in the catalog is the
  right call - the coexistence claim is the one thing this state has to prove.
- **Minimum Bitwig version stated whether or not it is satisfied.** The index carries it per
  item and the library can already filter on it.
- **Offline with a cached index is not an error.** Correct: the index is one small file, and
  items are 20 to 30 KB.
- **Search covering author in the Catalog and UUID in Local**, with the reasoning stated.
  The two questions really are different.
- **No UUID in a catalog row.** Right: it identifies a thing you already have.

---

## Round 3: found while building the window against revision 7

These came out of holding every rendered screen against the rendered mockup and
comparing the numbers, not from reading the bundle. Two are for the designer; the
rest are recorded because the next person to measure will find them too and should
know they were looked at.

### 1. The install bar is 42 tall, except on `noinstall`, where it is 50

`InstallBar.dc.html` is `padding:9px 12px` around a 24-tall row, which is the 42 the
whole app is drawn to. The shell does not import it for the "no installation" state:
it inlines a near-copy at `padding:13px 12px`, which comes out 50, so the one screen
a first-run user is most likely to see has a bar eight pixels taller than every other
screen. Nothing else about it differs. It reads as a copy that drifted rather than a
decision, and this draws 42 everywhere until told otherwise.

### 2. The `·` separator is the design's, and the app now draws it

The bundle uses a middle dot between the parts of one line - `Update entries ·
Bitwig may stay open`, the three extensions, `Step 3 of 5 · Verify`. This wrote a
full stop for a while, because the house rule keeps wide characters out of the
source. The codepoint is in both faces, so it is drawn as the design has it and
escaped in the source rather than typed. No question outstanding; noted because the
next reader will see `\u{b7}` and wonder.

### 3. What the bundle draws and this deliberately does not

- **Cancel, in the progress dialog.** Nothing can cancel a preparation yet. The
  dialog is drawn without the control rather than with a dead one - which is the
  shape the bundle itself draws from the Activate step onwards.
- **The row actions and a catalog row's status and `Install`.** The columns are
  reserved at the design's widths and left empty, because the design reserves them
  too: it hides those controls off hover rather than removing them.
- **The catalog detail's footer** - `Remove` and the primary control,
  `CatalogDetail.dc.html:89-98`. Both are behind `sc-if` in the bundle, so a panel
  without the bar is a shape it already draws. Installing from the catalog is not
  built, and removal is not built, so there is nothing to put in either slot.
- **`Remove entry` in the inspector** - `Inspector.dc.html:116`, the one action in
  that list the bundle draws unconditionally. It waits on the same removal work as
  the footer above. The two actions beside it are conditional in the bundle, so the
  group is drawn without this one rather than with it dead.
- **`Reviewed in <commit>` in the inspector** - `Inspector.dc.html:90-92`, under
  Source and behind `fromCatalog`. Not drawn, and not because of the layout: the
  entry list does not record which commit published an item. `Provenance::Catalog`
  carries the version and nothing else, and `entries.tsv` spells it as two columns,
  a version and the word `catalog` - so there is no revision to name and no URL to
  put behind it. Nothing in the running application produces a catalog provenance
  at all yet; every one in the tree is a test fixture. Recording the revision means
  widening a persisted format, and the first thing that would write one is
  installing from the catalog, which is where that decision belongs. The catalog
  detail *does* draw its own `Reviewed in`, off the index's `merged_in` - a
  different fact, about the item rather than about this machine's copy of it.
- **`Licences` on the About screen** - `AboutScreen.dc.html:64`, the second of the
  pair beside `Copy diagnostics`. Nothing in this build carries the text it would
  show. The plural is the point: the crate's own licence is one line in its
  manifest, and what a `Licences` control opens is the licences of everything
  linked into the binary - which nothing generates and nothing bundles. `Copy
  diagnostics` is drawn alone rather than beside a control that opens an empty
  sheet. Worth asking the designer whether the pair is meant to be a pair; if it
  is, the work is a build step that collects the dependency licences, not a
  layout.

### 4. 13.5px is tracked two ways, and only one of them can be drawn

For the designer. The bundle sets 13.5px/600 at five places, split two ways. At
`-0.015em`: `InstallBar.dc.html:29`, the bar's own title, and `ORNG Registry.dc.html:90`,
the "No installation selected" title the bar draws in its place. At `-0.02em`: all three
screen headers - `SettingsScreen.dc.html:30`, `RestoreScreen.dc.html:30`,
`AboutScreen.dc.html:30`. Nothing in the bundle uses 13.5 at any other tracking.

The split is exactly bar against screen, which is what makes it look deliberate rather
than a slip in one file.

`theme::font::tracking` answers from the face and the size, which is the whole reason no
call site can lose the tracking, and those are identical in both cases. So the derived
`-0.015em` is what both draw. The difference is 0.0675 of a pixel a character: half a
pixel across `Settings` and about one across `About ORNG Registry`, which is inside the
tolerance this is judged by. Raised rather than worked around, because keying tracking on
the call site would give every other run in the window a way to lose it - and if the two
are meant to differ, the cleanest fix is for one of them to be a different size.

### 5. Settings has no mockup for a machine with no installation

Also for the designer, and lower priority. The overflow is on the install bar in every
state, including "no Bitwig Studio found" and "this build could not be read", so Settings
is reachable from both - and the bundle draws one machine, with every path resolved.

This draws the rows anyway and writes `-` where there is nothing to say, so the group does
not change height with what went wrong, and the diagnostics report states what there is:
the root that was refused and why, or the places that were searched. That is a guess at
what the design would want. It is also the state somebody is most likely to be *in* when
they open this screen, so it is worth drawing on purpose.

