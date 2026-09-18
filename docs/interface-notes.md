# Building the interface against the design bundle

How to check that what the window draws is what `docs/design_handoff_orng_registry/`
draws, and the traps that have already cost a session each. Written because every one
of these was looked at, called fine, and was not.

`ui-spec.md` is **not** followed. It predates the design, and its section 12 - headed
"Hint only" - was once read as a layout. Where the two disagree the bundle wins.
Anything the bundle does not draw is asked about rather than invented.

---

## 1. The bundle is HTML, so render it

The mockups are self-contained pages. Headless Chrome draws them, and the comparison
becomes two images and a column of measurements rather than an opinion.

```bash
cd docs/design_handoff_orng_registry
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" \
  --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=2 \
  --virtual-time-budget=5000 --window-size=820,42 \
  --screenshot=/tmp/comp-InstallBar.png --allow-file-access-from-files \
  "file://$PWD/InstallBar.dc.html"
```

**Render each component at its own `$preview` size**, which is in the `data-props`
attribute of its `.dc.html`. At any other window size a component that is `width:100%`
fills the viewport and measures nothing useful.

| Component | size |
| --- | --- |
| `InstallBar` | 820 x 42 |
| `ListToolbar` | 820 x 42 |
| `ActionBar` | 820 x 52 |
| `EntryRow` | 796 x 36 |
| `EmptyState` | 820 x 420 |

`ORNG Registry.dc.html` renders the whole shell, but only its default screen: the
others are picked by clicking, which headless Chrome will not do.

## 1a. Take the screen out of the page, and ask the page for its numbers

Both problems - only one screen, and the shell's chrome around it - are fixed by one
copy of the shell with three edits. Write it beside the original so its relative
imports still resolve, and delete it afterwards; it is a tool, not a document.

1. **Make the screen an argument.** The component's `state = { theme: "dark", screen:
   "main", ...}` becomes `screen: new URLSearchParams(location.search).get("screen") ||
   "main"`, and the same for `theme`. Every key of `SCREENS` is then a URL.
2. **Hide everything but the window.** Append to the `<style>` in `<helmet>`:
   `#ordoc{padding:0!important}` plus `display:none` for the title block, the chip
   panel and the caption - they are `#ordoc > div > div:nth-child(1)`, `(2)` and
   `(3) > div:nth-child(2)`. At `--window-size=820,560` the mockup then *is* the
   viewport, and a pixel in it is a pixel in ours.
3. **Have it report its own geometry.** Append a script that waits for the render,
   walks `#dc-root` calling `getBoundingClientRect` on every element, and writes tag,
   left, top, width, height and text into a `<pre id="rects">`. Then:

```bash
"/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" --headless=new \
  --disable-gpu --window-size=820,560 --virtual-time-budget=4000 --dump-dom \
  --allow-file-access-from-files "file://$PWD/Probe.dc.html?screen=catalog"
```

**This is the measurement that ends arguments.** It is the design's own numbers rather
than an inference from pixels: `DIV 0 140 820 36` is a row, and the four cells inside
it are the grid. It found the missing 84px actions column in `EntryRow`, which no
screenshot shows because the column is empty - the bundle hides those controls off
hover and keeps their space.

Read the `.dc.html` of a component beside it. The rects say where things are; the
component's `rowStyle`, `chip()` and `tab()` say *why*, and carry the numbers for
states the shell does not happen to render.

**Caveat.** Chrome resolves Inter from Google Fonts and has no network in this
sandbox, so it falls back. Letterforms and text widths in a rendered mockup are not
trustworthy; geometry, colour and control boxes are. Measure boxes, not glyphs.

## 2. Ours renders at the same size

`metric::WINDOW` is 820 x 560, the size the design is drawn at, and both the window
and the render fixtures use it. Take a 2x shot by adding
`.with_pixels_per_point(2.0)` to the harness builder.

**The bundle's window is 30 pixels taller than ours in every shot**, because it draws
a mock title bar that a real window does not have: its content starts at y=30 and runs
to 560, ours starts at 0. Heights and widths compare directly; absolute y does not.

Regenerate ours at 2x without disturbing the committed pictures: copy `render.rs`
aside, add `.with_pixels_per_point(2.0)`, run `UPDATE_SNAPSHOTS=1 cargo test -p
orng-registry --release render::`, copy the pictures out, then put `render.rs` back and
`git checkout -- apps/orng-registry/tests/snapshots`. **Restore by copy and not by
`git checkout` on the source**, or an edit made to `render.rs` during the same session
is thrown away with it.

