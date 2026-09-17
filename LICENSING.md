# Licensing

This workspace is licensed per crate, because one dependency forces copyleft on
part of it and there is no reason to spread that to the rest.

| Crate | Licence | Why |
| --- | --- | --- |
| `bitwig-install` | MIT OR Apache-2.0 | No copyleft dependencies |
| `bitwig-document` | MIT OR Apache-2.0 | No copyleft dependencies |
| `orng-catalog` | MIT OR Apache-2.0 | No copyleft dependencies |
| `bitwig-classfile` | GPL-3.0-only | Links [Krakatau](https://github.com/Storyyeller/Krakatau), which is GPL-3.0 |
| `bitwig-registry` | GPL-3.0-only | Links `bitwig-classfile` |
| `orng-tools` | GPL-3.0-only | Links `bitwig-classfile` |
| `orng-registry` | GPL-3.0-only | Links `bitwig-classfile` |
| `orng-catalog-lint` | MIT OR Apache-2.0 | Reads documents and the catalog format only |

## What this means for you

**Reading Bitwig documents, locating an installation, or working with the
catalog format** needs only the permissive crates. Use them in anything, under
either MIT or Apache-2.0, at your choice.

**Editing bytecode or preparing an installation** goes through
`bitwig-classfile`, which statically links Krakatau. Rust links statically, so
anything you distribute that depends on it is a combined work and must be
GPL-3.0.

## Why `-only` and not `-or-later`

`GPL-3.0-only` is a deliberate choice, not an inherited default. `-or-later` licenses this
code under terms nobody has read yet, and the move from GPL-2.0 to GPL-3.0 was contentious
enough to show that a future version can change the bargain rather than restate it. A
relicence remains possible; it just has to be a decision rather than an automatic one.

## Why not permissive throughout

Krakatau does the part that is genuinely hard: reassembling a class and
recomputing what an edit invalidates, including stack map frames. Everything
cheap -- constant pool scanning, access flag edits -- is already hand-written
here without it. If `bitwig-classfile` ever grew its own class writer, the whole
workspace could go permissive; until then, that boundary is where the licence
changes.

## How a file states its licence

Every source file carries two SPDX lines, and the crate roots of the copyleft
crates carry the full GNU notice as well:

```rust
// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only
```

The identifier on a file is the licence of the crate it belongs to. Moving a
file between crates changes its licence, so the header moves with it.

## Contributions

A contribution to a permissive crate is taken under MIT OR Apache-2.0, and one
to a copyleft crate under GPL-3.0-only, matching the crate it lands in.

## Not covered here

Content published to ORNG Catalog carries its own licence, declared per item
in `orng.toml`. An author's choice for their device is independent of the
licence on this code.

Nothing in this repository is licensed to you by Bitwig GmbH, and nothing here
redistributes Bitwig code or assets.
