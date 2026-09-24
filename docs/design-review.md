# Design review: ORNG Registry

Validation of the design handoff against the app's actual behaviour and against the
Bitwig internals it depends on. Everything below was checked against a real Bitwig Studio
6.1 installation and against the library that implements this, not against the spec.

Round 1 is closed: all ten items were addressed, and several were answered better than
they were asked. Round 2 adds the Catalog view, and raises eight items - three factual,
one that contradicts a safety property of the transaction, three that depend on data no
format currently carries, and one that was our bug rather than the design's and is now
fixed. Round 3 collects what building the window against revision 7 turned up, and its
last two items are open questions for the designer rather than findings. Round 4 is the
designer's answer to all of them, revision 8, and what it leaves to build.

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

Both halves have since landed. Version and source went in as format 2, and `Changed` needed
one more - format 3 carries the digest of the document as it was placed, because comparing
a file against the *catalog's* digest cannot tell an available update from a local edit, and
nothing else recorded what the entry was registered with. All four computed statuses are
drawn now. The one cost worth stating: a row written by an older build has no digest and
never reports `Changed`, and nothing can repair that, because hashing whatever is on disk
today would record the present as the past.

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

### ~~1. The install bar is 42 tall, except on `noinstall`, where it is 50~~

*Settled in round 4, C: 42 everywhere, and the shell now says so.*

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
  shape the bundle itself draws from the Activate step onwards. Its foot is that
  state's too: 12 around the percent with the bar centred on it, 37 tall against
  the 52 the control gives the `progress` scenario (`ORNG Registry.dc.html:280-293`,
  probed at both). The first pictures of it carried egui's own centring instead,
  six pixels low in a row the theme had floored at 26, and were re-recorded when
  the plan confirmation took the same skeleton.
- ~~**A catalog row's status and `Install`.**~~ *Drawn.* All seven states and the
  control each offers, transcribed from `CatalogRow.dc.html:68-76` into a second
  table in `status.rs` beside the entry list's ten. **The claim in the earlier
  version of this item was wrong and is worth recording**: it said the controls
  are hidden off hover "because the design reserves them too". They are not.
  `CatalogRow.dc.html:98` gates the control on there *being* one and never on
  hover - only its opacity moves - and that is right: an entry row's controls act
  on something you already have, and a catalog row's control is how you decide.
  The entry row's own actions were the other half of this item and are also
  drawn, `EntryRow.dc.html:44-60`, all five of them. What is still missing from
  that group is below.
- **The update modal, and so `Update` on a catalog row.** *Drawn in revision 8 and
  not built - round 4, A1, which also corrects this item.* The README is explicit
  that updating confirms where installing does not, and says what the modal has
  to carry: the item and the target version in its title, `installed 2.0.3 ->
  catalog 2.1.0` in mono beneath the lead, and the one sentence that matters -
  projects already using the device will use the new version. **Nothing in the
  bundle draws it.** There is no `.dc.html` for it, and the full-window mockup
  does not show it.

  So the row states `Update available` in the accent, as the design colours it,
  and offers no press. That is the same shape the progress dialog's missing
  `Cancel` takes, and here the case for it is stronger: Bitwig resolves a device
  by identity, so an unconfirmed update would replace the file under every open
  project - which is the one thing the design put a dialog in front of. Doing it
  without the dialog is not a smaller version of the design.

  Kept honest in the code rather than by intent: `Offer` has no `Update` variant
  at all, and the only press that writes a document is offered by `Available`,
  which by definition means the identity is not registered here. **For the
  designer:** the modal.