## 3. Measure, do not look

A scanline through a bar, above the text, gives the control boxes:

```python
im = Image.open(path).convert('RGB').crop(box); px = im.load()
y = 20  # 10 logical, inside the controls and above their labels
bg = px[2, y]
# contiguous runs where the pixel differs from the bar's background
```

Compare the runs: start, end and width of every control. What this has caught that
eyes did not - a field 22px too wide, gaps 6px too large, a row pitch of 42 against
36, an 8px margin around the entire window.

Scanning *through* the text finds glyph fragments and is useless. Scan above it, or
scan through it and merge runs less than about eight pixels apart: the merged runs are
the columns, and comparing those against the rects from section 1a is how a column
that starts 96 pixels late is caught in one line of output.

The same down a column gives the bands: the bars, the section headings, the rows and
their pitch. The dominant colour of each scanline, collapsed into runs, reads as the
window's vertical structure - 42, 42, 26, 36, 36, 36, 26, ..., 52 - and that list
against the bundle's is the whole layout in one comparison.

---

## The traps, all of them found the hard way

**`item_spacing` is added between every allocated widget.** The theme sets six
pixels, which is right between a label and the thing it labels and wrong anywhere
the design states its own gaps. It made rows 42 apart instead of 36, and bar gaps 14
and 8 instead of 8 and 2. The list zeroes the vertical half in `widget::list`; the
three bars zero the horizontal half. **A new container that lays out to the design's
numbers must zero it too.**

**A widget's inner margin subtracts its state's outline width.** From
`widget_style.rs`: `button_padding + expansion - bg_stroke.width`. A hovered state
carrying a one-pixel outline is therefore two pixels narrower than the resting one,
and the whole bar slides as the pointer crosses it. Every state's `bg_stroke` and
`expansion` are zero, which is also what the bundle draws. Passing `Button::stroke`
does **not** help: the margin comes from the style, not from the button.

**An explicit `Button::fill` defeats every hover state.** Controls that should
respond go through `widget::filled_button`, which sets the pair on a scope.

**A child `Ui` is positioned before its height is known**, so a block inside a
centring layout lands against the top edge. `widget::centred_block` and
`widget::empty_state` measure first, in a detached `Ui` with a sizing pass, then
allocate at that height. Do not use `ui.scope_builder` for the measuring pass - it
allocates as well as measures, and the block ends up at the bottom of the window.

**`egui_kittest`'s `build_ui` wraps the app in an 8px outer margin** of its own. The
fixtures use `build_eframe`, which has none and runs the real `eframe::App`. Fonts
are also installed outside a pass there, so named families bind from the first frame.

**A named `FontFamily` is unbound until the pass after `set_fonts`,** and asking for
one panics. `font::emphasis` and `font::icon` check first.

**InterDisplay carries 745 Private Use Area glyphs** for its stylistic alternates,
and Phosphor's icons live in the same range. An icon font added as a *fallback*
behind Inter therefore renders Latin letters for half the set. Icons have a family
of their own, with nothing in front of them.

**egui requests a repaint every pass while `hovered_files` is non-empty**
(`InputState::wants_repaint_after`), because a drag is a gesture in progress. That is
why the two drag snapshots use `run_steps` where every other state uses `run`, which
is the check that nothing polls.

**Anything drawn from a timestamp must be formatted where it is read.** A time
rendered at the point of drawing is rendered in the drawing machine's zone, and the
picture then differs by a day west of Denver.

**Render fixtures use paths relative to the package directory.** An absolute one puts
the machine's home directory into the image and every runner disagrees.

**And they are written out rather than joined.** `Path::display` prints back the
separators it was given; `Path::join` contributes the platform's own. A fixture built
with `join` reads `target/render-fixtures\unprepared\Bitwig Studio.app` on Windows,
which is two glyphs no other machine draws, in the one string the install bar puts on
screen. Seventeen of the nineteen snapshots failed there and only there; the two that
passed are the two that draw their path from a literal.

**The picture is a function of the code, and that has been measured rather than
assumed.** egui rasterises text itself into its own atlas, so the renderer underneath
changes nothing. The proof: substituting the two separators `join` would have added
reproduced Windows's own pixel counts on macOS *exactly*, snapshot for snapshot - 30,
33, 22, 15, 28, 24 and the rest. Nothing else in nineteen images differed between
Metal and DirectX. So a snapshot that disagrees across platforms is reporting a real
difference in what was drawn, and is worth reading rather than re-recording.
