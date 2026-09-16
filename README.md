# Orange Tools

Register your own devices, modulators and Grid modules with [Bitwig Studio](https://www.bitwig.com),
so that projects recall them reliably and they behave like Bitwig's own content.

Bitwig only fully trusts content it knows by identity. A `.bwdevice`, `.bwmodulator` or
`.bwmodule` you made yourself has a UUID, but the installation has never heard of it. This
project teaches an installation that identity, puts the document where the installation
looks for it, and gives it a description and search keywords so it is findable in the
browser.

Not produced by or affiliated with Bitwig GmbH.

## Status

Early. The libraries work and are tested against a real installation; the application does
not exist yet. See [`docs/project-spec.md`](docs/project-spec.md) for what this is and how
it works, and the end of it for what is and is not built.

## Layout

```
crates/
  bitwig-install     locating an installation, the user library, the bundled JVM
  bitwig-document    reading and re-identifying Bitwig documents
  bitwig-classfile   class-file and archive surgery; knows nothing of Bitwig
  bitwig-registry    locating Bitwig's internals structurally; reading its registry
  orange-catalog     the community repository format and the rules that validate it
  orange-tools       the facade: registrations, entry list, placement, descriptions
apps/
  orange-registry    the application
docs/                specifications and design
```

## Building

Rust stable, 2024 edition.

```bash
cargo build --workspace
cargo test --workspace
```

Tests that need a Bitwig installation find it automatically and skip when there is none.
Tests that need sample documents skip unless you point them at some:

```bash
ORANGE_TEST_DOCUMENTS=/path/to/your/devices cargo test --workspace
```

No Bitwig content is redistributed here, and none ever will be.

## Documents

| | |
| --- | --- |
| [`docs/project-spec.md`](docs/project-spec.md) | What this is, how registration works, the decisions and why |
| [`docs/ui-spec.md`](docs/ui-spec.md) | What the application's interface must do |
| [`docs/ui-spec-catalog.md`](docs/ui-spec-catalog.md) | The community catalog, as a UI feature |
| [`docs/design-review.md`](docs/design-review.md) | Validation of the visual design against both |
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

- `bitwig-install`, `bitwig-document`, `orange-catalog`: **MIT OR Apache-2.0**
- `bitwig-classfile`, `bitwig-registry`, `orange-tools`, `orange-registry`:
  **GPL-3.0-only**, because they link [Krakatau](https://github.com/Storyyeller/Krakatau)

So reading Bitwig documents or working with the catalog format needs nothing
copyleft; editing bytecode does. See [LICENSING.md](LICENSING.md).
