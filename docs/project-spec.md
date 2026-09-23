# Orng Tools - project specification

The stable reference for what this project is, how it works and why it is built the way it
is. Where this document and another disagree, this one is wrong and should be fixed; it is
meant to be the thing that keeps the vision from drifting, not a record of a past plan.

Companion documents: `docs/design_handoff_orng_registry/` (the design bundle, and **the
authority for what the interface looks like**), `interface-notes.md` (how to check the window
against it, and the traps that have cost a session each), `design-review.md` (what was checked
against it and what came back), `AGENTS.md` (how code in this repo is written).

`ui-spec.md` is no longer followed. It was written before there was a design and its
structural sketch is a hint that was taken for a layout - the install bar grew a second line
of badge, guard and backup on the strength of it, when the bundle routes the guard and the
backup to Settings under Diagnostics and draws one line. Where the two disagree the bundle
wins, and anything the bundle does not draw is asked about rather than invented.

---

## 1. What this is

Bitwig Studio only fully trusts content it knows by identity. A `.bwdevice`,
`.bwmodulator` or `.bwmodule` that a user made themselves has a UUID, but the installation
has never heard of it, so projects cannot recall it reliably.

**Orng Tools** is a Rust workspace that closes that gap:

- a set of libraries for reading Bitwig documents and preparing a Bitwig installation to
  accept custom identities, publishable so other people can build on them;
- **ORNG Registry**, an application that lets a user register their own content with one
  drag and one button;
- **ORNG Catalog**, a curated public repository of community content that ORNG Registry
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
- Fetching, installing and updating content from ORNG Catalog.
- Undoing all of the above.

Out of scope, and intended to stay that way:

- Editing device contents: panels, DSP graphs, Nitro code. That is `bwmodule-parser`'s
  territory and a different product.
- The donor-UUID method, which reassigns a native device's identity to custom content. It
  sacrifices a stock device per custom one and makes projects non-portable between users
  with different donor mappings.
- Anything that unlocks Bitwig functionality the user has not paid for. See section 9.
- Accounts, telemetry, update checks, or any network traffic beyond ORNG Catalog.

---

## 3. Vocabulary

Several things here are near-synonyms in ordinary speech. Each word below has exactly one
meaning in this project, and "catalog" means only the public repository.

| Term | Meaning |
| --- | --- |
| **Core Registry** | Bitwig's internal list of every identity it treats as native. What preparation teaches to read our entries. |
| **Bitwig library** | `Library/` inside the installation: `devices/`, `modulators/`, `modules/`. Where registered paths resolve. |
| **User library** | The user's own content folder, under `Documents` or `$HOME`. Survives Bitwig updates. |
| **ORNG Catalog** | The public repository of community content. The only thing this project calls a catalog. |
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
| Encrypted binary | Ramona binary behind a stream cipher. Factory content. Needs the installation's section key - see below. |

All three are read. A new UUID is applied by splicing, not re-serializing: the identity is
fixed width in every encoding, so the surrounding bytes are untouched and the operation is
exactly reversible.

**The section key is read out of the installation, never carried here.** The encrypted
form is Bitwig's own material, and so is the key that opens it, so a copy of it in this
source tree would be a copy of Bitwig's material in this source tree. `bitwig-registry`
reads it from the jar of the installation in front of it, every time.

It is not stored there as a run of bytes. The key is a `byte[]` literal built by bytecode -
`newarray byte`, then one `bastore` per element - so its bytes sit one every four,
interleaved with opcodes, and searching a jar for the key as a contiguous string finds
nothing in any encoding. Resolution follows the same rule as every other anchor: the class
holding it is obfuscated and moves between releases, so what is named is the unobfuscated
package, and what settles the answer is neither a name nor a length but a decryption - a
candidate is accepted only once it has opened a real document and the result has parsed.
It fails closed, and no error it raises carries the material.

**It is resolved once per installation and never cached to disk.** Extraction costs about
80ms, roughly half of which is the archive's central directory, so the shape that matters
is the API rather than a cache: every read takes the key as an argument, which makes
"resolve once, pass it down" the natural way to write a caller and a per-document
re-extraction the unnatural one. A key written to `~/.orng` would be Bitwig's material in
a file this project wrote - the thing removing it from the source tree was for, relocated
to every user's disk - and it would go stale the next time Bitwig updates. It is held for
as long as the resolved installation is held, and dropped with it, which is the same rule
`session.rs` already follows for everything else the machine reports.

