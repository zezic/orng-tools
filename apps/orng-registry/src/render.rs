// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Rendering the interface without a window, so it can be looked at.
//!
//! egui decides a lot that the calling code does not say out loud: where a
//! baseline sits, how much width is left after a layout, what a margin does to a
//! centred block. Reading the code is not a reliable way to know what came out,
//! and a screenshot is. These write one image per state into
//! `tests/snapshots/`, which is both the check and the record of what changed.

use eframe::egui;
use egui_kittest::kittest::Queryable as _;
use egui_kittest::Harness;
use orng_tools::{
    Condition, Destination, GuardState, Helper, Manifest, OrngHome, RunState, Strategy,
    UserLibrary,
};

use crate::app::{App, View};
use crate::session::{Found, Session};
use crate::settings::{Appearance, Settings};
use crate::catalog::Fetching;
use crate::staging::{self, Staged};
use crate::theme::metric;
use crate::work::{Applying, Stage, State};

/// The window's own size, so what is rendered is what would be seen - and the
/// size the design is drawn at, so a picture can be held against the bundle.
const SIZE: egui::Vec2 = egui::vec2(metric::WINDOW[0], metric::WINDOW[1]);

/// A fixed place to build a fake installation.
///
/// **Relative, and that is the whole point.** The interface draws the
/// installation's path, so whatever this returns ends up in the rendered image.
/// A temporary directory puts a random name there and no two runs can match; an
/// absolute one puts *this machine's* home directory there and no two machines
/// can match, which is how every snapshot but `no-installation` came to fail on
/// every continuous integration runner while passing here.
///
/// Cargo runs a test binary with the package directory as its working
/// directory - the same thing `tests/snapshots` is already resolved against - so
/// a relative path is a real location and a constant string at the same time.
///
/// **Written out rather than joined, and that is the rest of the point.**
/// `Path::display` prints back the separators it was given and `join` adds the
/// platform's own, so a joined fixture reads
/// `target/render-fixtures\unprepared\Bitwig Studio.app` on Windows - two glyphs
/// no other machine draws, in the one string the window puts on screen. Windows
/// takes a forward slash everywhere its API is concerned, so only the drawn text
/// changes, and the drawn text is what is being compared. Asserted below.
fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::path::PathBuf::from(format!("target/render-fixtures/{name}"));
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// The installation inside a fixture, named as an installation is named: the
/// path is drawn, and a fixture that does not look like one teaches the reader
/// to expect something else.
fn install_root(fixture: &std::path::Path) -> std::path::PathBuf {
    std::path::PathBuf::from(format!("{}/Bitwig Studio.app", fixture.display()))
}

/// The path the install bar draws, spelled the same on every platform.
///
/// This is the only assertion here that cannot fail on the machine it was
/// written on, and it is kept for the machine it can: `join` put a backslash in
/// this string on Windows and every snapshot that draws a fixture - seventeen of
/// nineteen - failed there and nowhere else, by the twenty to thirty pixels two
/// glyphs cost. The two that passed are the two that draw a path from a literal.
#[test]
fn the_drawn_installation_path_is_not_a_function_of_the_platform() {
    assert_eq!(
        install_root(&fixture("separators")).display().to_string(),
        "target/render-fixtures/separators/Bitwig Studio.app"
    );
}

fn entries() -> Manifest {
    // Written as the wire format rather than built through the API, so the
    // sample is also a readable example of what is on disk.
    let rows = "#orng-registry 2\n\
        80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
        devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\t\tlocal\n\
        8b330d22-73fa-4ba5-a42f-2f2300cbd8bf\tDEVICE\tVOLSHAPER\t\
        devices/My Devices/VOLSHAPER.bwdevice\tBeat-synced volume LFO\tvolshaper\t1.0.0\tcatalog\n\
        1f2e3d4c-5b6a-4798-8899-aabbccddeeff\tMODULATOR\tSHAPER\t\
        modulators/My Modulators/SHAPER.bwmodulator\tCurve modulator\tshaper\t\tlocal\n\
        2a3b4c5d-6e7f-4801-9192-b3c4d5e6f708\tMODULE\tGATE IN\t\
        modules/My Modules/GATE IN.bwmodule\tGrid gate input\tgate in\t\tlocal\n";
    Manifest::parse(rows).expect("the sample entry list does not parse")
}

/// A build to name in the install bar. Bitwig's own shape: a version, and forty
/// hex characters of revision that the bar shows the first eight of.
fn build() -> Option<orng_tools::BuildId> {
    Some(orng_tools::BuildId {
        version: orng_tools::BitwigVersion::parse("6.1").expect("a version"),
        revision: "94a904110c7f2b3e6d5a81f409cbe27d3a16b850".to_owned(),
    })
}