- ~~**The catalog's own toolbar**~~ - *Drawn.* `CatalogToolbar.dc.html`: the
  wide-scope search, the three kind facets, and `All / Installed / Updatable` as
  a three-state segmented control over `Published`'s table. Three things in it
  are the design's own and none of them is obvious from a picture.

  **The search field is capped at 320 where the Local toolbar's is capped at
  216**, and it says what it searches - name, author, description and keywords -
  because the question is a different one. Local answers *what is this thing I
  have*; the catalog answers *is there a thing that does X*, and the answer is in
  the description and the keywords rather than in the name. There is no UUID
  among them, for the reason a catalog row does not draw one.

  **The facet counts are taken after the install filter and before the kind
  filter**, which is the shell's own arithmetic at `ORNG Registry.dc.html:672` -
  it facets `installFiltered` rather than the drawn list. A facet answers "how
  many would I see if I switched this kind on", so narrowing it by the control it
  belongs to would make every count read either the number already on screen or
  nothing at all.

  **The query and the kind filters are shared with the Local toolbar and the
  install filter is not**, which is also the shell's: `:101` and `:111` hand both
  toolbars one `query` and one `kinds`, and only `catalogFilter` is the catalog's
  alone. Switching views keeps what was asked for rather than quietly widening
  it.

  One difference from the Local bar that reads as an omission and is not: **the
  kind chips keep their full 9px padding beside the panel**. `ListToolbar`'s
  `chip()` takes a `narrow` argument and `CatalogToolbar`'s does not, and the
  probe says why - this bar has one box fewer, so the field still comes to 82
  there, clear of the 80 a search field stops being worth having at.

- ~~**`Browse all` in the catalog's no-match empty state**~~ - *Dropped in revision 8,
  round 4, B4.* `EmptyState.dc.html:97`
  draws it as the alt beside `Clear filters`, and it is not drawn here. **The
  shell's own two handlers do the same thing**: `Clear filters` at
  `ORNG Registry.dc.html:770` sets `kinds: null, catalogFilter: null` and
  `Browse all` at `:773` sets `kinds: null, catalogFilter: "All"` - and `:637`
  resolves that null to `"All"`. So the second control offers the user nothing
  the first does not, and it is drawn as one rather than as two that read as a
  choice and are not. **For the designer:** if `Browse all` is meant to do something
  `Clear filters` does not - leave the search but drop the filters, say, or take
  the user out of a kind they had pinned - it needs saying, and then it is a
  two-line change.

- ~~**The cached catalog, and everything the bundle words around it**~~ -
  *Drawn.* All four: the action bar's summary gaining ` · cached` after the
  count, its note `Offline · installing a cached item still works`, the age
  beside the view switch (`Catalog from 12 days ago`, accent-coloured once
  stale) and the `Refresh` control that grows its label with it -
  `ORNG Registry.dc.html:465-469` and `InstallBar.dc.html:35-41`. The empty
  state for a window with nothing kept gained `Try again`, which
  `EmptyState.dc.html:90` draws and which had nothing behind it until there was
  something to refresh.

  **What is kept is the bytes and the signature over them**, in
  `~/.orng/catalog/`, never a re-serialised index: reading the pair back is the
  same `Index::verified` call the download went through, so a file edited under
  the user's home is refused on exactly the terms a tampered download is. The
  age is the file's own write time, and every successful fetch rewrites both
  files whether or not the bytes changed - so the age states when the catalog
  was last *confirmed* rather than when it last said something new.

  **The index is read on opening and checked on opening**, which the user asked
  for in as many words. Reading is two files and a signature, so the first frame
  has a catalog; the check behind it is on a worker and the window never waits
  for it. The older rule here - do not reach for a socket before being asked -
  was protecting against a window that hangs on a train, and this one does not
  hang. `ui-spec-catalog.md:201` also asks for a quiet automatic refresh on view
  entry; that is deliberately not a second trigger, because a launch already
  covers it and two would fetch twice for one look. *Agreed in round 4; the spec
  now says launch and `Refresh` only.*

  Two things in it are ours and are recorded in item 6 below: when an index
  stops being called current, and how a duration is written.
- **The entry row's overflow** - *Filled in revision 8 and not built - round 4, A2.*
  `EntryRow.dc.html:60`, the `ph-dots-three-vertical` control the design draws on
  every row in every state. Nothing in the bundle says what its menu holds: not the
  row section of the README, not the interactions section, not the full-window
  mockup. So there is a control to draw and no menu to put behind it, and a
  control that opens nothing is worse than a control that is not there. **For the
  designer:** what is in it? The five conditional actions are all already on the row, and the
  narrow grid drops the identity column rather than any control, so it is not
  obviously the overflow for a squeezed row either.