`bitwig-document` needs no installation, which is why it takes the key as an argument and
refuses the encrypted form without one. The catalog never supplies one: everything it
carries is text or plain binary, and an encrypted document in a pull request is Bitwig's
factory content being redistributed, which it refuses by name. Tests that need the real key
take it from `ORNG_SECTION_KEY`.

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
| `orng-tools` | The facade: registrations, the entry list, description bundles, document placement, and the two operations that write - preparing an installation, and updating its entries. |
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
| 7 | The digest of the document as it was placed, which is what `Changed` is read against |
| 8, 9, 10 | Version, the catalog commit the item was reviewed in, and source (7.6), which only the app reads |

That asymmetry is why the list can grow: an installation prepared before a column was added
keeps working against a longer list, so a format change costs a parser change here and not
a re-preparation. A marker line states the format, and a reader that does not know the
number refuses the file rather than reading it under a guessed layout. Source is the last
column and never empty, because the column that goes missing is the empty one an editor
stripping trailing whitespace would eat - which is also why the digest sits before the
provenance group rather than after it: it is empty for every row registered by a build that
did not record one, and nothing can fill it in afterwards, because hashing whatever is on
disk now would write the present down as the past. The review commit sits between the
version and the source for the same reason: it is empty for every local file, and for any
catalog item whose index could not name the change that published it.

**The review commit is recorded at install and never looked up.** The catalog goes on
publishing, so an item installed today and superseded next month has nowhere left for the
question to be asked - the index that described this copy is gone. What the catalog's detail
panel draws is a different fact, about the item as the catalog currently publishes it; what
the inspector draws is about this machine's copy of it, which is why one is read off the
index and the other off the entry list.

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

**6.13 The window declares its colour space on macOS.** A `CAMetalLayer` whose
`colorspace` is nil is not unmanaged-but-harmless: nil means *these values are
already in the display's own space*, so the window server hands them to the panel
untouched. wgpu sets it to nil in the belief that the layer's default is sRGB,
and the default is nil - so on a wide-gamut display every colour the design
specifies is stretched across a gamut it was not authored for, and the interface
reads oversaturated. The layer is tagged sRGB, which is what the palette is
transcribed from, and the tag is re-applied every frame because the layer is
rebuilt when the window moves between displays. Confirmed by reading the layer
back: it is nil before, sRGB after, and nil again after a rebuild.

**6.14 A document is never written over unless its identity says it is the same
content.** Under the linking strategy a document is placed in the folder Bitwig's
own "Save device..." writes into, and a registered library path is derived from a
file name, so what is already at the target is as likely to be the user's own
work as an older copy of what is being registered. A matching UUID is what
licenses a replacement; a different one, or a file that cannot be read as a
document at all, is refused by name. The same check answers before the write, so
the interface states the collision on the row rather than after a press.

**6.5 Identity is never reassigned implicitly.** A UUID is how a saved project finds its
device. Reassigning one already in use silently breaks every project referencing it, so it
is offered only before an identity has been registered.

**6.6 Structural resolution over pinned builds.** Section 5.3.

**6.7 The document is the source of truth for identity.** Anything derivable from a
document is read from it rather than restated alongside it. Applies to the entry list and
to ORNG Catalog manifests alike.

**6.8 The backup is the patch source, not the installed file.** A backup is taken once per
build and never overwritten, and every preparation of that build patches it rather than
whatever is installed now. Preparing twice then yields the same archive byte for byte
instead of stacking a second copy of every edit, and restore always has an unmodified
original. An installation that is already modified with no backup to work from is refused,
because there is then nothing pristine to patch.

**6.9 `ORNG` in user-facing text, `orng` in machine identifiers.** The product names are
**ORNG Registry** and **ORNG Catalog**; the binary, the paths, the packages, the domain and
the repositories are lowercase `orng`. Read as an abbreviation rather than a word, which is
what the uppercase is for. It does not stand for anything yet.

Splitting on that line rather than picking one casing keeps a heading from having to look
like a command and a path from having to shout. It also supersedes the round-two letter,
which said the product name became "ORNG Registry"; the designer was told in round three.