fn destination(fixture: &std::path::Path) -> Destination {
    Destination {
        install: orng_tools::testing::install(&install_root(fixture)),
        library: UserLibrary::at(&fixture.join("Library")),
        // Under the fixture rather than *at* it, and that matters now that a
        // screen draws these paths: Settings writes a path under the user's home
        // as `~/...`, so a home that was also the installation's parent would draw
        // `~/Bitwig Studio.app` and the picture would record a shape no real
        // machine has. With the home one level in, the entry list and the backups
        // come out as the design draws them and the installation comes out whole.
        home: OrngHome::at(&fixture.join("home")),
        placement: Strategy::Link,
    }
}

fn found(root: &std::path::Path, helper: Helper, guard: GuardState) -> Session {
    found_with(root, helper, guard, entries())
}

fn found_with(
    root: &std::path::Path,
    helper: Helper,
    guard: GuardState,
    entries: Manifest,
) -> Session {
    running_found(root, helper, guard, entries, RunState::Clear)
}

fn running_found(
    root: &std::path::Path,
    helper: Helper,
    guard: GuardState,
    entries: Manifest,
    running: RunState,
) -> Session {
    Session::Found(Box::new(Found {
        to: destination(root),
        condition: Condition { build: build(), helper, guard },
        running,
        entries,
    }))
}

/// One dropped file of each kind a drop can produce, staged by the code that
/// stages a real one.
///
/// Written to disk and read back rather than assembled by hand, because the
/// three answers a row can carry are what staging decides and a hand-built row
/// would draw a conclusion nothing reached.
fn dropped(root: &std::path::Path, to: &Destination, entries: &Manifest) -> Vec<Staged> {
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");

    let write = |name: &str, uuid: &str, display: &str| {
        let path = drop.join(name);
        let kind = orng_tools::Kind::from_path(&path).expect("a document extension");
        let document = orng_tools::testing::document(kind, uuid.parse().unwrap(), display);
        std::fs::write(&path, document.bytes()).expect("could not write the sample");
        path
    };

    let paths = vec![
        write(
            "WAVESHAPER ALPHA.bwdevice",
            "1f6c85d4-9a02-47be-83c1-d5e70b14a629",
            "WAVESHAPER ALPHA",
        ),
        // The same display name as an entry already in the list, under another
        // identity: Bitwig's browser could not tell the two apart.
        write("DISPERSER.bwdevice", "4b71c8d9-21f0-4ab3-8c77-1e9b0d4a6f22", "DISPERSER"),
        {
            // Something renamed to a document extension: it has the length of
            // one and none of the shape.
            let path = drop.join("BROKEN.bwmodule");
            std::fs::write(&path, vec![b'x'; 4096]).expect("could not write the sample");
            path
        },
    ];
    staging::read(&paths, entries, to, &[])
}

/// Lay a state out, and compare it against the stored picture.
///
/// **Running the harness is worth doing everywhere.** It lays every state out,
/// which is where a panic in a layout shows up, and it fails if the interface
/// keeps asking to be redrawn - so it passing is the check that no timer has
/// crept back in.
///
/// **Comparing pixels needs a renderer**, and a continuous integration runner on
/// Linux has none: `egui_kittest` asks wgpu for an adapter and there is not one,
/// not even a software one. The renderer is built lazily on the first
/// comparison, so laying out costs nothing there and only the picture is given
/// up.
///
/// Opt out, never opt in, and never silently. Absent the variable this compares
/// and fails, so a machine that has stopped checking the pictures has to say so
/// out loud - the same discipline `ORNG_SKIP_BITWIG_TESTS` exists for.
fn look<S>(harness: &mut Harness<'_, S>, name: &str) {
    harness.run();
    compare(harness, name);
}

/// The same, for a state egui itself keeps repainting.
///
/// `run` is the assertion that the interface has settled, and it is the check
/// that no timer has crept back in. One state is exempt and only one: egui asks
/// for an immediate repaint on every pass while `hovered_files` is non-empty,
/// because a drag is a gesture in progress and the window has to stay live for
/// it. That is egui's decision, in `InputState::wants_repaint_after`, and not
/// this application polling - so these draw a fixed number of passes instead of
/// waiting for a quiet that cannot come.
fn look_while_dragging<S>(harness: &mut Harness<'_, S>, name: &str) {
    harness.run_steps(SETTLING_PASSES);
    compare(harness, name);
}

/// Enough passes for a layout to settle and for the named font family to bind,
/// which takes the pass after the one that installed it.
const SETTLING_PASSES: usize = 3;

