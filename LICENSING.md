# Licensing

This workspace is licensed per crate, because one dependency forces copyleft on
part of it and there is no reason to spread that to the rest.

| Crate | Licence | Why |
| --- | --- | --- |
| `bitwig-install` | MIT OR Apache-2.0 | No copyleft dependencies |
| `bitwig-document` | MIT OR Apache-2.0 | No copyleft dependencies |
| `orange-catalog` | MIT OR Apache-2.0 | No copyleft dependencies |
| `bitwig-classfile` | GPL-3.0-only | Links [Krakatau](https://github.com/Storyyeller/Krakatau), which is GPL-3.0 |
| `bitwig-registry` | GPL-3.0-only | Links `bitwig-classfile` |
| `orange-tools` | GPL-3.0-only | Links `bitwig-classfile` |
| `orange-registry` | GPL-3.0-only | Links `bitwig-classfile` |

## What this means for you

**Reading Bitwig documents, locating an installation, or working with the
catalog format** needs only the permissive crates. Use them in anything, under
either MIT or Apache-2.0, at your choice.

**Editing bytecode or preparing an installation** goes through
`bitwig-classfile`, which statically links Krakatau. Rust links statically, so
anything you distribute that depends on it is a combined work and must be
GPL-3.0.

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

Content published to Orange Catalog carries its own licence, declared per item
in `orange.toml`. An author's choice for their device is independent of the
licence on this code.

Nothing in this repository is licensed to you by Bitwig GmbH, and nothing here
redistributes Bitwig code or assets.
