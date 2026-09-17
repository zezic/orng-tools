# ORNG Catalog - UI requirements (design round 2)

Read with `ui-spec.md` (the app as designed so far) and `design-review.md` (corrections to
round 1, which are still outstanding). This document covers one new feature and only the
places it touches the existing design.

Same rules as before: this says what must exist and how it must behave, not how it looks.

---

## 1. What the catalog is

**ORNG Catalog** is a public, curated repository of devices, modulators and Grid modules
contributed by the community. ORNG Registry can install from it directly, so a user gets
one place to see what exists instead of hunting for downloads, unzipping archives and
dragging files.

In user terms: *the app already manages what you have; the catalog shows you what you could
have.*

Two facts about it shape everything below:

- **Content is small.** Items are 20 to 30 KB. Installing is effectively instant, and the
  whole index is a single small file. Nothing here needs a download manager.
- **An item is executable DSP.** A device is a signal graph that Bitwig runs. The
  repository's review process is the only trust boundary, so the author is not decoration
  and must be visible before installing.

---

## 2. Where it lives

This is the first real decision and it breaks a round-1 rule.

The current design has one primary view and puts Settings, Restore and About behind an
overflow menu, each replacing the window. That was right when the app had one activity.
It now has two: **manage what is registered**, and **discover what is available**. They are
peers - neither is a setting, and neither belongs inside the other.

**Recommended: two top-level views with a two-item switch**, `Entries` and `Catalog`. The
install bar and action bar stay put across both; only the middle region changes. Settings,
Restore and About stay in the overflow exactly as they are.

A two-item switch is not the tab bar that decision ruled out. It is the smallest affordance
that admits the app has two jobs.

Two alternatives considered and why they lose:

- *Catalog as a fourth overflow screen.* Consistent with Settings, but files a primary
  activity under a menu that otherwise holds maintenance. Users would not find it.
- *Catalog items mixed into the entries list, behind a filter.* One list and one mental
  model, which is genuinely attractive. But the catalog will hold hundreds of items against
  a user's handful, so the default must be installed-only - which hides the catalog behind
  a filter chip and lands back at the discoverability problem, with a noisier list.

Worth pushing back on if the design finds something better. The constraint is that browsing
must be reachable in one obvious action from the main window.

---

## 3. The catalog view

### Toolbar

Parallel to the entries toolbar so the two views feel like one app, with one difference
that matters:

- **Search** covers name, author, description and keywords. The entries view searches name
  and UUID. Different fields, because the questions are different: "what is this thing I
  have" versus "is there a thing that does X". State the scope in the placeholder.
- **Kind filters** identical to the entries view.
- **Installed** filter: `All` / `Installed` / `Updatable`. Not a checkbox - three states,
  and `Updatable` is the one a returning user wants.
- Index freshness and a manual refresh live here or in the action bar. See section 7.

### The row

An item is not an entry, and the row should not pretend otherwise. The entries row leads
with kind and identity; a catalog row leads with **what it is and who made it**.

Required, in reading order:

1. **Kind** - same treatment as the entries view.
2. **Name**, with a one-line description beneath it. The description is the only thing that
   answers "what is this", so it is not optional detail here.
3. **Author** - a first-class column, not a detail. It is the trust signal.
4. **Version** - monospace.
5. **Status** - see below.
6. **Primary action** - `Install`, `Update`, or nothing when installed and current.

The UUID does **not** belong in the row. It identifies a thing you already have; it does
not help you choose one. It belongs in the detail panel.

### Statuses

| Status | Meaning | Action |
| --- | --- | --- |
| `Available` | In the catalog, not installed | `Install` |
| `Installed` | Installed and current | none; row is quieter |
| `Update available` | Installed, catalog has a newer version of the same identity | `Update` |
| `Superseded` | Installed, and the catalog has a **replacement under a new identity** | `See replacement` |
| `Incompatible` | Needs a newer Bitwig than the detected installation | none; state the required version |
| `Failed` | Download or verification failed | `Retry`, with the reason |

`Update available` and `Superseded` look similar and are completely different events.
Section 5 is about that, and it is the part of this feature most likely to be got wrong.

---

## 4. Item detail

Opens from a row, in the same 272px panel the entries view uses. Contents:

- Name, kind, author, version.
- Full description.
- **Requires Bitwig \<version\> or newer**, stated whether or not it is satisfied.
- Licence.
- Keywords, as the same pills the entry inspector uses - these become the search keywords
  once installed, and the user can still edit them afterwards in the entries view.
- Homepage or source link, if the author gave one.
- **Provenance**: a link to the merged change in the repository. This is what makes review
  the trust boundary rather than a claim about one.

  The index carries this per item. It is not derivable from the index's own revision, which
  names the commit the whole index was generated from and therefore traces the index rather
  than the item; the two must not be confused in the UI.