/// A palette asked for by name, which is what a fixture wants.
///
/// Never [`Appearance::System`]: a picture drawn in whatever the runner's desktop
/// happens to be set to is a picture two machines disagree about, and the whole
/// point of these is that they do not.
fn appearance(dark: bool) -> Appearance {
    if dark { Appearance::Dark } else { Appearance::Light }
}

fn compare<S>(harness: &mut Harness<'_, S>, name: &str) {
    let skipping = std::env::var_os("ORNG_SKIP_RENDER_SNAPSHOTS")
        .is_some_and(|value| !value.is_empty());
    if skipping {
        eprintln!("no renderer here: laid {name} out without looking at it");
        return;
    }
    harness.snapshot(name);
}

/// Render one state, with work held still, and write it out.
fn shot_applying(name: &str, applying: Applying) {
    let root = fixture(name);
    let session = found(&root, Helper::Absent, GuardState::Armed);
    let mut session = Some(session);
    let mut applying = Some(applying);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_applying(applying.take().expect("built once"));
        app
    });
    look(&mut harness, name);
}

/// Render the list with documents dropped on it and not yet written.
fn shot_staged(name: &str, helper: Helper, guard: GuardState) {
    let root = fixture(name);
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, helper, guard, entries);

    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_staged(staged.take().expect("built once"));
        app
    });
    look(&mut harness, name);
}

/// Render the window with files held over it but not yet dropped.
///
/// `hovered_files` is the one piece of raw input egui carries from frame to
/// frame rather than taking, which is what makes this drawable at all: a drag is
/// a state the window is in, not an event it received.
fn shot_dragging(name: &str, over: &[&str]) {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let drop = root.join("dragged");
    std::fs::create_dir_all(&drop).expect("a place to drag from");
    for file in over {
        std::fs::write(drop.join(file), b"the drag does not read it").expect("could not write");
    }

    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.input_mut().hovered_files = over
        .iter()
        .map(|file| egui::HoveredFile { path: Some(drop.join(file)), ..Default::default() })
        .collect();
    look_while_dragging(&mut harness, name);
}

/// Render the catalog view with a fetch held still.
fn shot_catalog(name: &str, fetching: Fetching) {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut fetching = Some(fetching);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_catalog(fetching.take().expect("built once"));
        app.show_view(View::Catalog);
        app
    });
    look(&mut harness, name);
}

/// Render one state and write it out under `name`.
fn shot(name: &str, session: Session, view: View, dark: bool) {
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_appearance(appearance(dark), &cc.egui_ctx);
        app.show_view(view);
        app
    });
    look(&mut harness, name);
}

#[test]
fn the_local_list() {
    let root = fixture("local-dark");
    shot("local-dark", found(&root, Helper::Present, GuardState::Disarmed), View::Local, true);
}

#[test]
fn the_local_list_in_light() {
    let root = fixture("local-light");
    shot("local-light", found(&root, Helper::Present, GuardState::Disarmed), View::Local, false);
}

/// The state a machine is in before anything has been done to it, which is what
/// most people will see first.
#[test]
fn an_unprepared_installation() {
    let root = fixture("unprepared");
    shot("unprepared", found(&root, Helper::Absent, GuardState::Armed), View::Local, true);
}

#[test]
fn nothing_installed() {
    shot(
        "no-installation",
        Session::NoInstallation {
            searched: "/Applications, ~/Applications and /opt/bitwig-studio".to_owned(),
        },
        View::Local,
        true,
    );
}

/// Halfway through, which is what a user watches.
#[test]
fn a_preparation_in_flight() {
    use orng_tools::Step;
    shot_applying(
        "preparing",
        Applying::frozen(
            Some([
                (Step::Backup, State::Done),
                (Step::Patch, State::Done),
                (Step::Verify, State::Running),
                (Step::Activate, State::Waiting),
                (Step::Link, State::NotRun),
            ]),
            Stage::Preparing,
            None,
        ),
    );
}

/// The case the transaction exists for: it stopped, and nothing was touched.
#[test]
fn a_preparation_that_failed() {
    use orng_tools::Step;
    shot_applying(
        "preparing-failed",
        Applying::frozen(
            Some([
                (Step::Backup, State::Done),
                (Step::Patch, State::Done),
                (Step::Verify, State::Failed),
                (Step::Activate, State::Waiting),
                (Step::Link, State::Waiting),
            ]),
            Stage::Preparing,
            Some(Err("the patched archive did not load under the bundled JVM".to_owned())),
        ),
    );
}