**6.10 Three platforms, and the differences live in two places.** ORNG Registry targets
macOS, Windows and Linux. Everything platform-shaped is already confined to
`bitwig-install`, which knows where an installation, a user library and a settings
directory sit on each, and to one function in `orng-tools` that links a folder. Nothing
above those two knows what it is running on, and nothing new should.

Continuous integration builds and tests all three, because the two-thirds of that code
nobody exercises locally is exactly the two-thirds that rots.

**6.11 Windows links with a junction, not a symbolic link.** A symbolic link there needs
`SeCreateSymbolicLinkPrivilege`, which an ordinary account does not hold unless Developer
Mode is on, so linking the library folders would fail for most of the people it is for. A
junction needs no privilege, is resolved below the application, and is indistinguishable to
Bitwig. It cannot point at a network share or live on a volume without reparse points; both
fail loudly, and the answer to either is the `Copy` strategy, which makes no links at all.

Two consequences that are easy to get wrong and were: a junction is a directory, so removing
one is not the call that removes a file; and it stores its target in a form Windows
normalises, so an existing link is recognised by resolving it rather than by comparing the
text it reads back.

**6.12 The index signature is detached, and the key is pinned in the app.** An embedded
signature has to be excluded from what it covers, so what is signed becomes a
canonicalisation of the file rather than the file, and a canonicalisation is a second
serializer for two programs to agree about. A second asset keeps the bytes signed, the bytes
served and the bytes parsed identical, with nothing to normalise, and leaves `index.json`
untouched for any reader that has not learned about signatures.

The public key is compiled into the application rather than fetched. A key downloaded beside
the index would be chosen by whoever serves the index, which is the party the signature
exists to distrust. The cost is that rotation needs an application release, which is the
right price: a scheme where the key can be replaced remotely is a scheme where it can be
replaced by the wrong person.

---

## 7. ORNG Catalog

A public repository of community content that ORNG Registry installs from, so users get a
curated view of what exists instead of hunting for downloads.

### 7.1 What it actually is

Not a file host. Content is 20 to 30 KB per item; the entire Bitwig factory device set is
4.7 MB. Hosting is a non-problem.

ORNG Catalog is an **identity authority**. Its job is to guarantee that a UUID means one
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

**The index is signed**, and published with its signature as a second asset,
`index.json.sig`. Every digest in the index is only as good as the index, so whoever serves
it decides what gets installed; a mirror at `orng.tools` would otherwise be a second party
able to do that. The app carries the public key and refuses an index that does not verify
against it, which leaves a mirror able to be stale but not able to be wrong. Decision 6.12
records why the signature is detached rather than embedded. `docs/index-signing.md` is the
note the catalog's own workflow is written from.

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
- Provenance on each registration: local file, or ORNG Catalog item at a version. Recorded
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
  still loads, as local content. The list has since gained two more columns - the digest of
  the document as placed, and the catalog commit an item was reviewed in - and a list
  written before either of them loads too, saying nothing about its documents or about
  which change published them, which is the truth about it.

- Index signing, both halves: the lint signs with a key it will only take from the
  environment and verifies against a public one, and the library offers the single call the
  application makes, which parses a downloaded index only once the signature over those
  exact bytes is proved. A tampered index, a signature from another key and malformed key
  material are each covered by a test.

Verified on Windows on ARM64, against Bitwig Studio 6.1: the whole suite, including the
anchors, the factory registry, the tamper guard, and the prepare transaction end to end
under Bitwig's own bundled JVM. Linking was checked against a token that genuinely lacks
`SeCreateSymbolicLinkPrivilege`, because an elevated one cannot reproduce the failure a
junction exists to avoid, and a build agent is elevated.

Known gaps:

- Nothing has run on Linux yet, including the settings directory path, which is the one
  Windows got wrong.
- `AppData::config_json` and `AppData::lock_file` name files nothing reads. Run detection
  scans processes instead, so the lock is not used and neither name has been checked against
  a Windows or Linux installation. Whichever the application ends up needing should be
  verified against one before it is trusted.
- Nothing has verified the consent dialog itself. The Windows machine available is reached
  over SSH and its session is already elevated, so `runas` there succeeds without raising
  one. What that proves is the child, the channel and the wait; what it does not is what a
  user sees when the dialog appears, or what happens when they dismiss it. The declined
  path is written against `ERROR_CANCELLED` and is unexercised.

