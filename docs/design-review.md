# Design review: Orange Registry

Validation of the design handoff against the app's actual behaviour and against the
Bitwig internals it depends on. Everything below was checked against a real Bitwig Studio
6.1 installation and against the library that implements this, not against the spec.

Round 1 is closed: all ten items were addressed, and several were answered better than
they were asked. Round 2 adds the Catalog view, and raises eight items - three factual,
one that contradicts a safety property of the transaction, three that depend on data no
format currently carries, and one that was our bug rather than the design's and is now
fixed.

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

`SettingsScreen` shows `Library/orange-registry.json`. It is `~/.orange-registry/entries.tsv`.

The path is not a detail here. Inside the installation's `Library` the list would be
destroyed by every Bitwig update - and "a Bitwig update costs one preparation, not one
operation per registered item" is the promise the whole architecture is built to keep. The
list lives outside the installation precisely so an update cannot touch it.

The extension is also load-bearing. The format is tab separated because the class injected
into the archive reads it at every launch, and `split("\t")` needs no parser, no dependency
and no error handling worth the name. It is not JSON and should not be drawn as JSON.

### 2. The backups path is wrong

`~/Library/Application Support/OrangeRegistry/backups` appears in Settings, in the plan
confirmation and on the Restore screen. It is `~/.orange-registry/backups/`, one directory
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