/// The installation is prepared and the entries are being written, which is the
/// half of a press that the step list does not cover.
#[test]
fn the_entries_being_written_after_a_preparation() {
    use orng_tools::Step;
    shot_applying(
        "registering",
        Applying::frozen(
            Some(Step::ALL.map(|step| (step, State::Done))),
            Stage::Registering,
            None,
        ),
    );
}

/// The state a user reaches the morning after a Bitwig release: an installation
/// is selected and nothing inside it could be located. Nothing can be listed and
/// nothing can be applied, so the region is given over to saying so - and the
/// copy must not read as the user's fault, because it is not.
#[test]
fn a_build_that_was_not_recognised() {
    shot(
        "unknown-build",
        Session::Unreadable {
            root: "/Applications/Bitwig Studio.app".to_owned(),
            why: "no class in this archive registers devices/*.bwdevice".to_owned(),
        },
        View::Local,
        true,
    );
}


/// Bitwig is open and a preparation is pending, which is the one thing that
/// stops the primary action. It blocks only this mode: in the cheap one the
/// banner never appears at all.
#[test]
fn bitwig_is_running() {
    let root = fixture("running");
    shot(
        "running",
        running_found(
            &root,
            Helper::Absent,
            GuardState::Armed,
            entries(),
            RunState::Running(vec!["BitwigStudio".to_owned(), "BitwigAudioEngine".to_owned()]),
        ),
        View::Local,
        true,
    );
}

/// A build whose guard site is not in a shape this version recognises.
/// Preparation refuses before it looks at anything else, so the badge, the
/// banner and the button all have to agree about that.
#[test]
fn a_guard_this_build_does_not_recognise() {
    let root = fixture("unknown-guard");
    shot(
        "unknown-guard",
        found(&root, Helper::Absent, GuardState::Unknown),
        View::Local,
        true,
    );
}

/// The list narrowed to nothing. Minor, and it still has to offer a way out.
#[test]
fn the_filters_match_nothing() {
    let root = fixture("no-match");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_query("wavesh");
        app
    });
    look(&mut harness, "no-match");
}

/// What a press leaves behind. The dialog is gone by then - the work is over -
/// and what happened is stated above the action bar until it is put away.
///
/// The entries-only half of a press, on purpose: a finished *preparation*
/// re-reads the machine, and a picture of this machine is not one any other
/// machine can check.
#[test]
fn entries_that_were_written() {
    let root = fixture("registered");
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries.clone());

    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut applying = Some(Applying::frozen(None, Stage::Registering, Some(Ok(entries))));
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_staged(staged.take().expect("built once"));
        app.set_applying(applying.take().expect("built once"));
        app
    });
    look(&mut harness, "registered");
}

/// The everyday state: pending work pinned above what is registered, with each
/// of the three answers a drop can get.
#[test]
fn documents_dropped_on_a_prepared_installation() {
    shot_staged("staged", Helper::Present, GuardState::Disarmed);
}

/// The same drop onto an installation that has never been prepared. One press
/// does both, and the action bar says which mode it is about to run.
#[test]
fn documents_dropped_on_an_unprepared_installation() {
    shot_staged("staged-unprepared", Helper::Absent, GuardState::Armed);
}

/// The whole window is the target, and what it will take is stated by name
/// before the drop rather than after it.
#[test]
fn files_held_over_the_window() {
    shot_dragging(
        "dragging",
        &["WAVESHAPER ALPHA.bwdevice", "SLEW LIMITER.bwmodule", "BREATH.bwmodulator"],
    );
}

/// A drag carrying nothing this application can take. Refusing before the drop
/// is the whole point of drawing the overlay from the names.
#[test]
fn files_held_over_the_window_that_cannot_be_registered() {
    shot_dragging("dragging-refused", &["notes.txt", "mix.wav"]);
}

/// The onboarding surface: an installation found, nothing registered, nothing
/// dropped. It has to name the extensions and say that the first run needs
/// Bitwig closed.
#[test]
fn nothing_registered_yet() {
    let root = fixture("onboarding");
    shot(
        "onboarding",
        found_with(&root, Helper::Absent, GuardState::Armed, Manifest::default()),
        View::Local,
        true,
    );
}