The catalog repository's own continuous integration, which checks a pull request, decides
whether it may auto-merge, and on merge regenerates the index, signs it and publishes both
assets. The published index verifies against the published key.

Updating entries as one operation rather than three loose calls: a change to the list
carries the documents it names, places them, rewrites all three description bundles from
the whole list, and writes the list last - so a failure part way leaves files nothing
points at rather than entries pointing at nothing. Every write in it is idempotent, which
is what makes recovering from one a second press rather than a repair.

The application, as far as: reading what the machine has, listing what is registered,
registering documents the user drops on it, preparing an installation, and reading the
catalog and installing from it. It draws in the design's palette and typefaces, and every state it can be in
renders headlessly into `apps/orng-registry/tests/snapshots` so a change to the interface
can be looked at rather than reasoned about.

The whole window is a drop target; a dropped folder is read one level deep; each file
resolves off the drawing thread to `Staged`, `Conflict` or `Rejected` with the reason on
the row; and `Add files...` is there because drag and drop may not be the only way in. One
press runs both modes when both are pending, which is also what puts the description
bundles back after a Bitwig update has replaced them.

The design bundle's own components, at its own measurements: the install bar naming the
installation, its build, its path and its registry badge; the list toolbar with search and
the kind filters; the list, with sections dividing pending work from what is registered;
and the action bar carrying the summary, the mode it will run, and the one primary action.
A row is a grid, so identities and statuses line up down the list. The icons are the
design's own set, Phosphor, in the Light weight it names. Every empty state is the
designer's copy verbatim rather than something invented beside it.

A row's own controls, at the right end of it, hidden off hover rather than removed so the
columns before them do not move when the pointer arrives. Which of the five a row offers
is a table the design states per status, and that table is written once - in
`apps/orng-registry/src/status.rs` - for the two surfaces that draw it. Getting it wrong
is not cosmetic: during design the inspector offered `Reveal file` on a missing file,
which is the one action that cannot work, while omitting `Locate file`, which is the one
that fixes it.

**A registered row's status is computed, not assumed.** Four of the nine drawn here are
answers about the machine rather than about the press that was just made, and all four are
resolved when the list is read rather than per row per frame - the same shape every screen
here uses, and for the same reason: this audience keeps libraries on external and network
volumes, where one `stat` against a spun-down mount is tens of milliseconds of a frame.
`Missing file` is the registered path failing to resolve. `Changed` is the document there
hashing to something other than what was recorded when it was placed (5.2), which is the
one statement that separates a file somebody else rewrote from one that is simply gone -
same remedy on the surface, different cause, and the cause is what the user needs in order
to act. `Update available` is the recorded provenance held against the published index, by
identity and never by name, and only once an index has been fetched: a window nobody has
opened the catalog in must not answer "up to date" any more than "out of date". `Pending
restart` is what a run wrote into a live installation, until the list is read off the
machine again - Bitwig reads the entry list when it launches, so a row written while it is
open is one it is not showing, and nothing here watches for it being restarted.

One row says one thing, and which one is the design's own colours in order: broken first,
then a decision waiting, then work in flight with nothing to decide. A row that is both
changed and updatable states the change, because updating it would discard exactly that.

**A removal is queued, not done.** The entry stays registered, struck through, and the
row that queued it offers only the undo, until the press of the primary action that
carries it out - so one press is the confirmation for every removal in the list, and
"2 to add, 1 to remove" in the action bar is what the design always assumed. Whether the
document goes with the entry is the preference in Settings, read for the first time here,
and the control that queues the removal names which way it is set rather than warning in
the abstract: the design made that wording the confirmation instead of adding a dialog.
The delete runs after the entry list is written, which is the mirror of the reason the
documents are placed first - deleting before would leave, for as long as the write takes
and for ever if it fails, an entry registered with nothing behind it.

`Assign new UUID` mints an identity for a staged document and reads the whole pending set
again, because a conflict is a statement about the set and settling one row settles the
row it collided with. `Locate file` points a registered entry back at a document and
keeps the *recorded* registration: the entry still exists and its words may have been
edited since, so deriving them again from the file found would quietly undo that. It is
not a re-drop. A file carrying another identity is refused by name.

