# Orange Tools - project specification

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

**Orange Tools** is a Rust workspace that closes that gap:

- a set of libraries for reading Bitwig documents and preparing a Bitwig installation to
  accept custom identities, publishable so other people can build on them;
- **Orange Registry**, an application that lets a user register their own content with one
  drag and one button;
- **Orange Catalog**, a curated public repository of community content that Orange Registry
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
- Fetching, installing and updating content from Orange Catalog.
- Undoing all of the above.

Out of scope, and intended to stay that way:

- Editing device contents: panels, DSP graphs, Nitro code. That is `bwmodule-parser`'s
  territory and a different product.
- The donor-UUID method, which reassigns a native device's identity to custom content. It
  sacrifices a stock device per custom one and makes projects non-portable between users
  with different donor mappings.
- Anything that unlocks Bitwig functionality the user has not paid for. See section 9.
- Accounts, telemetry, update checks, or any network traffic beyond Orange Catalog.

---

## 3. Vocabulary

Several things here are near-synonyms in ordinary speech. Each word below has exactly one
meaning in this project, and "catalog" means only the public repository.

| Term | Meaning |
| --- | --- |
| **Core Registry** | Bitwig's internal list of every identity it treats as native. What preparation teaches to read our entries. |
| **Bitwig library** | `Library/` inside the installation: `devices/`, `modulators/`, `modules/`. Where registered paths resolve. |
| **User library** | The user's own content folder, under `Documents` or `$HOME`. Survives Bitwig updates. |
| **Orange Catalog** | The public repository of community content. The only thing this project calls a catalog. |
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
| `orange-tools` | The facade: registrations, the entry list, description bundles, document placement, the transaction. |
| `orange-registry` | The application. |

The split is by what each layer knows, not by convenience. `bitwig-classfile` has no Bitwig
concepts in it and is independently useful; `bitwig-document` needs no installation.

### 5.2 A prepared installation

Preparation is a **fixed edit that does not vary with what is registered**:

- a small class injected into the archive,
- one call added to the Core Registry's initialiser,
- one call added to the entitlement constructor,
- the tamper guard disarmed,
- the register method's access widened so a direct call verifies.

At startup the injected class reads the entry list from a file this project owns and
registers each row. Consequences that matter:

- Adding, removing or renaming content rewrites a text file. No archive work, no backup, no
  requirement that Bitwig be closed.
- A Bitwig update costs **one** preparation, not one operation per registered item.
- The archive is never a source of truth for what is registered, so it cannot drift from
  the record and there is no half-applied state to detect.

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

Bitwig ships a JRE (no compiler). Before a prepared archive replaces the installed one, it
is run under that JVM to force class initialisation through the real verifier. If that does
not exit clean, the installation is never touched.

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

**6.3 Preparation links all three kinds.** Otherwise the first modulator added to a
device-only installation would have to create a folder inside the installation - a write
that can demand authorisation, during the mode that promises never to block.

**6.4 Entitlement: grant entries, never the flag.** Section 4.2.

**6.5 Identity is never reassigned implicitly.** A UUID is how a saved project finds its
device. Reassigning one already in use silently breaks every project referencing it, so it
is offered only before an identity has been registered.

**6.6 Structural resolution over pinned builds.** Section 5.3.

**6.7 The document is the source of truth for identity.** Anything derivable from a
document is read from it rather than restated alongside it. Applies to the entry list and
to Orange Catalog manifests alike.

---

## 7. Orange Catalog

A public repository of community content that Orange Registry installs from, so users get a
curated view of what exists instead of hunting for downloads.

### 7.1 What it actually is

Not a file host. Content is 20 to 30 KB per item; the entire Bitwig factory device set is
4.7 MB. Hosting is a non-problem.

Orange Catalog is an **identity authority**. Its job is to guarantee that a UUID means one
thing, permanently, across contributors who do not know each other. Every rule below
follows from that.

### 7.2 Layout

```
orange-catalog/
  devices/
    <author>/
      <slug>/
        <name>.bwdevice        the document
        orange.toml            what the document cannot say
        README.md              optional
  owners.toml                  author -> GitHub account
  index.json                   generated, never hand-edited
  .github/workflows/
```

`orange.toml` carries only what is not already inside the document - version, author,
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

### 7.6 What this adds to the app

- A second top-level view for browsing, which the current single-view design does not have.
- Provenance on each registration: local file, or Orange Catalog item at a version.
- An `Update available` status, distinct from a locally modified file - different cause,
  different remedy.
- Author, version and the merged commit visible before installing. A document is a DSP
  graph that Bitwig executes; review is the only trust boundary and users should see who
  signed off.

---

## 8. State of the work

Built and tested against a real installation:

- All five library crates, with the anchors resolving correctly and 428 factory entries read
  back whole.
- Document reading for all three serializations, with identity rewriting verified byte-exact
  and reversible.
- The tamper guard: inspect, disarm, idempotent.
- Registrations, the entry list format, description bundles, document placement and linking.

Not built yet:

- The injected class and the two calls that reference it.
- The prepare transaction: backup, rewrite, verify under the bundled JVM, activate, roll
  back.
- The application.
- Orange Catalog and its validator.

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