- **`Show factory entries`, and the whole factory section under it.** The user
  was asked and chose not to have it: "we don't need that for now." It comes off
  the plan rather than sitting on it, and this is the record of what that costs.

  It is not one control. `ListToolbar.dc.html:43-45` draws the checkbox - the
  `ph-check-square`/`ph-square` pair Settings uses, inline with no sentence under
  it - and `ORNG Registry.dc.html:131-138` draws what it opens: a second list
  section below the normal rows, with its own `Factory` heading, its own count,
  and rows that are inert. The caption at `:406-408` says what they are,
  "Bitwig's own 428 entries are recessed and read-only. Off by default."

  The cost of reading them is why it was never built in the first place: 428
  entries means parsing a class out of a 30k-entry archive, about a second, so it
  needs a worker of its own and the installation's section key resolved beside
  the `Destination` - `project-spec.md` 4.5. A toggle that stalled the window, or
  one that did nothing, is worse than no toggle.

  **Three things follow from the decision, and none of them is a bug.** The
  entry list is nine states rather than the design's ten: `Factory` came out of
  `status.rs` because nothing could construct it, and the bundle does not gate
  the row on that word anyway - `EntryRow.dc.html:117-120` flattens the cursor,
  drops the hover fill and hides the control group on a separate `factory`
  boolean. The Local toolbar is four boxes of the design's five and so three
  gaps of its four, which is what `App::local_toolbar`'s
  `BETWEEN_TOOLBAR_GROUPS` states; `widget.rs`'s search-field test still
  computes against the bundle's four, because what it pins is the formula and
  not this bar. And the diagnostics report has no `factory` line, which round 2
  already recorded - it is the same count and the same worker.
- ~~**The catalog detail's footer**~~ - *Drawn.* `Remove` at one end and the
  primary at the other, `CatalogDetail.dc.html:89-98`, 30 and 32 tall against the
  bar's 11 of padding. Both are still behind their own condition, as the bundle
  has them: `Remove` on exactly the three states that mean the item is registered
  here, and the primary on the four that offer a press. A superseded item draws
  no primary, because the notice above it already carries `See <replacement>` and
  two controls for one press is a panel disagreeing with itself. The `Remove` is
  the Local row's queued removal and not a second kind of one - an installed
  catalog item *is* a registered entry.
- ~~**`Reviewed in <commit>` in the inspector**~~ - *Drawn.*
  `Inspector.dc.html:90-92`, under Source and behind the same condition. The user
  was asked and chose to record it: `entries.tsv` is format 4, a tenth column
  between the version and the source, and `Provenance::Catalog` carries the
  revision beside the version. The cost was what the digest column measured - one
  column, one parser arm and a format number.

  **Recorded at install and never looked up**, which is the part worth keeping.
  The catalog goes on publishing, so an item installed today and superseded next
  month has nowhere left for the question to be asked. The catalog detail's own
  `Reviewed in` is a different fact, read off the index's `merged_in`: one is
  about the item as it is published now, the other about this machine's copy of
  it. A row registered before format 4 names no review and cannot be made to -
  the same shape as the digest, and for the same reason.
- **`Licences` on the About screen** - `AboutScreen.dc.html:64`, the second of the
  pair beside `Copy diagnostics`. Nothing in this build carries the text it would
  show. The plural is the point: the crate's own licence is one line in its
  manifest, and what a `Licences` control opens is the licences of everything
  linked into the binary - which nothing generates and nothing bundles. `Copy
  diagnostics` is drawn alone rather than beside a control that opens an empty
  sheet. Worth asking the designer whether the pair is meant to be a pair; if it
  is, the work is a build step that collects the dependency licences, not a
  layout.

### ~~4. 13.5px is tracked two ways, and only one of them can be drawn~~

*Settled in round 4, C: `-0.015em` everywhere, which is what the window draws.*

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