**The bar's note answers the filter once the filter has emptied the list.** The line under
the summary normally says what the press would cost - which mode it runs in, and whether a
backup is written first. A search or a kind chip that leaves nothing on screen takes it
instead and states how many entries are behind the filter: the empty state offers the way
out but states no number, and how much of the list is still there is what says whether
clearing it is worth doing. The count is of the rows the list would have drawn, so a
document dropped over an entry already registered is one row and not two, and a dropped
file that is not a document is not counted at all - it carries no entry for a filter over
names and identities to hide it by. The sentence is the design's
(`ORNG Registry.dc.html:490`); the arithmetic under it and the rule about when the line
changes hands are ours, and are in `design-review.md` round 3.

A condition that stands in the way of the press is a banner directly above the action bar,
where the design puts it: the tone as a dot, the reason under the headline, and the one
thing that can be done about it at the right end. A preparation in flight is the design's
dialog over the window, holding it still - the scrim takes the pointer as well as the
light - and what a press came to is a banner in the same place afterwards, dismissible,
carrying `Copy details` when it failed.

**Preparing confirms, and nothing else does.** The press that modifies the installation
puts the design's plan over the window first - what is backed up and where, the archive
written beside the original and moved into place once it verifies, which rows are
registered and which forgotten, where the documents go, and the links - and runs on the
dialog's own press or is put away by its `Cancel`. The lines are worked out as the dialog
is drawn, from the same rows the press will write, so a drop that lands under the scrim is
described rather than missed; only what the disk has to be asked is read once, when the
press is made. The dialog holds the window still in both senses, a scrim that takes every
click and egui's modal layer that keeps the keyboard out of the bars under it, and the
progress dialog shares the skeleton. The words are the bundle's, with four exceptions the
data forced; `design-review.md` round 3 item 8 records them.

**Rights, which on Windows are the ordinary case.** An installation under `Program Files`
is not writable by an ordinary account, and both modes reach inside it: preparing writes
the archive, and every entry change rewrites the description bundles in the installation's
`localization` directory (4.4). So the interface's old claim that the cheap mode touches no
part of the installation was wrong, and it was wrong four ways - an inspector edit, a
locate, a catalog install and a removal under the copy strategy all arrive there.

Whether it can be written is answered **by writing**: a file created and removed in each of
the three directories a modification touches. Permissions are not one model across three
platforms and cannot be read as one, and on Windows `Permissions::readonly` reports the DOS
attribute rather than the ACL that actually decides. It is asked once when the installation
is read, beside whether Bitwig is running, because both are conditions on a press.

**A running process cannot gain rights**, so a press that needs them is not done but
*described*, and the description is carried to a second copy of this application that
Windows starts holding them - `ShellExecuteEx` with the `runas` verb, whose consent dialog
is the system's own. What crosses is a recipe and not a built transaction: the child replays
the same `Update::add` and `Update::revise` calls the window would have made, so their
assertions run on both sides of the process boundary. Rows cross in the entry list's own
format rather than in a second per-row representation, and documents cross as bytes rather
than as paths, because a path would be a file the elevated child reads on the say-so of
something unelevated. They follow the recipe's one line of JSON rather than sitting inside
it, each as many bytes as the line says it is. So every read has a bound: a line is at most
16 MiB, the size of a very long entry list, in both directions, and a document at most
64 MiB, refused on its stated length before a byte of it is read - and by the window before
the child is started, so a document too large to cross never costs a consent dialog.

**The installation root travels on the command line and never in the recipe.** Preparation
runs the installation's own bundled JVM to verify its patch, so a child that took its root
from the message would run `java.exe` from wherever that message pointed, elevated. The
pipe is named with sixteen random bytes and created with `FILE_FLAG_FIRST_PIPE_INSTANCE`,
so squatting the name is a guess rather than a race and a name already taken stops the
launch rather than redirecting it. A restore crosses the same way and names a directory,
which the child holds against the copies it can actually see rather than restoring what it
was handed.

The window waits on the connection and on the child's own handle together, so a child that
dies before it calls back ends the run instead of hanging it, and a child that closes
without saying how it went is a failure rather than a success - the rule the in-process
worker already followed. Declining the dialog is neither: `ERROR_CANCELLED` is reported as
rights declined and nothing changed.