/// Monospaced runs are set as tightly as the design sets them.
///
/// The one text measurement in the bundle that means anything. The mockup
/// resolves Inter from the network and falls back when there is none, so its
/// proportional runs are the wrong letterforms at the wrong widths - but it
/// loads the real Iosevka from `uploads/`, so a mono run there is directly
/// comparable to one here. `94a90411` at 10px measures 44 wide in the bundle.
///
/// Without the design's `-0.05em` this laid out at 48: four pixels on eight
/// characters, and the same 9% on every identity, path, count and version in
/// the window. Which is why this is asserted on the advance width rather than
/// left to a picture - a snapshot shows text that looks like text either way.
#[test]
fn monospaced_runs_are_set_as_tightly_as_the_design_sets_them() {
    let root = fixture("tracking");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    // Laid out through `font::format`, which is what the widgets use: the
    // tracking lives on the run and not on the `FontId`, so measuring the font
    // alone would report the untracked width and pass whatever happened.
    use crate::theme::font;
    let job = egui::text::LayoutJob::single_section(
        "94a90411".to_owned(),
        font::format(font::mono(font::MONO_TIGHT)),
    );
    let width = harness.ctx.fonts_mut(|fonts| fonts.layout_job(job)).rect.width();
    // The bundle's own number, out of `getBoundingClientRect` on that span.
    const BUNDLE: f32 = 44.0;
    assert!(
        (width - BUNDLE).abs() <= 2.0,
        "the build revision lays out {width} wide, the design draws it {BUNDLE}"
    );
}

/// The empty state's pair sits in the middle of the window.
///
/// Asserted against the controls themselves rather than left to a picture. The
/// pair used to be centred from an estimate of how wide the labels would be -
/// six-and-a-bit pixels a character - which over-stated them by 69 pixels on
/// this screen and put the block 35 to the left. A snapshot froze that happily,
/// because a snapshot records what was drawn and has no opinion about where the
/// middle is.
#[test]
fn the_empty_states_controls_are_centred_in_the_window() {
    let root = fixture("centred");
    let session = found_with(&root, Helper::Absent, GuardState::Armed, Manifest::default());
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    // By role as well as by label: the label alone matches the button and the
    // text node inside it, which are not the same rectangle.
    use egui::accesskit::Role;
    // Every match, not the first. A control inside a centring layout leaves
    // more than one node in the accessibility tree - egui lays the block out
    // once to find out how big it is and once to place it - and they differ in
    // `y` while sharing `x` and width. Which of them is the drawn one is not
    // worth depending on, and this is a question about horizontal position, so
    // the horizontal extremes answer it whichever order they arrive in.
    let edges = |label: &str| {
        let rects: Vec<_> = harness
            .get_all_by_role_and_label(Role::Button, label)
            .map(|node| node.rect())
            .collect();
        assert!(!rects.is_empty(), "{label} is not on this screen");
        let left = rects.iter().map(|r| r.left()).fold(f32::INFINITY, f32::min);
        let right = rects.iter().map(|r| r.right()).fold(f32::NEG_INFINITY, f32::max);
        (left, right)
    };
    let (action_left, action_right) = edges("Browse the catalog");
    let (alt_left, alt_right) = edges("Add files...");

    let middle = (action_left + alt_right) / 2.0;
    let window = metric::WINDOW[0] / 2.0;
    assert!(
        (middle - window).abs() <= 1.0,
        "the pair is centred on {middle}, the window on {window}"
    );

    // And they are one block rather than two: the design's gap, not egui's.
    let gap = alt_left - action_right;
    assert!(
        (gap - metric::TOOL_GAP).abs() <= 0.5,
        "the pair is {gap} apart, the design says {}",
        metric::TOOL_GAP
    );
}

/// The catalog as it is published today, drawn from a real index rather than a
/// made-up one, so what is rendered is a shape the catalog actually produces.
#[test]
fn the_catalog_view() {
    let index = orng_catalog::Index::parse(include_str!("../tests/published-index.json"))
        .expect("the sample index does not parse");
    shot_catalog("catalog", Fetching::frozen(Ok(index)));
}

/// The catalog's detail panel, on the one item the catalog publishes today.
///
/// From the real index, like the browse view it opens out of: what is drawn is
/// a shape the catalog actually produces, down to the licence still reading
/// `TBA` and the commit the item was reviewed in.
#[test]
fn a_catalog_item_in_detail() {
    let index = orng_catalog::Index::parse(include_str!("../tests/published-index.json"))
        .expect("the sample index does not parse");
    shot_detail("catalog-detail", index, VOLSHAPER);
}

/// An item another one has replaced, on an installation too old to load it.
///
/// Synthetic, and it has to be: the published catalog contains neither state,
/// and both are things the design writes copy for. The notice is the design's
/// own words - a new identity is what lets both be installed at once, which is
/// the whole reason the catalog publishes the old one at all.
#[test]
fn a_catalog_item_that_was_superseded() {
    let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index does not parse");
    shot_detail("catalog-detail-superseded", index, "c0ffee00-1111-4222-8333-444455556666");
}

