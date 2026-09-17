# ORNG Registry: changes for round four

For the designer. A work list, not a review - the reasoning behind each item is in
`design-review.md` if you want it, but you should not need it to make these changes.

Round three is accepted, and it is a clean round. All five substantive items from the last
letter are answered, and two of them better than they were asked:

- The paths are right in shape. `entries.tsv` with the extension load-bearing,
  `backups/6.1-94a90411/` one directory per build, `Contents/Java/bitwig.jar` in the
  diagnostics block.
- The apply list is the five real steps, and `Your installation was not changed` replaced
  the restore message. Cancel up to Activate now means what it says.
- Provenance keys on UUID (`README.md:806`), with the reasoning written out better than we
  put it.

The problems are all in the rename pass, which is the one part that arrived without a spec
to check against. Four changes and one decision we owe you. None of them touch a design
value; nothing you drew needs redrawing.

---

## 1. The app's root is `~/.orng/`, not `~/.orng-registry/`

`SettingsScreen.dc.html:53,155,156`, `Orange Registry.dc.html:387`,
`README.md:16,82,85,561`.

One systematic substitution, and the only item here that would cost real work if it reached
implementation.

The directory is `~/.orng/`, holding the entry list and the backups:

| | |
| --- | --- |
| Entry list | `~/.orng/entries.tsv` |
| Backups | `~/.orng/backups/<version>-<short revision>/` |

This is not a preference. `.orng` is compiled into the Java class that preparation injects
into `bitwig.jar`, which reads the entry list on every launch from a path it holds as a
literal. A mockup drawn against `~/.orng-registry/` describes a product that cannot be
built without re-preparing every installation that already exists.

The root is also deliberately *not* named for the app. It holds backups of the
**installation**, which are Bitwig's files and not the registry's, so a root called
`orng-registry` would be claiming them. That was in the last letter and is worth keeping:
one root, named for the project, holding both.

## 2. The product name is ORNG Registry

`README.md:16` records the display name as unchanged, still "Orange Registry". That follows
the last letter, which said the product names become "Orng Registry" - so this is us
changing our answer, not you missing it.

The convention, which is new and settles the question for everything downstream:

| | |
| --- | --- |
| **User-facing text** | **`ORNG`**, uppercase. "ORNG Registry", "ORNG Catalog" |
| **Machine identifiers** | `orng`, lowercase. Binary, paths, packages, domain, repositories |

So `ORNG Registry` in the About heading, in prose, in documentation, and in the window
title. And `orng-registry` in the mono identity line, `~/.orng/` in Settings,
`orng.tools` as the domain, `orng-catalog` in a provenance string.

Read it as an abbreviation rather than a word, which is what the uppercase is for. It does
not stand for anything yet. It will.

Two knock-on edits: the bundle's own filename (`Orange Registry.dc.html`) and its title
still say Orange. About's mono identity line, which already reads `orng-registry`
beside the version and the platform, is correct and should stay lowercase.

## 3. The guard is not a string, and it has three states

`SettingsScreen.dc.html:148`, `README.md:17,29,632,633,641`.

Two separate things here, and the second is the one that changes a drawing.

**Nothing is written into the installation.** The tamper guard is a bytecode edit: the
guard compiles to a value load, a comparison and a branch, and disarming replaces the load
with a zero so the comparison always takes the normal path. The state is read back by
reading that instruction. There is no marker, no version, and no name, so
`disarmed by orng-registry 0.9.2` cannot be rendered - the app does not know who disarmed
an installation or with what. It is also not how "already prepared" is decided.

Please draw the disarmed state as just that: `disarmed`, or `guard disarmed`.

**And there are three states, not two.** `SettingsScreen.dc.html:148` is a boolean,
`guard === "armed"`. The real states are:

| | |
| --- | --- |
| `armed` | Untouched. The normal state of an installation that was never prepared |
| `disarmed` | Prepared |
| `unknown` | The guard site is there but does not have the shape we recognise |

`unknown` is the one worth drawing, because it is the one with a consequence: preparation
**refuses** on it rather than editing blind. It is what a Bitwig release that changed the
guard would look like, so it is the state a user hits on upgrade day, and Settings saying
`armed` there would be a lie that sends them to press Prepare and get a refusal with no
explanation.

## 4. Two passages state opposite provenance rules

`README.md:120` still has the name-matching rule, in the constraints section:

> A Local entry is catalog-sourced iff a catalog item of the **same name** reports ...

`README.md:806` has the corrected one. Line 120 is a leftover, but it is the passage an
implementer reads first, so please delete or update it. Nothing drawn depends on it.

## 5. The provenance link, in the form we owe you

`CatalogDetail.dc.html:101,121`, `Inspector.dc.html:122,160`,
`Orange Registry.dc.html:368-375`, currently `orange-catalog #412`.

The last letter said this field was being added and that the exact form was ours to give
rather than yours to invent. It is built now, so here it is.

The index carries, per item, the **commit that published it** - forty lowercase hex digits.
Not a pull request number. The index is generated from git history on merge, and a commit
is what that history can name; the pull request number survives only inside a commit
message, as text.

| | |
| --- | --- |
| Label | The first seven characters |
| Links to | `https://github.com/zezic/orng-catalog/commit/<the forty>` |
| Suggested rendering | `orng-catalog@3f9a1c2` |

Keep the panel as you drew it - a link with a short label is exactly right, and seven
characters is close enough to `#412` that the layout does not move. If you want a longer
label, `orng-catalog@3f9a1c2` reads well on its own.

The other half of that item is also built: the entry list now records the version an item
was installed at, so the update modal's `installed 2.0.3 -> catalog 2.1.0`
(`README.md:758`) has both numbers behind it. Keep the modal as drawn.

## 6. There is nothing to migrate

`README.md:29` raises migrating `~/.orange-registry/` and accepting both guard strings, and
concludes that either way the UI has to describe the resulting states.

It does not. Nothing has ever been released. No machine has an `~/.orange-registry/`
directory, and no installation has ever been prepared by an `orange-registry` build,
because there has never been a build to prepare one. The rename landed while the project
was unpublished, which is the reason it was done then.

So there is no compatibility state to draw, and no first-launch migration screen. If you
added anything for it, it can come out.

---

## Not changes

- The five-step apply list, the failure message, and the skipped-step treatment on step 5.
  All correct.
- The paths' shape, extension and per-build backup directory naming. Only the root moves.
- Provenance keyed on UUID, at `README.md:806`.
- The numbers, unchanged and still right: 35 MB backup, 34 MB archive, 428 factory entries
  as 152 devices, 43 modulators and 233 Grid modules, `6.1 (94a90411)` as a build string.
- The mono identity line's lowercase `orng-registry`. That one is correct as drawn.