- UUID, monospace, secondary.
- Primary action, plus `Remove` when installed.

---

## 5. Install, update, supersede

### Install

Installing writes a document, adds a row to the entry list, and writes a description and
keywords. On a prepared installation that is all **Update entries** work: no backup, no
confirmation, Bitwig may stay open, takes effect on next launch. So `Install` is genuinely
one click and the design should treat it that way.

On an unprepared installation it is **Prepare install** work, with all of that mode's
weight - exactly as dropping a file is today. Same rule, no new case.

### Update, and the hazard

**An update is not like updating an app.** Bitwig resolves a device by identity to a file in
its library; a project stores parameter values and the identity, not the structure. So
replacing the file changes every project that already uses that device.

The repository's rules keep this safe - changes that alter the parameter set must take a new
identity instead - but "safe" here means "your projects still load", not "nothing changes".
A user updating a device is changing something that reaches backwards in time.

So updates need a confirmation that installs do not, and it must say the thing plainly:
projects that use this device will use the new version. One sentence, stated before the
action, no ceremony beyond that.

### Supersede

When an author makes an incompatible revision, the catalog publishes it as a **new item
with a new identity** and marks the old one superseded. The old one stays published so
existing projects keep working.

This is the subtlest state in the feature and needs designed copy, because the honest
explanation is counterintuitive: *a newer version exists as a separate device. Installing it
leaves your projects alone. Both can be installed at once.*

It must not look like a pending update the user is behind on. It is an offer, not a debt.
Whether the user then removes the old one is their call and a separate action.

---

## 6. What changes in the entries view

Small, but required:

- **Provenance per entry**: local file, or a catalog item at a version. A marker on the row
  and the full detail in the inspector.

  **Keyed on UUID, never on display name.** The entry list records where each entry came
  from and at which version, so the match is exact. Display names are allowed to collide in
  the repository and the app renames entries when they do, precisely because the name is not
  the identity - so a name-keyed match would eventually mark the wrong row, and would do it
  first to the user who hit the rename path.

  This is also what makes `Update available` computable. It needs the installed version and
  the catalog version, and the installed one is recorded rather than inferred: comparing the
  file against the catalog's digest would only say that it differs, which cannot tell
  `Update available` from `Changed`. Those have different causes and different remedies and
  the UI is right to keep them apart.
- A catalog-sourced entry gains `Update available` as a status, alongside the existing ones.
- Removing a catalog-sourced entry is less consequential than removing a local one - it can
  be reinstalled in one click. The delete-the-file question (see `design-review.md` item 5)
  can default differently here, and the copy can say so.

---

## 7. Network and trust states

The app currently has no network surface at all. This adds a failure class that did not
exist, and it should be handled in the least alarming way that is still honest.

- **Offline or fetch failed.** Show the cached index with its age stated plainly
  (`Catalog from 3 days ago`). Browsing still works, installing a cached item still works
  if it downloads. This is a degraded state, not an error, and should not be a banner.
- **Never fetched.** First run with no connection. An empty state that explains and offers
  `Try again`.
- **Stale index.** Anything older than a threshold gets the age shown more prominently, plus
  `Refresh`.
- **Download failed.** Per-item, `Retry`. Ordinary.
- **Verification failed.** The downloaded file does not match the hash the index states.
  This is **not** a network error and must not read like one - it means the file is not what
  the repository says it is. Refuse the install, keep the item in a `Failed` state, and
  offer `Copy details`. It is the one genuinely alarming case here and deserves its own
  treatment.

Refresh should be automatic and quiet on view entry with a cached index, never a modal,
never blocking the list.

---

## 8. What does not change

Stated so the design does not drift:

- The install bar and action bar are shared across both views.
- The two apply modes, and everything in `design-review.md`.
- Settings, Restore and About stay in the overflow.
- No account, no sign-in, no telemetry. The app fetches two kinds of file over HTTPS and
  that is the whole network surface.
- Publishing to the catalog is **not** in the app. It is a pull request. The app may link to
  instructions; it does not upload.

---

## 9. Open questions

1. **Navigation.** Is the two-item switch right, or is there a shape that admits two primary
   activities without adding a top-level control? This is the one decision I would most like
   challenged.
2. **`Superseded` presentation.** It must read as an offer and not a nag, while still being
   noticeable. Badge, inline note, or something in the detail panel only?
3. **Does the catalog row need the description inline**, or is name plus author enough with
   the description in the panel? Inline costs vertical space and makes rows uneven; without
   it, browsing means opening every item.
4. **Empty catalog search** versus the entries view's "no match" - same treatment, or does
   browsing deserve a suggestion (clear filters, browse all)?
5. **Where index freshness lives.** Toolbar, action bar, or only surfaced when stale?