/// Two items, the second replacing the first, and the first needing a Bitwig
/// newer than the fixture's 6.1.
const SUPERSEDED: &str = r#"{
  "schema": 2,
  "revision": "bd1f83732b8b093154e7956baa724e9855ba8995",
  "items": [
    {
      "uuid": "c0ffee00-1111-4222-8333-444455556666",
      "kind": "modulator",
      "name": "BREATH FOLLOWER",
      "author": "mono-lab",
      "slug": "breath-follower",
      "version": "1.2.0",
      "min_bitwig": "6.4",
      "license": "MIT",
      "description": "Envelope follower with a breath curve, for ducking a pad under a vocal.",
      "keywords": ["breath", "follower", "duck", "envelope"],
      "path": "content/mono-lab/breath-follower/BREATH FOLLOWER.bwmodulator",
      "digest": "bfdab5de7d0cf6981d6e7252afb8c925071161174788e03916040eb785f3b29a",
      "size": 18320,
      "homepage": "mono-lab.dev/breath",
      "merged_in": "8c41d0b9a3e5f7126d4b80ca35fe91d7b2064e83"
    },
    {
      "uuid": "d0d0caf0-2222-4333-8444-555566667777",
      "kind": "modulator",
      "name": "BREATH FOLLOWER II",
      "author": "mono-lab",
      "slug": "breath-follower-ii",
      "version": "2.0.0",
      "min_bitwig": "6.0",
      "license": "MIT",
      "description": "The breath follower again, with a parameter set that could not be added in place.",
      "keywords": ["breath", "follower", "duck"],
      "path": "content/mono-lab/breath-follower-ii/BREATH FOLLOWER II.bwmodulator",
      "digest": "bfdab5de7d0cf6981d6e7252afb8c925071161174788e03916040eb785f3b29a",
      "size": 19004,
      "supersedes": ["c0ffee00-1111-4222-8333-444455556666"],
      "merged_in": "3f9a1c2e8b4d7a61c05f2d93ab7e14c8f6021b5d"
    }
  ]
}"#;

/// A catalog row opens the detail, the notice walks to the replacement, and the
/// panel's own control closes it.
///
/// Three presses a picture cannot check, and the third is the one worth having:
/// `See BREATH FOLLOWER II` is the only control in the window that moves a
/// panel from one thing to another, so a snapshot of it says nothing about
/// whether it arrives.
///
/// The narrowing is asserted through the version column, which the narrow
/// catalog grid has no room for: `2.0.0` belongs to the second row, so it is on
/// screen while the list is wide, gone while the panel is open on the first
/// item, and back again in the panel once the notice has been followed.
#[test]
fn a_catalog_row_opens_the_detail_and_the_notice_walks_to_the_replacement() {
    let root = fixture("detail-opening");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
        app.set_catalog(Fetching::frozen(Ok(index)));
        app.show_view(View::Catalog);
        app
    });
    harness.run();
    assert!(harness.query_by_label("2.0.0").is_some(), "the list is not showing versions");

    harness.get_by_label("BREATH FOLLOWER").click();
    harness.run();
    assert!(
        harness.query_by_label(crate::widget::icon::DISMISS).is_some(),
        "clicking a catalog row did not open the detail"
    );
    assert!(
        harness.query_by_label("2.0.0").is_none(),
        "the detail is open and the list still has a version column"
    );

    harness.get_by_label_contains("See BREATH FOLLOWER II").click();
    harness.run();
    assert!(
        harness.query_by_label("2.0.0").is_some(),
        "the notice did not move the panel to the item that replaces this one"
    );

    harness.get_by_label(crate::widget::icon::DISMISS).click();
    harness.run();
    assert!(
        harness.query_by_label("1.2.0").is_some(),
        "the panel's own control did not close it"
    );
}

fn shot_detail(name: &str, index: orng_catalog::Index, open: &str) {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut index = Some(index);
    let open = open.parse().expect("a sample identity");
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_catalog(Fetching::frozen(Ok(index.take().expect("built once"))));
        app.show_view(View::Catalog);
        app.set_detailing(open);
        app
    });
    look(&mut harness, name);
}

/// The overflow menu, open.
///
/// The only state here that has to be *reached* rather than assembled, and the
/// reason it is worth reaching: the control used to end in
/// `Response::context_menu`, which opens on a secondary click, so a primary
/// click on the three dots did nothing and this surface had never been drawn.
/// A picture of a menu that no press opens is not a thing any other snapshot
/// can be missing.
#[test]
fn the_overflow_menu() {
    let root = fixture("menu");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();
    // By the glyph the control draws, so the press lands on the control the
    // design names rather than on a position measured off a picture.
    harness.get_by_label(crate::widget::icon::OVERFLOW).click();
    look(&mut harness, "menu");
}