### ~~5. Settings has no mockup for a machine with no installation~~

*Drawn in revision 8 - round 4, A5, with two differences from what this draws.*

Also for the designer, and lower priority. The overflow is on the install bar in every
state, including "no Bitwig Studio found" and "this build could not be read", so Settings
is reachable from both - and the bundle draws one machine, with every path resolved.

This draws the rows anyway and writes `-` where there is nothing to say, so the group does
not change height with what went wrong, and the diagnostics report states what there is:
the root that was refused and why, or the places that were searched. That is a guess at
what the design would want. It is also the state somebody is most likely to be *in* when
they open this screen, so it is worth drawing on purpose.

### 6. What the catalog's bar says that the bundle does not

*Answered in round 4, B1: yes to all of it, and `Fetching...` goes on the row too.*

For the designer, and small. The bundle words this bar for ten catalog scenarios
and the app takes all ten as written. Two states it reaches are not among them,
and one sentence it now states has a rule underneath that the bundle does not
state.

**A fetch in flight.** Pressing `Install` starts a download, and the seven row
states have no word for one - `CatalogRow.dc.html` goes straight from the offer
to the outcome. So the press was followed by a second or two in which nothing in
the window had changed. The bar says `Fetching <item>` for that gap, in the
neutral tone, which is the tone the reading of a drop already takes: nothing has
been written yet, and `Registering` in the warn follows when something is. If it
belongs on the row instead, that is an eighth published state and a bigger
change than this one.

While that fetch is out, **the Local view's primary action is disabled**, with
`In progress` on its hover - the reason the bar's press already gives during a
run, so no new words. The fetch's second half is a run, and two runs started
together had one report over the other. The label keeps saying what the press
would do (`Apply 2 changes`), unlike a run's `Applying`, because nothing is
being applied yet. And a fetch that lands after the installation was changed
to one that cannot be read is a failed install whose detail reads `<item> was
fetched, and there is no installation to register it in`.

**The count under the `Installed` filter.** Two of the three positions are
captioned - `Catalog · 9 items` under `All` (`ORNG Registry.dc.html:433`) and
`Catalog · 1 update available` under `Updatable` (`:439`) - and the third is
drawn but never captioned. It reads `Catalog · N installed`, which is the same
sentence with the same substitution. Worth a look only because the pattern it
extends is the design's and the extension is not.