Where the platform has no way to ask - macOS and Linux, where an installation is the user's
own anyway - the press is refused before it starts, with a banner naming the directory that
refused and offering no control, because the remedy is the permissions or the account and
neither is a press.

**The index is kept, and checked on every launch.** What is written down is the bytes
that arrived and the signature over them, in `~/.orng/catalog/`, never an index this
application re-serialised: reading the pair back is the same verification the download
went through, so a file edited under the user's own home is refused on exactly the terms
a tampered download is. `~/.orng` is an ordinary directory writable by anything running
as the user, and an index decides what gets downloaded into a DAW and what digest it is
held against, so it is the one place a cache must not be believed just because it is
local.

Two acts on opening and not one: the pair is read, which is two files and a signature, so
the first frame has a catalog with no network at all; and a fetch starts behind it on a
worker, so what the first frame has is not silently last month's. The window never waits
for the second. Failing it is a degraded state and not an error, which is what the design
insists on: the list still browses, the bar states the age of what is on screen and says
`cached`, and the age turns accent-coloured and grows a labelled `Refresh` once it is old
enough - a week, which is ours and is recorded in `design-review.md` round 3 item 6. Only
a machine that has never fetched one at all gets the empty state and `Catalog
unavailable`, and that one offers `Try again`. The age is the kept file's own write time,
and every successful fetch rewrites both files whether the bytes changed or not, so it
states when the catalog was last confirmed rather than when it last said something new.

The catalog's detail panel, which a catalog row opens: who wrote the item, what it is
for, whether this installation is new enough to load it, what it is licensed under, its
keywords, the change that published it, its author's page and its identity. Two of those
are worked out rather than read: compatibility, from the build beside the item's declared
minimum, and whether another published item has taken this one's place - which is a fact
about the whole index and is drawn as the design's notice, with the control that walks
the panel over to the replacement.

**Installing from the catalog**, which is what the panel and the row's own control finally
do. A document is fetched over plain HTTPS at the commit the index names itself built
from - never at a branch, because a branch moves and bytes fetched from one are bytes the
verified index never described. What arrives is held against the row three ways, all of
them against a signed index and none of them needing a socket: the length it states, the
digest it states, and whether it reads as the document it claims to be. Anything else is
refused before it reaches the library, and the refusal is **two states and not one**. A
download that did not arrive is ordinary and is offered again. A file that is not the one
the catalog describes is a trust event - the review is the only thing standing between a
stranger's DSP and somebody's projects, and the digest is how that review reaches this
machine - so it is held on the row, said out loud in a banner that promises nothing was
written, and offered `Copy details` and deliberately never `Retry`. Asking again gets the
same bytes.

What is then registered is derived from the **document** and not from the index row - the
same derivation a drop uses, because the description and the search keywords Bitwig will
show live in the document's own identity and the index only copied them out of there. What
the index adds is the two things the document cannot know: which publication this is, and
which change published it.

**Installing is entries work and asks nothing.** An item is 20 to 30 kilobytes and a new
identity cannot affect a project that already exists, so there is no backup, no
confirmation, and no requirement that Bitwig be closed - it takes effect at the next
launch, like every other entry change. Preparing the installation stays the other press,
and the action bar goes on offering it where it is needed.

**A catalog row's state is a fact about this machine**, resolved in one pass against the
list and the index before the list is drawn: nothing in a signed index knows what is
registered here, and nothing registered here knows what has been published since. A failed
attempt outranks everything, because it is the only one of the seven states that is about a
press the user just made. After that the question is whether the item is here at all - an
installed item this Bitwig is too old for is still installed, and telling somebody they
need a newer Bitwig for something already in their browser is telling them nothing they can
act on. `Replacement available` therefore belongs to an item that is installed; for one
nobody has, a replacement existing changes nothing, because the offer is to whoever already
owns the old one.

**`Update available` states the fact and offers no press, and that is deliberate.** The
design confirms an update through a modal that names the item and both versions, because
Bitwig resolves a device by identity and replacing the file changes every project that
already uses it. Nothing in the bundle draws that modal. A press that overwrote a device
under every open project rather than asking is not a smaller version of the design, so the
row wears the word in the accent and stops there - the same shape the progress dialog's
missing `Cancel` takes. The guarantee is kept by the types rather than by intent: the only
press that writes a document is offered by `Available`, which means the identity is not
registered here.