/// The inspector, open on a registered entry.
///
/// `VOLSHAPER` and not the first row, because it is the one entry in the sample
/// that came from the catalog: the source line and its version are drawn, and
/// the placement is read off the disk rather than assumed.
#[test]
fn the_inspector() {
    shot_inspector("inspector", true);
}

/// The same panel in light, which is where its shadow can actually be seen.
///
/// The panel is the first thing to draw on `panel_2`, in a field fill, on the
/// accent with ink over it, and on a tone wash - four of the palette's slots
/// that nothing else in the window uses - and its shadow is the one colour here
/// derived from the design rather than stated by it. A dark picture proves none
/// of that.
#[test]
fn the_inspector_in_light() {
    shot_inspector("inspector-light", false);
}

fn shot_inspector(name: &str, dark: bool) {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_appearance(appearance(dark), &cc.egui_ctx);
        app.set_inspecting(VOLSHAPER.parse().expect("a sample identity"));
        app
    });
    look(&mut harness, name);
}

/// The catalog entry in [`entries`], by identity.
const VOLSHAPER: &str = "8b330d22-73fa-4ba5-a42f-2f2300cbd8bf";

/// A row opens the inspector, and the inspector's own control closes it again.
///
/// The class of fault a picture structurally cannot catch, and one this
/// application has already had: the overflow menu was drawn correctly and no
/// press opened it, because the call it ended in wanted a secondary click. A
/// panel that only `set_inspecting` could reach would be the same thing with a
/// snapshot to vouch for it.
///
/// The narrowing is asserted through the identity column rather than through a
/// rectangle: the narrow grid has no room for one, so an identity on screen is
/// an identity the list found room for.
#[test]
fn a_row_opens_the_inspector_and_the_panel_closes_itself() {
    let root = fixture("opening");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    let shortened = "8b330d22";
    assert!(harness.query_by_label(shortened).is_some(), "the list is not showing identities");

    // On the name, which is what anybody aims at, and which is a label: egui
    // makes labels selectable text by default and a selectable label senses
    // clicks, so this press used to land on the label and stop there while the
    // empty half of the same row opened the panel.
    harness.get_by_label("VOLSHAPER").click();
    harness.run();
    assert!(
        harness.query_by_label(crate::widget::icon::DISMISS).is_some(),
        "clicking a row did not open the inspector"
    );
    assert!(
        harness.query_by_label(shortened).is_none(),
        "the inspector is open and the list still has an identity column"
    );
    // The panel is taller than the window gives it, in the bundle as well as
    // here, so its last group is below the fold and no picture of it exists.
    // Asserted rather than left to the snapshot for exactly that reason.
    assert!(
        harness.query_by_label_contains("Reveal file").is_some(),
        "the panel's action list was not laid out"
    );

    harness.get_by_label(crate::widget::icon::DISMISS).click();
    harness.run();
    assert!(
        harness.query_by_label(shortened).is_some(),
        "the inspector's own control did not close it"
    );
}

/// A word typed into the keyword field becomes a keyword.
///
/// The one interaction in the panel that is not a press, and the reason the
/// panel exists at all: what a registered device's description and keywords say
/// is what makes it findable in Bitwig's browser. Asserted through the panel
/// rather than against the buffer behind it, because "Enter commits the word"
/// is a statement about the field and not about a `Vec`.
#[test]
fn a_word_typed_into_the_inspector_becomes_a_keyword() {
    use egui::accesskit::Role;

    let root = fixture("keywords");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_inspecting(VOLSHAPER.parse().expect("a sample identity"));
        app
    });
    harness.run();

    // The lower of the panel's two fields. The description is above it, and
    // both are inside the panel rather than out on the toolbar.
    let panel = metric::WINDOW[0] - metric::ASIDE;
    let adding = harness
        .get_all_by_role(Role::TextInput)
        .filter(|node| node.rect().left() > panel)
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the panel has nowhere to add a keyword");
    adding.focus();
    adding.type_text("reverb");
    harness.run();
    harness.key_press(egui::Key::Enter);
    harness.run();

    assert!(
        harness.query_by_label("reverb").is_some(),
        "the word was typed and pressing Enter did not make it a keyword"
    );
    // And it is still the field the next one goes into: Enter means "and
    // another", not "and that is the last".
    let panel = metric::WINDOW[0] - metric::ASIDE;
    let still = harness
        .get_all_by_role(Role::TextInput)
        .filter(|node| node.rect().left() > panel)
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the field went away");
    assert!(still.is_focused(), "Enter dropped the user out of the list they were writing");
}

