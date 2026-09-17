# Orng Tools - project specification

The stable reference for what this project is, how it works and why it is built the way it
is. Where this document and another disagree, this one is wrong and should be fixed; it is
meant to be the thing that keeps the vision from drifting, not a record of a past plan.

Companion documents: `ui-spec.md` (what the app's interface must do), `design-review.md`
(validation of the visual design against it), `AGENTS.md` (how code in this repo is
written).

---

## 1. What this is

Bitwig Studio only fully trusts content it knows by identity. A `.bwdevice`,
`.bwmodulator` or `.bwmodule` that a user made themselves has a UUID, but the installation
has never heard of it, so projects cannot recall it reliably.

**Orng Tools** is a Rust workspace that closes that gap:

- a set of libraries for reading Bitwig documents and preparing a Bitwig installation to
  accept custom identities, publishable so other people can build on them;
- **Orng Registry**, an application that lets a user register their own content with one
  drag and one button;
- **Orng Catalog**, a curated public repository of community content that Orng Registry
  can install from directly.

The user-facing promise is small and should stay small: *your own devices behave like
Bitwig's own devices, and keep working across Bitwig updates.*

---

## 2. Scope

In scope:

- Reading the identity of Bitwig device, modulator and Grid module documents.
- Giving a document a new identity when the user explicitly asks.
- Registering identities with a specific Bitwig installation.
- Placing documents where the installation resolves them.
- Making registered content appear and be searchable in Bitwig's browser.
- Fetching, installing and updating content from Orng Catalog.
- Undoing all of the above.

Out of scope, and intended to stay that way:

- Editing device contents: panels, DSP graphs, Nitro code. That is `bwmodule-parser`'s
  territory and a different product.
- The donor-UUID method, which reassigns a native device's identity to custom content. It
  sacrifices a stock device per custom one and makes projects non-portable between users
  with different donor mappings.
- Anything that unlocks Bitwig functionality the user has not paid for. See section 9.
- Accounts, telemetry, update checks, or any network traffic beyond Orng Catalog.

---

## 3. Vocabulary

Several things here are near-synonyms in ordinary speech. Each word below has exactly one
meaning in this project, and "catalog" means only the public repository.

| Term | Meaning |
| --- | --- |
| **Core Registry** | Bitwig's internal list of every identity it treats as native. What preparation teaches to read our entries. |
| **Bitwig library** | `Library/` inside the installation: `devices/`, `modulators/`, `modules/`. Where registered paths resolve. |
| **User library** | The user's own content folder, under `Documents` or `$HOME`. Survives Bitwig updates. |
| **Orng Catalog** | The public repository of community content. The only thing this project calls a catalog. |
| **Entry list** | This project's durable record of what it has registered. The prepared installation reads it at startup. |
| **Kind** | `Device`, `Modulator` or `Module`. Fixed by Bitwig; a property of a document, never a user choice. |
| **Registration** | One piece of custom content as this project records it: identity, kind, name, path, description, keywords. |
| **Prepare** | The one-time operation that modifies an installation. Needs Bitwig closed. |
| **Update entries** | Every other change. Touches no part of the installation. |

---

## 4. How registration works

Everything in this section was verified against a real Bitwig Studio 6.1 installation and
is exercised by tests in `bitwig-registry`.

### 4.1 The Core Registry

One class holds every native identity: 428 entries on 6.1, being 152 devices, 43
modulators and 233 Grid modules. Each is registered by a call taking
`(UUID, display name, kind, library path, boolean)`, where the kind is a constant of an
enum whose three members are literally named `DEVICE`, `MODULATOR` and `MODULE`.

A registered library path resolves inside the Bitwig library, so
`devices/My Devices/NAME.bwdevice` means exactly that directory inside the installation.

### 4.2 The entitlement grants

Bitwig checks a UUID against a per-kind grant map before treating content as available:

```
allowed = everythingAllowedFlag || grants.get(uuid) != null
```

A full Bitwig Studio licence sets the flag and the check short-circuits, so custom
identities pass untouched. A restricted edition does not set it, and an unknown UUID then
fails **both** when filtering library content and when instantiating a device - so on those
editions the content is absent or inert rather than merely unsearchable.

Preparation therefore seeds the user's own UUIDs into the grant maps. It adds one entry
per registered identity and **never touches the flag**. That distinction is the whole
ethical difference: the flag would unlock Bitwig's entire device set, while an entry
covers exactly one identity belonging to a file the user already has. See section 9.

The three grant maps are seeded identically rather than resolving which belongs to which
kind. A UUID identifies exactly one document, so a device's identity is never asked about
as a modulator; seeding all three is equivalent in effect and avoids a three-hop resolution
chain that would be the most fragile part of the system.

### 4.3 The tamper guard

Bitwig verifies its own archive at runtime and degrades audio in every project if the seal
is broken. Any modification trips it, so disarming it is part of preparation - one
operation, not an option. The guard compiles to a value load, a comparison against a
constant, and a branch; replacing the load with a zero makes the comparison always take the
normal path.

An unrecognised guard is refused, never edited blind.

### 4.4 Descriptions and search keywords

These are **not** in the archive. Bitwig reads them from properties bundles in the
installation's `localization` directory, keyed by display name:

```
device.glue_comp.desc=...
device.glue_comp.keywords=glue comp dynamics
```

Writing them requires no bytecode work and does not disturb the tamper seal. A missing key
yields an empty keyword array, which is why unregistered content is silently unfindable
rather than broken.

Bitwig ships six localised copies of each bundle, each a complete translation. The loader
builds a narrowing list of candidate files and merges them base-first, letting the
locale-specific file overwrite key by key - so a key present only in the base file survives
in every language. One file per kind is enough, with no per-language work.

### 4.5 Document identity

A document carries its own UUID, display name, description, category and creator. Bitwig
writes three serializations and all three occur in the wild:

| Serialization | Where it comes from |
| --- | --- |
| Text | Relaxed JSON. Development builds and some authoring tools. |
| Plain binary | Ramona binary, unencrypted. |
| Encrypted binary | Ramona binary behind a stream cipher. Factory content. |

All three are read. A new UUID is applied by splicing, not re-serializing: the identity is
fixed width in every encoding, so the surrounding bytes are untouched and the operation is
exactly reversible.

Custom content is commonly identified by a **name-derived (v5) UUID**, while Bitwig's own
content uses random (v4). Name-derived means two authors who pick the same device name get
the same identity deterministically - collisions are a likely event, not a theoretical one.
Section 7 depends on this.

### 4.6 Where documents live

A registered path resolves inside the installation, which a Bitwig update replaces. So by
default documents live in the user library and the installation's folder is linked to it;
the documents then survive an update and only preparation has to be repeated. Copying into
the installation is offered as an alternative for users who prefer self-containment and
accept losing the documents on update.

Preparation links **all three kinds** regardless of what is registered, so that entry
changes can never need to write into the installation. See decision 6.3.

---

## 5. Architecture

### 5.1 Crates

Layered bottom-up; each knows nothing of the layers above.

| Crate | Responsibility |
| --- | --- |
| `bitwig-install` | Locating an installation, the user library, the settings directory, the bundled JVM. Detecting whether Bitwig is running. |
| `bitwig-document` | Reading and re-identifying documents. All three serializations. No knowledge of installations. |
| `bitwig-classfile` | Class-file and archive surgery. Constant pool scanning and rewriting, bytecode editing, archive rewriting. No knowledge of Bitwig. |
| `bitwig-registry` | Locating Bitwig's internals structurally; reading the Core Registry; the tamper guard. |
| `orng-catalog` | The catalog repository format: manifests, the index, the ownership and identity rules. No knowledge of installations. |
| `orng-tools` | The facade: registrations, the entry list, description bundles, document placement, the transaction. |
| `orng-registry` | The application. |

The split is by what each layer knows, not by convenience. `bitwig-classfile` has no Bitwig
concepts in it and is independently useful; `bitwig-document` needs no installation.

`orng-tools` depends on `orng-catalog`, in that direction only. A registration records
where its document came from (5.2), and a catalog item's version is what that record holds,
so the two must agree on the version type rather than each declaring one. The catalog crate
stays free of installation concepts and of this one, which is what keeps it, and the lint
binary built on it, permissively licensed.

### 5.2 A prepared installation

Preparation is a **fixed edit that does not vary with what is registered**:

- a small class injected into the archive,
- one call added to the Core Registry's initialiser, after its own registrations,
- one call added to the entitlement constructor, after its grant maps exist,
- the tamper guard disarmed,
- two accesses widened so that direct calls verify: the register method, and the grant row's
  constructor, which the injected class builds one of per identity.

Both calls go at the very end of a method that is straight-line and ends in a single
`return`, so the edit cannot change a branch target, invalidate a stack map frame, or run
before the state it needs exists.

The injected class knows nothing about Bitwig by name. It is compiled against placeholders
and retargeted at patch time (5.3), and the grant maps are read at the call site and passed
in as plain `HashMap`s rather than read inside it - so every obfuscated field name stays in
the class that already knew it.

It must also never throw. It runs inside a class initialiser and a constructor that Bitwig
cannot start without, so a failure there would cost the application rather than a device.
It catches everything and prints instead, and preparation refuses if verification sees it
print (5.4).

At startup the injected class reads the entry list from a file this project owns and
registers each row. Consequences that matter:

- Adding, removing or renaming content rewrites a text file. No archive work, no backup, no
  requirement that Bitwig be closed.
- A Bitwig update costs **one** preparation, not one operation per registered item.
- The archive is never a source of truth for what is registered, so it cannot drift from
  the record and there is no half-applied state to detect.

The list is tab-separated, and the injected class reads it with `split("\t")`: no parser,
no dependency, no error handling worth the name in a class that must not throw. It reads
the **first four columns** - identity, kind, name, library path - and accepts any row with
at least that many. Columns after them are the application's business:

| | |
| --- | --- |
| 1 to 4 | What the injected class registers |
| 5, 6 | Description and search keywords, applied by the app when it writes the bundles |
| 7, 8 | Version and source (7.6), which only the app reads |

That asymmetry is why the list can grow: an installation prepared before a column was added
keeps working against a longer list, so a format change costs a parser change here and not
a re-preparation. A marker line states the format, and a reader that does not know the
number refuses the file rather than reading it under a guessed layout. Source is the last
column and never empty, because the column that goes missing is the empty one an editor
stripping trailing whitespace would eat.

### 5.3 Obfuscated names are never persisted

Bitwig's internal names change on every release. Nothing in this project writes one down.

Every anchor is a user-facing string, an unobfuscated enum constant, a JDK type inside a
descriptor, or a bytecode shape:

| Target | Anchor |
| --- | --- |
| Core Registry class | hundreds of pooled strings shaped like `devices/*.bwdevice` |
| Register method | descriptor `(UUID, String, <enum>, String, boolean)` |
| Kind enum and its members | the enum constant names, which `Enum.valueOf` keeps readable |
| Entitlement class and grant maps | the shape of the predicate: flag test, map lookup, null check |
| Grant row type | the type constructed from a UUID in the constructor |
| Tamper guard | a stable UI string, then the comparison shape |
| Build identity | the `"<version> - <40 hex>"` format |

Names are resolved at patch time, in Rust, and written only into the injected class's own
constant pool - inside the archive they describe, destroyed with it when Bitwig updates.

**Resolution fails closed.** An anchor that is missing or ambiguous is reported by name; a
build that changed shape is refused, never guessed at. This is the difference from
allowlisting known builds: an unseen release usually works on the day it ships, and a
genuinely changed one fails loudly instead of mispatching.

### 5.4 Verification

Bitwig ships a JRE with no compiler. Before a prepared archive replaces the installed one,
a small class is put on the classpath ahead of it and asked to load every class the patch
edited. Loading links a class, linking is what runs the JVM's own verifier, and forcing
initialisation then runs the registry's several hundred registrations for real. If that
does not exit clean, the archive is discarded and the installation is never touched.

That class is assembled at run time from Krakatau assembly held in this repository, so
producing it needs no JDK and the bytes are reviewable as text. It takes the class names as
arguments rather than holding them as constants, which is what keeps it fixed across
builds - nothing in it is retargeted. It goes in a temporary directory, never the archive.

The class that is injected is verified the same way, and its own initialisation is what
runs the two calls preparation added. A helper that cannot reach what it was retargeted at
catches the failure rather than throwing, because at run time it must never stop Bitwig
starting - so verification reads what it printed as well as how it exited. A clean exit
with nothing said is the only result that counts as verified.

The order of a preparation follows from this:

1. Back up the archive and the description bundles.
2. Write the patched archive beside the installed one, under a temporary name.
3. Verify it.
4. Move it into place. One rename, and the first moment the installation changes.
5. Link the library folders for all three kinds, unless documents are being copied into the
   installation instead (6.3). A plan states which steps it will run, so a step list drawn
   before the run cannot promise one it will skip.

Everything that can fail happens before step 4, so a failed preparation leaves an
installation that was never touched rather than one that has to be repaired.

---

## 6. Decisions of record

Recorded with their reasons so they are not re-litigated.

**6.1 Data outside the archive, not baked into it.** The alternative appends bytecode per
entry, so the patch grows with the user's library, removal needs a rebuild from backup, and
the identity list lives in two places. A fixed patch plus an external list makes the archive
a pure consequence of the record.

**6.2 Two apply modes.** Preparation is rare, modifies the installation, needs Bitwig
closed, writes a backup, and is worth confirming. Everything else is a file write. Treating
both as heavyweight would make routine work feel dangerous and train users to click through
the dialog that matters.

**6.3 Preparation links all three kinds, unless documents are being copied.** Otherwise the
first modulator added to a device-only installation would have to create a folder inside
the installation - a write that can demand authorisation, during the mode that promises
never to block.

Under the copy strategy the links must not be made at all. A registered path resolves
inside the installation's `Library`, so a linked folder resolves straight back out into the
user library and a document "copied into the installation" lands in exactly the file the
linked strategy would have used. The setting would read as a choice and make none. Copying
is the user asking for entry changes to write into the installation, which is the invariant
this decision protects, so opting out of it opts out of the link as well.

**6.4 Entitlement: grant entries, never the flag.** Section 4.2.

**6.5 Identity is never reassigned implicitly.** A UUID is how a saved project finds its
device. Reassigning one already in use silently breaks every project referencing it, so it
is offered only before an identity has been registered.

**6.6 Structural resolution over pinned builds.** Section 5.3.

**6.7 The document is the source of truth for identity.** Anything derivable from a
document is read from it rather than restated alongside it. Applies to the entry list and
to Orng Catalog manifests alike.

**6.8 The backup is the patch source, not the installed file.** A backup is taken once per
build and never overwritten, and every preparation of that build patches it rather than
whatever is installed now. Preparing twice then yields the same archive byte for byte
instead of stacking a second copy of every edit, and restore always has an unmodified
original. An installation that is already modified with no backup to work from is refused,
because there is then nothing pristine to patch.

---

## 7. Orng Catalog

A public repository of community content that Orng Registry installs from, so users get a
curated view of what exists instead of hunting for downloads.

### 7.1 What it actually is

Not a file host. Content is 20 to 30 KB per item; the entire Bitwig factory device set is
4.7 MB. Hosting is a non-problem.

Orng Catalog is an **identity authority**. Its job is to guarantee that a UUID means one
thing, permanently, across contributors who do not know each other. Every rule below
follows from that.

### 7.2 Layout

```
orng-catalog/
  content/
    <author>/
      <slug>/
        <name>.bwdevice        the document; any of the three kinds
        orng.toml            what the document cannot say
        README.md              optional
  owners.toml                  author -> GitHub account
  index.json                   generated, never hand-edited
  .github/workflows/
```

**One content root, grouped by author, not one root per kind.** A document states its own
kind and the index republishes it; a path that stated it too would be a third copy to keep
in step, against decision 6.7. Ownership is also per author, so an author owns one prefix
rather than three, and their items stay together for whoever reviews them.

`orng.toml` carries only what is not already inside the document - version, author,
licence, minimum Bitwig version, homepage, superseded identities. Identity, name, kind,
description and category are read from the document itself, per decision 6.7, and CI fails
a manifest that tries to restate them.

Content lives in the tree as plain files. **Not submodules:** they would require a git
implementation in the app, make PRs into pointer bumps whose content CI cannot see, and let
a contributor force-push their own repository to change what an approved pointer resolves
to. For something that installs into a DAW, that is the wrong trust model.

### 7.3 Identity rules

1. **A UUID is permanent.** Once merged it is bound to that item forever.
2. **Uniqueness is checked, never assumed.** Name-derived UUIDs collide deterministically
   (section 4.5), so CI asserts global uniqueness and new submissions should use random
   identities or a per-author namespace.
3. **Content changes under a stable identity are constrained.** Bitwig resolves a native
   device by UUID to a file in the Bitwig library; the project stores parameter values and
   the identity, while the structure comes from that file. Republishing changed structure
   under the same UUID therefore reaches back into projects that already use it. So:
   compatible changes keep the UUID and bump the version; anything altering the parameter
   set or its order takes a **new** UUID in a new directory, with the old one left
   published so existing projects still recall, linked by `supersedes`.
4. **Display names are unique in practice.** Bitwig's browser is flat and matches on name
   and keywords. The repository may hold collisions, but the app refuses to register two
   entries under one display name and offers a rename - free, since the name is a field in
   the entry list and not part of the identity.

### 7.4 Governance

GitHub has no path-scoped write permission, and `CODEOWNERS` grants no access - it routes
and can require review, nothing more. So per-directory ownership is enforced in CI:

- Nobody has write access. Everything arrives as a fork pull request.
- A required check compares the changed paths against `owners.toml` **as it exists on the
  base branch**, and fails if a pull request touches a directory its author does not own.
  Reading the pull request's own copy would let a contributor add themselves as owner of
  someone else's directory in the commit that modifies it.
- The author id in a path is a lookup key, never a credential. It resolves through
  `owners.toml` to a GitHub account, and the check compares that against the account
  GitHub authenticated on the pull request. The **numeric account id** is what is compared:
  a login can be changed by its holder and the old one can then be claimed by someone else,
  while the id is immutable and never reused. The login is stored beside it so a human
  reading the file knows who it is.
- Auto-merge lands owner updates once checks pass, so an author maintains their own content
  without a human in the loop.
- A new directory can never be auto-approved, which is also when a first-time contributor's
  work should get eyes on it.
- Workflows, `owners.toml` and `index.json` are locked by a ruleset with maintainers as the
  only bypass.

The validator is this project's own libraries, published as a lint binary. The thing that
checks the repository and the thing that installs from it share one implementation, so they
cannot disagree about what a valid document is.

### 7.5 Distribution

CI regenerates `index.json` on merge and publishes it as a release asset. The app fetches
that one URL and then individual documents by path: plain HTTPS, no API, no token, no
account, no git client. Each row carries a hash and size, so a download is verified against
a reviewed index rather than trusted for coming from the right domain.

The index carries **two kinds of revision, which must not stand in for one another**: one
for the whole file, naming the commit it was generated from, and one per item, naming the
change that published that item. The per-item one is what the app shows before installing,
because it is the review a user is being asked to trust; the file-wide one traces only the
index. On merge both are read out of the checkout - `HEAD` for the file, and for each item
`git log -1 --first-parent` over its directory. **`--first-parent` is what makes that the
merging commit**: without it git answers with the contributor's own commit from inside the
branch, which nobody reviewed and which is not on the published history at all. A squash
merge gives the same answer either way.

This is also why an index generated in a pull request cannot fill the per-item field: the
commit that merges a contribution does not exist while it is still a pull request. The
field is left empty rather than guessed at, and the lint's `index` command asks git only
when told to, so the command stays a projection of the tree everywhere else.

### 7.6 What this adds to the app

- A second top-level view for browsing, which the current single-view design does not have.
- Provenance on each registration: local file, or Orng Catalog item at a version. Recorded
  in the entry list (5.2) as one value and not as a pair of optional fields, so that "a
  catalog item at no version" and "a local file at version 2.0.1" cannot be written down.
- An `Update available` status, distinct from a locally modified file - different cause,
  different remedy. Comparing digests only ever says that a file **differs**; it takes the
  recorded source to say why. A local file has nothing upstream, so a difference is the
  user's own edit. A catalog item carries the version installed, so the same difference can
  be read against the published one and reported as an update.
- Author, version and the merged commit visible before installing. A document is a DSP
  graph that Bitwig executes; review is the only trust boundary and users should see who
  signed off.

---

## 8. State of the work

Built and tested against a real installation:

- All six library crates, with the anchors resolving correctly and 428 factory entries read
  back whole.
- Document reading for all three serializations, with identity rewriting verified byte-exact
  and reversible.
- The tamper guard: inspect, disarm, idempotent.
- Registrations, the entry list format, description bundles, document placement and linking.
- The prepare transaction, end to end against a copy of the installed Bitwig: it backs up,
  patches, verifies under the bundled JVM, activates and links. Preparing twice is proved
  to produce the same archive, restore is proved to give back the original byte for byte,
  and a patch broken in a way only a JVM can catch is proved not to reach the installation.
- The injected class and both calls to it. A prepared archive is proved to read the entry
  list and register every row through Bitwig's own registration method, under Bitwig's own
  JVM, and to report rather than throw when the list cannot be applied.

- The catalog's rules and its validator, including the ownership check, and both schema
  changes the design was promised: the per-item merging commit in the index (7.5), and
  version and source in the entry list (5.2, 7.6). The entry list's format marker is now
  checked on read rather than only written, and a list written before the catalog existed
  still loads, as local content.

Not built yet:

- The application.
- The catalog repository's own continuous integration.

---

## 9. Position

This is interoperability work. It teaches a Bitwig installation to trust content its owner
made, and places that content where the installation looks for it.

The line it does not cross: nothing here unlocks functionality the user has not paid for.
The entitlement work adds specific identities belonging to specific files the user already
has, and deliberately leaves untouched the flag that would grant Bitwig's own device set on
a restricted edition. That flag is one boolean away at all times and stays that way.

Preparing an installation modifies a local copy of software the user owns. It is backed up
first, verified before activation, and reversible. Bitwig's published terms restrict
modifying and redistributing its products; users are responsible for their own licence and
local law. Nothing produced by this project may ship Bitwig code or assets, and a modified
archive or a backup must never be redistributed.

The workspace is licensed per crate. The crates that only read documents or
locate an installation carry no copyleft dependency and are permissive, so they
are useful to anyone; the crates that edit bytecode link Krakatau and are
GPL-3.0 because they must be. `LICENSING.md` records which is which.

Not affiliated with or endorsed by Bitwig GmbH.
