# Orng Tools

Register your own devices, modulators and Grid modules with [Bitwig Studio](https://www.bitwig.com),
so that projects recall them reliably and they behave like Bitwig's own content.

Bitwig only fully trusts content it knows by identity. A `.bwdevice`, `.bwmodulator` or
`.bwmodule` you made yourself has a UUID, but the installation has never heard of it. This
project teaches an installation that identity, puts the document where the installation
looks for it, and gives it a description and search keywords so it is findable in the
browser.

**ORNG Registry** is the application that does it. Drop documents on its window and it
registers them. It also installs and updates content from
[ORNG Catalog](https://github.com/zezic/orng-catalog), a community repository whose index
is signed and checked before anything from it is believed.

Not produced by or affiliated with Bitwig GmbH.

## Status

Early: 0.1.0 is the first release. Registering, preparing an installation and restoring
it, renaming, and installing and updating from the catalog are all built. The libraries
and the preparation are tested against real Bitwig Studio 6.1 installations on macOS and
Windows, and every state of the window has a test that draws it.

Not yet proven:

- Nothing has run on a real Linux machine. The Linux build compiles and its tests pass in
  CI, but nobody has opened the window there.
- On Windows, the consent dialog that asks for administrator rights has never been seen
  by a person. The elevated half has been tested without the window.

See [`docs/project-spec.md`](docs/project-spec.md) for what this is and how it works;
section 8 says in detail what is and is not built.

## Installing ORNG Registry

Each [release](https://github.com/zezic/orng-tools/releases) carries the application for
all three platforms. None of them is signed by a publisher.

**macOS** 11 or later, Apple silicon and Intel, into `/Applications`:

```bash
curl -fsSL https://github.com/zezic/orng-tools/releases/latest/download/install-macos.sh | sh
```

The application is not notarized, and a copy downloaded through a browser is refused by
Gatekeeper as if it were damaged. The script fetches it with `curl`, which does not mark it
as downloaded, and checks it against the release's `SHA256SUMS`.

**Windows**: download `orng-registry-windows-x86_64.exe` and run it. It is a single file
with nothing to install beside it, and it runs on ARM64 Windows too, under emulation.
SmartScreen asks first: `More info`, then `Run anyway`.

**Linux**, x86_64 with glibc 2.35 or later (Ubuntu 22.04 and newer): download
`orng-registry-linux-x86_64.tar.gz`, unpack it, and run `orng-registry`.

The application keeps its settings, its entry list, the catalog it last fetched and its
backups of prepared installations in `~/.orng`.

## Layout

```
crates/
  bitwig-install      locating an installation, the user library, the bundled JVM
  bitwig-document     reading and re-identifying Bitwig documents
  bitwig-classfile    class-file and archive surgery; knows nothing of Bitwig
  bitwig-registry     locating Bitwig's internals structurally; reading its registry
  orng-catalog        the community repository format and the rules that validate it
  orng-tools          the facade: registrations, entry list, placement, descriptions
apps/
  orng-registry       the application
  orng-catalog-lint   the catalog's validator, index generator and signer
packaging/macos/      the Mac bundle and its installer script
docs/                 specifications and design
```

## Building

Rust stable, 2024 edition.

```bash
cargo build --workspace
cargo test --workspace
```

Tests that need a Bitwig installation look for one and **fail** when there is none, so a
machine that stops testing against Bitwig has to say so. On a machine without Bitwig:

```bash
ORNG_SKIP_BITWIG_TESTS=1 cargo test --workspace
```

Tests that need sample documents skip unless you point them at some:

```bash
ORNG_TEST_DOCUMENTS=/path/to/your/devices cargo test --workspace
```

The application's tests draw every state of the window and compare it with the pictures
in `apps/orng-registry/tests/snapshots`. That needs a GPU or a software renderer.
`ORNG_SKIP_RENDER_SNAPSHOTS=1` skips the comparison, and `UPDATE_SNAPSHOTS=1` records new
pictures.

The Mac bundle is built with `packaging/macos/bundle.sh`. Releases are built by CI when a
version tag is pushed; see `.github/workflows/release.yml`.

No Bitwig content is redistributed here, and none ever will be.

## Documents

| | |
| --- | --- |
| [`docs/project-spec.md`](docs/project-spec.md) | What this is, how registration works, the decisions and why |
| [`docs/design_handoff_orng_registry/`](docs/design_handoff_orng_registry/) | The application's visual design, which the window is built to |
| [`docs/interface-notes.md`](docs/interface-notes.md) | How the window is checked against that design |
| [`docs/design-review.md`](docs/design-review.md) | What was checked against the design, round by round, and what came back |
| [`docs/ui-spec-catalog.md`](docs/ui-spec-catalog.md) | The community catalog, as a UI feature |
| [`docs/ui-spec.md`](docs/ui-spec.md) | The interface requirements written before the design; superseded by it |
| [`docs/index-signing.md`](docs/index-signing.md) | How ORNG Catalog signs its index, and where the key lives |
| [`LICENSING.md`](LICENSING.md) | Which crate is under which licence, and why |

## Scope

This is interoperability work. It teaches an installation to trust content its owner made.

It does not unlock anything you have not paid for. Where Bitwig checks whether a device is
licensed, this adds the specific identities of files you already have, and deliberately
leaves alone the flag that would grant Bitwig's own device set. See section 9 of the
project specification.

Preparing an installation modifies a local copy of software you own. It is backed up
first, verified before it is activated, and reversible. Bitwig's published terms restrict
modifying and redistributing its products; you are responsible for your own licence and
local law. Never redistribute a modified installation or a backup.

## Licence

Per crate, because one dependency forces copyleft on part of the workspace and
there is no reason to spread it further.

- `bitwig-install`, `bitwig-document`, `orng-catalog`, `orng-catalog-lint`:
  **MIT OR Apache-2.0**
- `bitwig-classfile`, `bitwig-registry`, `orng-tools`, `orng-registry`:
  **GPL-3.0-only**, because they link [Krakatau](https://github.com/Storyyeller/Krakatau)

So reading Bitwig documents or working with the catalog format needs nothing
copyleft; editing bytecode does. See [LICENSING.md](LICENSING.md).