/// The Settings screen, on the installation the rest of these are drawn against.
///
/// **A full-window surface**, so there is no install bar, no toolbar and no
/// action bar in this picture - which is the thing to check first, because
/// drawing it as a panel over the list would look nearly right.
#[test]
fn the_settings_screen() {
    shot_settings("settings", true, Settings::default());
}

/// The same screen in light.
///
/// Worth a second picture here more than anywhere else: this screen is what makes
/// the light palette reachable at all. Until it existed, light was drawn only by
/// the render tests, and a palette nobody can select is a palette nobody checks.
#[test]
fn the_settings_screen_in_light() {
    shot_settings("settings-light", false, Settings::default());
}

/// Every toned control in its other state: the second placement chosen, and the
/// delete-file default on.
///
/// The two above draw the first choice selected and the checkbox clear, so
/// between them they never show the accent wash on a choice, a filled radio below
/// an empty one, a checked mark, or either warmed sentence. The design states all
/// five separately and this is the only picture of them.
#[test]
fn the_other_half_of_every_settings_control() {
    let settings = Settings {
        placement: Strategy::Copy,
        delete_file: true,
        ..Settings::default()
    };
    shot_settings("settings-chosen", true, settings);
}

fn shot_settings(name: &str, dark: bool, settings: Settings) {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    // The palette goes in with the rest of the preferences rather than through
    // `set_appearance` beside them - which is what the window actually does, and
    // what the first attempt at this got wrong: the setter ran first, the
    // preferences replaced everything it had set, and `settings-light.png` came
    // out in dark with a passing test beside it.
    let mut settings = Some(Settings { appearance: appearance(dark), ..settings });
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_settings(settings.take().expect("built once"));
        app.show_settings();
        app
    });
    look(&mut harness, name);
}

/// The overflow opens Settings, and Settings goes back.
///
/// The class of fault a picture structurally cannot catch, and the one this
/// application has already had twice: the overflow menu was drawn correctly and
/// no press opened it, and a row's name could not be clicked while the empty half
/// of the row could. A screen that only `show_settings` could reach would be the
/// same thing with three snapshots to vouch for it.
///
/// Reached by the label a user aims at, and the swap is asserted through the
/// install bar rather than through a rectangle: the screen replaces that bar, so
/// the build revision beside the installation's name is on screen while the list
/// is and gone while Settings is.
#[test]
fn the_overflow_opens_settings_and_settings_goes_back() {
    let root = fixture("settings-opening");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    let revision = "94a90411";
    assert!(harness.query_by_label(revision).is_some(), "the install bar is not drawn");

    harness.get_by_label(crate::widget::icon::OVERFLOW).click();
    harness.run();
    harness.get_by_label_contains("Settings").click();
    harness.run();

    assert!(
        harness.query_by_label_contains("Document placement").is_some(),
        "the overflow's Settings item did not open the screen"
    );
    assert!(
        harness.query_by_label(revision).is_none(),
        "Settings is open and the install bar is still drawn - it is a panel, not a screen"
    );
    // The column is taller than the window gives it, in the bundle as well as
    // here, so the last two groups are below the fold and no picture of them
    // exists. Asserted rather than left to a snapshot for exactly that reason.
    assert!(
        harness.query_by_label_contains("Copy report").is_some(),
        "the diagnostics group was not laid out"
    );
    assert!(
        harness.query_by_label_contains("About ORNG Registry").is_some(),
        "the row at the foot of the screen was not laid out"
    );

    // And the way out is labelled with the view it goes back to.
    harness.get_by_label_contains("Local").click();
    harness.run();
    assert!(
        harness.query_by_label(revision).is_some(),
        "the screen's own control did not go back to the list"
    );
}

/// Choosing a placement in Settings reaches the destination every write is handed.
///
/// Not a picture: what this asserts is that a radio button changes where a
/// document would go, and the only honest place to read that is the `Destination`
/// the worker is given. A screen whose controls moved nothing would render
/// identically.
#[test]
fn choosing_a_placement_changes_where_a_document_would_go() {
    let root = fixture("settings-placement");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.show_settings();
        app
    });
    harness.run();
    assert_eq!(harness.state().placement(), Some(Strategy::Link), "not the default placement");

    harness.get_by_label_contains("Copy documents into the installation").click();
    harness.run();
    assert_eq!(
        harness.state().placement(),
        Some(Strategy::Copy),
        "the choice was made and the destination still says otherwise"
    );
}

/// An index that did not verify. Nothing is listed, and the reason is shown.
#[test]
fn a_catalog_that_does_not_verify() {
    shot_catalog(
        "catalog-refused",
        Fetching::frozen(Err("the signature does not match this index under this key".to_owned())),
    );
}