**The catalog's own toolbar**, which is the Local one's shape with the box at its right
end exchanged. `Add files...` and the design's factory toggle are about documents on this
machine; what somebody browsing needs instead is `All / Installed / Updatable`, and the
design makes it a three-state switch rather than a checkbox because `Updatable` is the
state a returning user comes back for. The search is the other difference and it is a
difference of question: Local matches a name and an identity - *what is this thing I
have* - and the catalog matches the name, the author, the description and the keywords,
because *is there a thing that does X* is answered in the description. The field says so,
and is capped a hundred pixels wider to have room to. There is no identity among the
fields it searches, for the same reason a catalog row does not draw one.

The kind facets count what the install filter left and not what the list is showing: a
facet answers "how many would I see if I switched this kind on", so narrowing it by the
control it belongs to would make every count read the number already on screen. The query
and the kinds are one setting across both views and the install filter is the catalog's
alone, which is the bundle's own arrangement - switching views keeps what was asked for
rather than quietly widening it. Narrowed to nothing, the region says so in the design's
own sentence and offers the press that undoes it, with the toolbar still above it: the
control that clears a filter must not go away with the rows it hid.

**The action bar under the catalog counts the catalog.** It used to run one arithmetic
whatever was on screen - staged rows, queued removals, which mode the press would use -
so browsing read `Nothing pending` over a list of things to install, which is a true
sentence about the other list. Nothing in the catalog is staged and nothing is queued: an
install is one press on one row, so what the design puts there instead is how big the
catalog is and what a press would cost. The count follows the install filter and nothing
else, which is the same arithmetic the kind facets run on and for the same reason - the
search and the chips move the note and never the number, so the summary says how big the
catalog is and the note says what the filters did to it. Only the word changes with the
filter, because the filter is what names what is being counted.

Two states replace the count rather than qualifying it, and are separated by tone. An
index that never arrived and has nothing kept behind it is `Catalog unavailable` in the
accent - a refresh that failed over an index already in hand only qualifies the count,
adding `cached` after it and saying under it that installing a kept item still works; an
install that was refused is `Install refused` in the error colour, one line for both
kinds of refusal, because what the bar has to say is that nothing was written and the
rows say which was which. A refusal outranks the count for the reason a failed attempt
outranks the other six row states: it is the only thing there about a press the user just
made. Two of the words are not the bundle's - `Fetching <item>`, because the bundle draws
no state at all for a fetch in flight and the press was otherwise followed by a second of
silence, and the count's wording under the `Installed` filter, which the bundle captions
no scenario for.

The inspector, which a row opens and which slides over the right of the list: what the
entry is called, what Bitwig's browser says under it, the words that find it, its
identity, where the registry points, where the document came from and whether it is
actually there. The description and the search keywords are editable, and that is what
the panel is for - they are what makes a registered device feel native in the browser
(4.4), and until this existed the only way to change either was to edit the entry list
by hand. An edit is written when a field is finished with, through the same worker a
press of the primary action uses, because it is the same operation: the three bundles
rewritten from the whole list, then the list. Nothing is said when one lands and a
failure is said either way. Its action list is the row's list, gated on the same status
table, because the design's own rule is that the panel's status rules are the row's and
not a second set - so an entry whose file is missing is offered `Locate file...` here and
never `Reveal file`, and a press from the panel goes through the same call a press on the
row does. The panel takes
272 of the window's 820, so the list beside it draws to a grid of its own and the toolbar
beside it drops its labels and shrinks its field - all three as the design has them.

The Settings screen, which the overflow opens. **A full-window surface and not a panel**:
`width:100%; height:100%` on the page colour with a 44-pixel header of its own, so it
replaces the install bar and the action bar as well as the page. It carries the six groups
the bundle draws - the two paths and the backups directory, the placement strategy, the
delete-file default, the appearance switch and the diagnostics report - and the row at its
foot that leads to About. It is also the first surface that *writes a preference*, which
is new: everything before it read the machine and remembered nothing.