**And two things about the catalog's age, now that it has one.** The two
sentences are the bundle's and are taken as written: `Catalog updated 20 minutes
ago` while the index is current (`InstallBar.dc.html:61`) and `Catalog from 12
days ago` once it is stale (`ORNG Registry.dc.html:467`). Between them the
bundle states nothing, so two rules underneath are decisions.

*When an index stops being current.* A week. The bundle draws 20 minutes as
current and 12 days as stale and no line in between. Seven days because of what
reaching it now means: the window checks on every launch, so a week without one
succeeding is a machine that has been off the network for a week rather than a
catalog nobody has published to. If the right answer is a day, or a month, it is
one constant - `catalog::STALE_AFTER`.

*How a duration is written.* The largest unit that gives a whole number, nothing
below a minute, and `just now` under one - because a fetch that landed nine
seconds ago would otherwise read `0 minutes ago`, which reads as broken rather
than as recent. Days is the largest unit, so a year off the network reads `370
days ago` rather than `1 year ago`.

### 7. The Local bar's hidden-entries note: the sentence is drawn, the number is ours

*Answered in round 4, B2: it counts pending changes, and is a different sentence.*

For the designer, and smaller still. `ORNG Registry.dc.html:490` is the one Local
scenario whose note is not what a press would cost: under a search that matches
nothing the bar keeps its pending-work summary and the note reads `5 entries
hidden by the current filter`. The window now draws that line. Two things about
it are decisions rather than readings, because the bundle's notes are declared
per scenario and computed nowhere - `:828` is `note: a.note ?? ""`, so the
literal is copy and not arithmetic.

**When the line changes hands.** It is drawn only once the filter has left
nothing on screen, which is the state the bundle draws it in and the only one. A
list with rows still in it goes on being told what the press would cost, because
the bar has one note line and a list you can see does not need to be told what is
missing from it. The bundle draws no filtered-but-not-empty Local scenario, so
this is unproven either way; widening it is one condition.

**What the number counts.** The rows the list would have drawn, which is
`0` shown against all of them hidden - so it states how much of the list is
behind the filter. The bundle's own `5` is not that: the sample has ten entries
and none of them shown, and `5` is the count in `Apply 5 changes` on the same
bar. If it was meant to count the *pending work* that is out of sight rather than
the list, this is a different sentence and should be said so.

One consequence worth naming: in the preparing mode the note it replaces is
`Prepare install · a backup is written first`, and that warning is gone for as
long as the filter is empty. The summary keeps the warn tone that only that mode
takes and the button still reads `Prepare installation`, so the mode is still
stated - but if the cost should outrank the count there, it is one arm in
`App::summary`.

### 8. The plan confirmation: the lines are the bundle's, and four things in them are ours

*Answered in round 4, B3: yes to the four, one line more, and no outline.*

For the designer, and the largest of the three that are about words.
`ORNG Registry.dc.html:223-252` draws the plan over one scenario - two documents
staged, one removal queued, a stock installation with no backup, the link
strategy - and the window draws the same dialog over whatever the press is about
to do. Every line derives from the rows and the machine: the first names the
backups directory for this build, the third and fourth count the rows that are
ready and the rows that are queued, the fifth counts by kind and folder, and the
sixth counts the links. A line about nothing is left out, as the action bar leaves
out a part that is nothing. Four things are said that the scenario does not say,
each because the data says it:

- **A backup that already exists is kept**, and the line says so: `The archive
  and the description bundles are already backed up in ..., and the patch is
  built from that copy.` The library's `Plan` takes that decision - it patches the
  pristine copy - and its `backup_exists` was written for this sentence. Reached
  after a restore, which puts the archive back and leaves the copy where it was.
- **A removal names what happens to its file**, in the words its own control
  used: `1 entry removed: OLD REVERB. The document file is kept.` The bundle's
  line stops at the name and the preference that decides the rest lives in
  Settings. It is the one irreversible thing a press can do, on the one dialog
  that exists to say what the press does.
- **The links are counted.** The bundle writes `1 library link created`;
  preparation links all three kinds whatever is registered (decision 6.3), so
  the line reads `3 library links created inside the installation's Library
  folder, once the archive is in place`, and `The installation's library folders
  are already linked to the user library.` when a restore left them standing.
  `after the transaction completes` became `once the archive is in place`, because
  linking is the transaction's last step and not something after it.
- **The entries already on record get no line.** The bundle's scenario has
  registered rows under the pending ones and says nothing about them, so neither
  does this. On a re-preparation the press also rewrites their description
  bundles, which is what the action bar's `6 entries to restore` is about; if the
  plan should say so, it is one line.

And two pixels: the bundle's press carries a one-pixel accent outline on the
accent fill, which makes it 34 where the action bar's is 32, and the dialog draws
the action bar's control.


### 9. Administrator rights: nothing in the bundle draws them, and Windows needs them

*Answered in round 4, A3, which also finds the paragraph on declining wrong.*

For the designer, and Windows only. An installation under `Program Files` is not
writable by an ordinary account, and **both** modes reach inside it: preparing
writes the archive, and every entry change rewrites the description bundles that
live in the installation's `localization` directory (4.4). The bundle draws no
state for any of this - no consent, no elevated run, no refusal - so everything
below is ours.

*On Windows the press is not blocked.* A running process cannot gain rights, so
preparation hands the work to a second copy of this application that Windows
starts with them, and the consent dialog the user answers is the system's own.
The window draws exactly what it draws for any other run: the progress dialog,
fed from the child. No dialog of our own is drawn, which is the decision - one
in front of Windows' would be two dialogs for one question.

*But the press says it will ask.* A press that ends in the consent dialog wears
a shield before its words - `ph-shield`, light, the same size and ink as the
arrow after them. It is Windows' own convention for a control that elevates,
and it needs no words. It is on the action bar's primary, both
`Prepare installation` (which opens the plan, one step before the dialog) and
`Apply N changes`, and on the plan's own `Prepare installation`. Before it, a
preparation that would raise the consent dialog drew the same plan as one that
would not. Only where the platform can ask and the installation is not this
account's: everywhere else the press is refused or needs nothing, and wears no
shield. **Not yet on** the other presses that elevate the same way - a catalog
row's `Install`, a row's `Locate`, and the Restore screen's press - whose
controls are the bundle's small ones, and where a shield has to be placed by
the designer rather than by us.

*Except an edit in the inspector, which waits for a press.* Every other edit is
written the moment a field is left, and announced nowhere. Here that moment would
raise the consent dialog because the pointer left a text box, which is a dialog
people learn to dismiss unread. So the words wait behind a warn-toned banner,
the user's own design, and only its wording is ours:

- Title: `Save the changes to <name>?`
- Body: `<installation> is not writable by this account, so saving asks Windows
  for administrator rights. The description and keywords Bitwig shows are kept
  inside the installation.`
- Two buttons, `Cancel` and `Save`, and no dismiss mark: a third way out would
  say neither. `Cancel` drops the words and the entry is what it was.

This is the one new thing the window draws for rights, and it is drawn before
the press rather than in front of Windows' dialog, so the decision above holds.
Only on Windows: elsewhere there is no `Save` that could succeed, so the edit is
written, refused, and reported as `The change was not saved.`

*Declining is not a failure.* *Not true of the window until `0650537` - round 4,
A3.* `ShellExecuteEx` answers `ERROR_CANCELLED` when the
consent dialog is dismissed, and the banner says `administrator rights were
declined, so nothing was changed` rather than reporting an error. It is the one
outcome where the user has already been told what they did.

*Where the platform cannot ask, the press is refused.* macOS and Linux have no
way for an application to ask for more rights that does not mean installing a
privileged helper, and an installation under `/Applications` is the user's own
anyway. So there the banner stands in the action bar's way and says `This
installation is not yours to change.`, naming the directory that refused. It
offers no control, for the reason the unrecognised tamper guard offers none:
the remedy is the installation's permissions or the account this runs as, and
neither is a press.

*What is checked, and how.* Whether the installation can be written is answered
by writing: a file created and removed in each of the three directories a
modification touches - the archive's, `localization`, and `Library`. Permissions
are not one model across the three platforms and cannot be read as one, and
`Permissions::readonly` reports the DOS read-only attribute rather than an ACL.
It is asked once when the installation is read, beside whether Bitwig is running,
because both are conditions on a press rather than facts about the list.

---

## Round 4: revision 8, the answers to the round-five letter

The letter is `design_handoff_orng_registry/design-round-4-changes.md` and the reply
is `design-round-5-answers.md` beside it; the bundle came back as revision 8. This was
read against the diff of the `.dc.html` files and not against the reply alone, and
where the two differ it is said. Each item below is one of three things: a **change**
to make, a **yes** to what the window already draws, or **open**.

### A1. The update modal - change

`UpdateModal.dc.html`. The title and lead as the README had them, the lead now naming
the kind (`this modulator`), and one new thing: with Bitwig open, an info-toned strip,
`Bitwig Studio is open, so it keeps playing 2.0.3 until you restart it.` - after which
the row reads `Pending restart`, so the two agree. The shield leads `Update` on
Windows without rights. The detail panel's `Update...` opens the same modal.

**Round 3 item 3 was wrong about this.** It said neither a component nor the
full-window mockup drew the modal. The mockup did: revision 7's shell carried it
inline, state `catalogupdateconfirm`. Only the component was missing.

Not built. `Offer` has no `Update` variant, and an update replaces a registered
document by UUID, which no press does yet.

### A2. The row's overflow menu - change

Kept, and filled: `Rename...`, `Copy UUID`, `Show in catalog` - the actions that do
not depend on the row's state. `Copy UUID` is the only way to the UUID once the narrow
grid drops that column. `Rename...` is shown disabled on a catalog-sourced row
(`A catalog item keeps the catalog's name`) and left out on `Pending removal`; `Show in
catalog` is on catalog-sourced rows only. No `More` on a `Rejected` row. The menu is
the install bar menu's surface, row and `--shadow-menu`, at `top:31px; right:10px`,
184 wide at least, and the pointer leaving the row closes it (`EntryRow.dc.html:67-80`).

### A3. Rights on Windows - yes to all five, and five changes

Yes as written: no dialog of our own, the shield on the action bar's primary and the
plan's press, the Save question's words, the refusal banner where the platform cannot
ask, and a declined consent dialog reading neutral.

**That last one is not what the window does, and round 3 item 9 says it is.** A
declined dialog comes back from `elevate::run` as an `Err` like any other, becomes
`Outcome::Failed`, and is drawn in the error tone under the errand's failure title
with `Copy details` - `administrator rights were declined, so nothing was changed` is
only what that control copies. The letter repeated the claim. The designer's answer,
neutral and dismissible and not an error banner, is therefore a change.

*Drawn, `0650537`.* A run now stops with `elevate::Stopped`, `Declined` or
`Failed`, and a declined one is `Outcome::Declined` for any press, a restore
included. The title is the sentence the letter quoted; the line under it is ours:
`Windows asks for them because this installation is not writable by this account.
Nothing was started, so there is nothing to undo.`

The rest:

- **The shield leads, and replaces a leading glyph.** On a catalog row's `Install`,
  `Update` and `Retry` (`gap:6px` inside the 24-tall control), the modal's `Update`,
  the inspector's `Save`, `Rename` for a registered entry, and `Restore this backup`,
  which drops its clock for it. *Drawn on `Install`, `Retry` and the Restore press,
  `73ccdb5`*; the other three come with A1, A3's footer and A4. **Open:** the
  catalog detail panel's `Install` elevates the same way and is not in the list,
  and `CatalogDetail.dc.html` takes no `elevate` - so it wears none yet.
- **Glyph-only controls do not wear it.** `Locate` keeps its glyph and its tooltip
  becomes `Locate file · asks Windows for administrator rights`. The staging controls
  write nothing until Apply, and Apply wears it. *Drawn, `73ccdb5`.*
- **The Save question moves into the inspector's footer**, below the fields it saves,
  rather than across the bottom of the window where the app draws it now: warn wash,
  `11px 12px 12px`, 28-tall `Cancel` and `Save`. While it is open the panel's close
  control is dimmed to .4 with `Save or cancel the changes first` on hover.
- **Not in the reply, only in the shell**: state `windowslocal` gives the action
  bar's note as `Update entries · asks for administrator rights` where the window
  says `Bitwig may stay open`. Taken as a change. *Drawn, `f34d654`.*

### A4. Renaming - change, and the largest

`RenameDialog.dc.html`. Reached from the overflow, from the inspector's action list
(`Rename...` above `Reveal file`, not on a catalog-sourced, `Rejected` or `Pending
removal` entry), and from a name conflict's row. The inspector's name field stays
read-only. A catalog-sourced entry cannot be renamed: an update replaces its document
by UUID and would bring the catalog's name back.

A conflict now says which kind it is (`EntryRow` `conflict: "name" | "uuid"`). A name
conflict offers the pencil, accent, in place of the fingerprint; a UUID conflict
keeps `Assign new UUID`. The dialog: a field; while the name is taken, an error
outline and `A registered device is already called DISPERSER.` in `--err-text`, and
`Rename` disabled with `Choose a name no other entry uses` on hover; the rule and
why; and when it is written - for a staged document when Apply runs, for a registered
entry on the press, so it wears the shield.

**Open, and ours rather than the designer's:** `staging::objection` raises five
collisions and the design names two kinds. By what settles them, four are name-shaped
- a name a registered entry holds, another dropped document with the same name, and
the two file collisions, since the file name is made from the display name - and one
is UUID-shaped, two dropped documents with one identity. The design's reason words
(`Name already used by a registered device`) are also not ours (`DISPERSER is already
registered under another identity`). Both are for when it is built.

The README's two sentences are corrected: the app never renames by itself.

### A5. Settings with no installation - yes, with two differences

Drawn as state `settingsnoinstall`. Rows keep their height; `Browse` stays. Two things
differ from what the window draws. The placeholder is an em dash in `--ink-3` where
the window writes `-`, which it has done for every such value since revision 7
without recording why. And the report keeps all its lines, with the dash on each one
that needs an installation, its first reading
`install    not found · searched /Applications, ~/Applications` - where the
window's has two lines, `install none selected` and `searched`.

**The dash stays `-`**: the user's decision on 2026-09-24, over the design's em dash.
*The report is drawn, `8234b13`*, with the empty path rows in `--ink-3`. It leaves
out two of the bundle's lines, `entries` and `placement`: neither depends on an
installation, and a session that found none carries neither the home nor the
preferences to state them from. The first picture of this state is
`settings-no-installation.png`.

### A6. The inspector while it cannot write - change

A footer where the Save question sits: info wash, a 6px dot in `--ink-2`,
`Saved when <the work> finishes` over `Your changes wait here until the other work is
over, so the panel stays open until then.` The close control dimmed as in A3, with
`Your changes are saved when <the work> finishes` on hover. The work is named: `the
preparation`, `the registration`, `the download`.

### A7. A drop during a run - yes

No drop target, and no words. The platform's no-drop cursor is the refusal.

### B1. The catalog's action bar - yes, and one change

Yes to all six. The change is that `Fetching...` also goes on the row, as an eighth
published state: `--ink-2`, no action (`CatalogRow.dc.html:54`). *Drawn, `92490f0`*,
for as long as the fetch is out.

### B2. The hidden-entries note - change

It was meant to count pending work out of sight, because Apply acts on rows the user
cannot see. `N changes hidden by the current filter`, N the pending changes the
filter hides, shown whenever N is above zero and not only on an empty list. In the
preparing mode the cost note keeps the line. *Drawn, `6778a80`.* A change is what
Apply counts - a ready row or a queued removal - so a row still to fix is not one.

### B3. The plan confirmation - yes, and one line

Yes to the four. One line added after the registrations: `6 entries already
registered keep their UUIDs; their description bundles are written again.` The press
has no outline, so the window's 32 is right. *Drawn, `53f61a2`*, counting the entries
the press neither registers again nor removes. The singular is ours: `1 entry already
registered keeps its UUID; its description bundles are written again.`

### B4. `Browse all` - yes

Dropped from `EmptyState.dc.html`.

### C. Values - yes to three, one change

- 13.5px is `-0.015em` everywhere - yes, round 3 item 4 settled.
- The install bar is 42 on `noinstall` too - yes, round 3 item 1 settled.
- `--shadow` is `.85` - yes.
- ~~**The light shadows are stated, and not by ratio.**~~ *Drawn, `833ffa3`.*
  `--shadow-menu` is `0 12px 28px rgba(0,0,0,.16)` in light, `--shadow-panel`
  `-14px 0 32px rgba(0,0,0,.12)`. The window derived `.18` and `.13` and kept the
  dark geometry; the geometry moves too. `CatalogDetail.dc.html:21` still writes the
  dark panel shadow as a literal; the token is taken as the intent.

### Not drawn, on purpose - yes, with one change

- **No `Cancel` in progress: yes, and the note changes.** Up to Activate it reads
  `Nothing in the installation changes until the patched archive verifies.` and no
  more; from Activate on, `Once the preparation completes, undoing it means
  Restore.` The window draws one note for every step, and its second sentence is not
  the bundle's. *Drawn, `a917bff`.*
- Factory entries: yes.
- `Licences`: yes for now. A public release has to ship the notices for Inter, Iosevka
  and Phosphor, so it comes back before 1.0.
- No refresh on entering the Catalog view: yes. `ui-spec-catalog.md` section 7 now
  says launch and `Refresh` only.