**Preferences live in `~/.orng/settings.toml`, beside the entry list and for the same
reason** - a Bitwig update must not be able to touch them. Five of them: the installation
root, the user library, the placement strategy, the delete-file default and the
appearance. The first two are the awkward pair, because an installation can be moved or
replaced and a remembered root is a claim that may have stopped being true; what is stored
is that the user said where to look, and a stored root that no longer resolves falls back
to discovery. The file is read once on opening and written only when something changes,
and a file that does not parse is reported rather than replaced. The format is TOML rather
than the entry list's tab-separated form because nothing outside this binary reads it: the
entry list is a wire format shared with the class injected into the installation, and this
is not.

The appearance switch is what makes the light palette reachable at all - until Settings
existed it was drawn only by the render tests. `System` is a third value rather than the
absence of a choice: it follows the desktop, and it has to keep following it while the
window stands open.

The diagnostics report is the design's shape with this project's own facts in it, and
`design-review.md` round 2 records why those differ: three of the paths the bundle draws
are wrong, and the block exists to be pasted into a bug report by somebody who will be
believed. One line the bundle draws is missing - `factory`, the count of Bitwig's own
content - because reading it means parsing a class out of the archive, and the window has
no worker that does so and is not getting one: see `design-review.md` round 3 item 3.

The Restore and About screens, which the overflow opens and which Settings also leads to.
Both are full-window surfaces of the same shape as Settings, and Restore is the only one
with a bar at its foot: it is the only screen that ends in a decision. It lists every
pristine copy `~/.orng/backups` holds, newest first, each naming the build it was taken
from, when it was taken and how big it is - and the build comes back out of the copy's own
directory name, which is the only record of it there is, so a directory not named for a
build is not offered as a backup at all. The press puts that copy back over the
installation, which leaves the archive as Bitwig shipped it and the entry list untouched:
the `Needs re-apply` state, said in the banner the window already has. It is refused while
Bitwig is running, for the reason preparation is.

**Neither screen's dates are drawn from the picture.** A backup's moment is turned into
words where the disk is read, in this machine's zone, because an instant has no day until
one is chosen - so the populated Restore screen has no snapshot and is checked through the
accessibility tree instead. The About screen's identity line names the platform and the
machine, so its snapshot is taken with that line supplied rather than read, for the same
reason the render fixtures' paths are relative.

Every measurement above was read out of the bundle rather than judged by eye, and the
row grids, the toolbar's flexible field, the inspector's own chain of gaps and the
Settings screen's column of groups are checked against its numbers in a test. `docs/interface-notes.md` has the method, including how to
make the bundle report its own geometry - and section 4 there, which is the other half:
the claims a picture structurally cannot hold, and what to ask instead.

Every state renders headlessly into `apps/orng-registry/tests/snapshots`, and the fixture
paths are relative so that the pictures are a function of the code rather than of the
machine that drew them.

Not built yet:

- `Browse all`, the alt the design draws beside `Clear filters` on the catalog's no-match
  state. The bundle's own two handlers reset the same two things, so a second control
  there would offer nothing the first does not - see `design-review.md` round 3 item 3.
- The entry row's overflow control, which the bundle draws on every row in every state
  and never says the contents of. A question for the designer rather than work - see
  `design-review.md` round 3 item 3.
- Cancelling a preparation. The design offers it up to the Activate step, on the grounds
  that nothing has changed until then, and nothing here can honour it: the dialog is drawn
  without the control rather than with a dead one, which is a shape the design itself draws
  from Activate onwards.
- The `Show factory entries` toggle, and the factory section the bundle opens under it.
  Not pending: the user was asked and chose not to have it. Reading Bitwig's own 428
  entries means parsing a class out of the archive, about a second, so it needs a worker
  of its own and the installation's section key - and a toggle that stalled the window,
  or one that did nothing, is worse than no toggle. The entry list is nine states rather
  than the design's ten because of it - see `design-review.md` round 3 item 3.
- The update modal.
- `Licences` on the About screen. The plural is the point: what it would open is the
  licences of everything linked into the binary, and nothing generates or bundles them.
  `Copy diagnostics` is drawn alone rather than beside a control that opens nothing - see
  `design-review.md` round 3 item 3.
- The design draws its small icons in Phosphor's `duotone`, which is two overlapping glyphs
  in two colours and has no single-colour font to be drawn from. Light is used throughout
  instead; whether that matters is a question for the designer.

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
