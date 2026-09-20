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
use crate::catalog::{Fetching, Install};
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

/// Written as the wire format rather than built through the API, so the sample
/// is also a readable example of what is on disk.
///
/// `VOLSHAPER`'s two catalog columns are the real published index's own - the
/// version it publishes and the commit it names as having published it - so the
/// inspector's Source block is drawn from a pair that actually goes together and
/// the link under it leads somewhere that exists.
const ROWS: &str = "#orng-registry 4\n\
    80c0dc4c-d142-53a7-85ee-b91427819b66\tDEVICE\tDISPERSER\t\
    devices/My Devices/DISPERSER.bwdevice\tAllpass phase-rotator\tdisperser allpass\t\t\t\tlocal\n\
    8b330d22-73fa-4ba5-a42f-2f2300cbd8bf\tDEVICE\tVOLSHAPER\t\
    devices/My Devices/VOLSHAPER.bwdevice\tBeat-synced volume LFO\tvolshaper\t\t1.0.0\t\
    175add8af8422d4b749467c187952dd6204fcadb\tcatalog\n\
    1f2e3d4c-5b6a-4798-8899-aabbccddeeff\tMODULATOR\tSHAPER\t\
    modulators/My Modulators/SHAPER.bwmodulator\tCurve modulator\tshaper\t\t\t\tlocal\n\
    2a3b4c5d-6e7f-4801-9192-b3c4d5e6f708\tMODULE\tGATE IN\t\
    modules/My Modules/GATE IN.bwmodule\tGrid gate input\tgate in\t\t\t\tlocal\n";

fn entries() -> Manifest {
    Manifest::parse(ROWS).expect("the sample entry list does not parse")
}

/// The same sample cut to its first row.
///
/// Cut rather than written out again, so the one row is the row the rest of the
/// suite draws and not a second sample that can drift from it. What it is for is
/// a count of one: the action bar's hidden-entries note is only drawn when the
/// filter hid the whole list, so the singular is unreachable on a machine with
/// four entries on it.
fn one_entry() -> Manifest {
    let mut lines = ROWS.lines();
    let header = lines.next().expect("the sample names its format");
    let first = lines.next().expect("the sample has a first row");
    Manifest::parse(&format!("{header}\n{first}\n")).expect("the cut sample does not parse")
}

/// Make the entry list above true: link the library folders and put a document
/// where each entry says its document is.
///
/// Without this the fixture is a list of claims about files that were never
/// written, and every registered row reads `Missing file` - which is a picture
/// of a broken installation rather than of the screen being checked. The
/// documents are *copied* rather than linked, which the design allows for and a
/// relative fixture requires: linking writes a symbolic link whose target is
/// the string it was given, and these paths are relative on purpose, so the
/// link would resolve against its own folder and lead nowhere. An absolute
/// library would fix the link and put this machine's home into the Settings
/// screen's picture, which is the failure `fixture` above exists to prevent.
///
/// Answers with the list the placement recorded, so the digests are the ones
/// the placed bytes hash to. Written here rather than into the literal above:
/// four sixty-four character hashes would make the sample unreadable, and would
/// be a hand-copied claim about bytes rather than a fact taken from them.
fn placed(to: &Destination, entries: Manifest) -> Manifest {
    let to = &Destination { placement: Strategy::Copy, ..to.clone() };
    let mut update = orng_tools::Update::to(Manifest::default());
    for entry in entries.entries() {
        let document = orng_tools::testing::document(entry.kind, entry.uuid, &entry.name);
        update.add(entry.clone(), document);
    }
    update.apply(to).expect("the fixture's documents could not be placed")
}

/// The sample list on a machine where two of its documents have gone wrong:
/// `SHAPER` deleted from under its entry, `DISPERSER` rewritten by something
/// that is not this application.
///
/// Damaged after the documents are placed and before the session looks at
/// them, which is the order a real machine reaches this state in - the entries
/// were registered while the files were still right, and something happened to
/// the files afterwards.
fn damaged(root: &std::path::Path) -> Session {
    let to = destination(root);
    let entries = placed(&to, entries());
    let at = |name: &str| {
        entries
            .entries()
            .iter()
            .find(|entry| entry.name == name)
            .expect("the sample list carries it")
            .library_path
            .resolve(&to.install)
    };
    std::fs::remove_file(at("SHAPER")).expect("the fixture's modulator could not be removed");
    std::fs::write(at("DISPERSER"), b"not the document that was registered")
        .expect("the fixture's device could not be rewritten");
    Session::Found(Box::new(Found::new(
        to,
        Condition { build: build(), helper: Helper::Present, guard: GuardState::Disarmed },
        RunState::Clear,
        entries,
    )))
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
    let to = destination(root);
    let entries = placed(&to, entries);
    Session::Found(Box::new(Found::new(
        to,
        Condition { build: build(), helper, guard },
        running,
        entries,
    )))
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

/// The catalog view with a fetch held still, on an installation with nothing
/// registered.
fn catalog_with(name: &str, fetching: Fetching) -> Harness<'static, App> {
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
    harness.run();
    harness
}

/// Render the catalog view with a fetch held still.
fn shot_catalog(name: &str, fetching: Fetching) {
    let mut harness = catalog_with(name, fetching);
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

/// And the catalog's, which is a different sentence over the same icon.
///
/// The bundle keeps them apart on purpose - `EmptyState.dc.html:93` against
/// `:99`: this one names three filters because the catalog has three, and the
/// list behind it is one somebody was browsing rather than one they own. The
/// toolbar stays above it either way, which is the half of this a picture does
/// hold: the control that undoes the filter must not go away with the rows.
#[test]
fn the_catalogs_filters_match_nothing() {
    let root = fixture("catalog-no-match");
    let session = copying(&root, superseded_entries());
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
        app.set_catalog(Fetching::frozen(Ok(index)));
        app.show_view(View::Catalog);
        app.set_query("granular");
        app
    });
    look(&mut harness, "catalog-no-match");
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
    shot_detail("catalog-detail", index, VOLSHAPER, entries());
}

/// An item another one has replaced, and the item that replaces it.
///
/// Synthetic, and it has to be: the published catalog contains neither state,
/// and both are things the design writes copy for. The notice is the design's
/// own words - a new identity is what lets both be installed at once, which is
/// the whole reason the catalog publishes the old one at all.
///
/// **Registered, and that is what makes the state reachable.** The design's own
/// sample shows the replaced item installed, and it has to be: a published item
/// nobody has is simply `Available`, however many things replace it, because the
/// offer is to whoever already owns the old one. The row in the list behind the
/// panel shows the other half - the replacement, not installed, offering
/// `Install`.
#[test]
fn a_catalog_item_that_was_superseded() {
    let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index does not parse");
    shot_detail("catalog-detail-superseded", index, BREATH_FOLLOWER, superseded_entries());
}

/// The sample list with the superseded item registered in it, at the version
/// the sample index publishes - so the row reads `Replacement available` and
/// not `Update available`, which is the quieter of the two on purpose.
fn superseded_entries() -> Manifest {
    breath_follower_registered_at("1.2.0")
}

/// The same list a version behind, which is the only way to reach
/// `Update available` on a catalog row: it is the one of the seven states that
/// needs this machine and the index to disagree about a *version* rather than
/// about an identity.
fn outdated_entries() -> Manifest {
    breath_follower_registered_at("1.0.0")
}

fn breath_follower_registered_at(version: &str) -> Manifest {
    let row = format!(
        "{BREATH_FOLLOWER}\tMODULATOR\tBREATH FOLLOWER\t\
         modulators/My Modulators/BREATH FOLLOWER.bwmodulator\t\
         Envelope follower with a breath curve\tbreath follower duck envelope\t\t{version}\t\
         8c41d0b9a3e5f7126d4b80ca35fe91d7b2064e83\tcatalog\n"
    );
    Manifest::parse(&format!("{ROWS}{row}")).expect("the sample entry list does not parse")
}

const BREATH_FOLLOWER: &str = "c0ffee00-1111-4222-8333-444455556666";

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
    let session =
        found_with(&root, Helper::Present, GuardState::Disarmed, superseded_entries());
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

fn shot_detail(name: &str, index: orng_catalog::Index, open: &str, entries: Manifest) {
    let root = fixture(name);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);
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
    //
    // Named through the glyph as well as the words. The row beneath offers
    // `Reveal file` too, and the pointer is still on that row after the press
    // that opened the panel, so the words alone now match twice. The panel's
    // item is the one carrying both on a single node, which is what
    // `labelled_icon` builds and what a row's icon-only control is not.
    let in_the_panel = format!("{}Reveal file", crate::widget::icon::REVEAL);
    assert!(
        harness.query_by_label(&in_the_panel).is_some(),
        "the panel's action list was not laid out"
    );

    harness.get_by_label(crate::widget::icon::DISMISS).click();
    harness.run();
    assert!(
        harness.query_by_label(shortened).is_some(),
        "the inspector's own control did not close it"
    );
}

/// A window on the Local view over a given machine, laid out once.
fn local_window(session: Session) -> Harness<'static, App> {
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();
    harness
}

/// A window with the sample list in it, and nothing else set.
fn listing(name: &str) -> Harness<'static, App> {
    local_window(found(&fixture(name), Helper::Present, GuardState::Disarmed))
}

/// The list with the pointer on a row, which is the only way the row's own
/// controls are ever on screen.
///
/// The one picture of them, and the reason it is worth having: the geometry is
/// asserted through the tree, but which glyph each control wears and what the
/// hover does to the ink are not things a rectangle holds. The other list
/// snapshots are all drawn with the pointer nowhere, so before this one nothing
/// looked at these at all.
#[test]
fn a_row_under_the_pointer() {
    let name = "row-actions";
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();
    // Between the name and the identity rather than on either. Both of those
    // carry a tooltip of their own - the library path and "click to copy" - and
    // a picture of the row's controls with a tooltip over the row beneath it is
    // a picture of the tooltip.
    let row = harness.get_by_label("DISPERSER").rect();
    harness.hover_at(egui::pos2(400.0, row.center().y));
    look(&mut harness, name);
}

/// What the list says about documents that are no longer what the entry list
/// says they are.
///
/// **The two are kept apart and the design says why**: same remedy on the
/// surface, different cause, and the cause is what the user needs in order to
/// act. So this asserts both words at once, against rows that reached those
/// states by having something done to their files - a check that answered
/// `Changed` for a file that is merely gone would offer `Locate file` nowhere
/// and `Reveal file` on nothing.
///
/// Through the tree rather than through the picture, because a word is what is
/// being claimed here and the colours are `statuses.png`'s job.
#[test]
fn a_document_that_went_missing_and_one_that_was_rewritten_say_so_separately() {
    let session = damaged(&fixture("statuses"));
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    assert!(harness.query_by_label("Missing file").is_some(), "the deleted document said nothing");
    assert!(harness.query_by_label("Changed").is_some(), "the rewritten document said nothing");
    // And the two untouched ones are still what they were, so this is telling
    // rows apart rather than marking the whole list.
    assert_eq!(
        harness.query_all_by_label("Registered").count(),
        // The band above the list wears the same word as the rows under it.
        3,
        "an untouched document was reported as something other than registered"
    );

    // The controls follow the status, which is the point of computing it: the
    // one state where revealing cannot work must offer the action that fixes it
    // instead. `SHAPER` is the row whose document was deleted.
    harness.get_by_label("SHAPER").hover();
    harness.run();
    assert!(harness.query_by_label("Locate file").is_some(), "a missing file offered no remedy");
    assert!(
        harness.query_by_label("Reveal file").is_none(),
        "a missing file offered to reveal the file that is missing"
    );
}

/// The inspector offers the remedy for a missing file, and does not offer to
/// reveal one.
///
/// The bundle's README names this as the defect worth checking, because it is
/// the one it made during design: the panel offered `Reveal file` on the one
/// entry whose file cannot be found, and omitted the action that fixes it. It
/// was unreachable until this application could compute `Missing file`, and
/// this is the test that it stayed fixed once it was reachable.
///
/// Named through the glyph as well as the words, for the reason
/// `the_inspector` gives: a row's control and the panel's item wear the same
/// words, and only the panel's carries both on one node.
#[test]
fn the_inspector_on_a_missing_file_offers_to_locate_it_and_not_to_reveal_it() {
    let session = damaged(&fixture("inspector-missing"));
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    // `SHAPER` is the entry whose document was deleted from under it.
    harness.get_by_label("SHAPER").click();
    harness.run();
    assert!(
        harness.query_by_label(crate::widget::icon::DISMISS).is_some(),
        "clicking the row did not open the inspector"
    );

    let locate = format!("{}Locate file...", crate::widget::icon::LOCATE);
    let reveal = format!("{}Reveal file", crate::widget::icon::REVEAL);
    assert!(harness.query_by_label(&locate).is_some(), "the panel offered no remedy");
    assert!(
        harness.query_by_label(&reveal).is_none(),
        "the panel offered to reveal a file that cannot be found"
    );
    // And the removal is still there, which is the other half of the design's
    // row for this state: every entry can be got rid of, broken or not.
    let remove = format!("{}Remove entry", crate::widget::icon::REMOVE);
    assert!(harness.query_by_label(&remove).is_some(), "a broken entry could not be removed");
}

/// An entry the catalog has moved on from says so, and one it has not does not.
///
/// Both halves matter. A window that has never fetched the catalog knows
/// nothing about what is published, and must not answer "up to date" for the
/// same reason it must not answer "out of date" - so the first assertion here
/// is that a list drawn with no index says nothing at all.
///
/// The index is the published one with a version moved forward, rather than an
/// invented row: the entry list fixture carries `VOLSHAPER` at the version the
/// catalog really publishes, so the untouched index is already the "nothing to
/// update" case and one field is the whole difference between them.
#[test]
fn an_entry_says_when_the_catalog_has_a_newer_revision_of_it() {
    let mut harness = listing("update-available");
    let published = || {
        orng_catalog::Index::parse(include_str!("../tests/published-index.json"))
            .expect("the sample index does not parse")
    };

    assert!(
        harness.query_by_label("Update available").is_none(),
        "a window with no catalog claimed to know what is published"
    );

    harness.state_mut().set_catalog(Fetching::frozen(Ok(published())));
    harness.run();
    assert!(
        harness.query_by_label("Update available").is_none(),
        "an entry at the published version was offered an update to it"
    );

    let mut newer = published();
    newer.items[0].version = "2.1.0".parse().expect("a version");
    harness.state_mut().set_catalog(Fetching::frozen(Ok(newer)));
    harness.run();
    // One row and not the list: the other three are local files, which have
    // nothing upstream to be behind.
    assert_eq!(
        harness.query_all_by_label("Update available").count(),
        1,
        "the update was claimed for entries with nothing upstream"
    );
}

/// Draw until neither worker is still out there.
///
/// The one place a test waits rather than asserting, and it has to: a fetch and
/// a write are both detached threads that answer through a channel the window
/// reads when it draws, so `Harness::run` returns as soon as the frame it drew
/// asked for nothing more - which is before the answer exists. **Looked for a
/// handle to wait on and there is none, deliberately**: the window must not
/// block on a worker, which is the whole reason they are detached.
///
/// The frame that takes an answer is also the frame that draws it, so when this
/// returns the tree already carries the result.
fn settle(harness: &mut Harness<'static, App>) {
    // Generous, because a real write reaches the disk: three documents, three
    // description bundles and the entry list. Whatever this is, it is a bound on
    // how long the test hangs before saying so, and not an interval the answer
    // waits for - the loop draws first and checks after.
    const GIVE_UP_AFTER: usize = 200;
    const BETWEEN_LOOKS: std::time::Duration = std::time::Duration::from_millis(10);
    for _ in 0..GIVE_UP_AFTER {
        harness.run();
        if !harness.state().is_working() {
            return;
        }
        std::thread::sleep(BETWEEN_LOOKS);
    }
    panic!("the window was still working after {GIVE_UP_AFTER} looks");
}

/// The same machine as [`found_with`], with the *copy* strategy in force.
///
/// What a test that actually writes needs, and for the reason [`placed`] already
/// copies: a fixture's paths are relative on purpose, and a symbolic link
/// resolves against its own folder rather than against the working directory -
/// so a document linked into one of these lands somewhere that does not exist,
/// and the row that was just registered reads `Missing file`. The strategy is a
/// preference either way and the design allows both, so this is the fixture
/// being possible rather than the test being lenient.
fn copying(root: &std::path::Path, entries: Manifest) -> Session {
    let to = Destination { placement: Strategy::Copy, ..destination(root) };
    let entries = placed(&to, entries);
    Session::Found(Box::new(Found::new(
        to,
        Condition { build: build(), helper: Helper::Present, guard: GuardState::Disarmed },
        RunState::Clear,
        entries,
    )))
}

/// The catalog's list, on the sample index with both replacement items, against
/// whatever this machine is said to have registered.
///
/// The list is the constant and the machine is the variable, because that is
/// where a published item's state comes from: the index says the same thing to
/// everybody, and every one of the seven words is what one machine makes of it.
fn catalog_listing(name: &str, entries: Manifest) -> Harness<'static, App> {
    let root = fixture(name);
    let session = copying(&root, entries);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
        app.set_catalog(Fetching::frozen(Ok(index)));
        app.show_view(View::Catalog);
        app
    });
    harness.run();
    harness
}

/// A published item's state is a fact about this machine, and every one of the
/// four this fixture can reach is read off it rather than declared.
///
/// Asserted through the tree and not against a picture, because what is being
/// claimed is which word went on which row: the two rows here are the same kind
/// by the same author, and a picture of them with the words swapped looks
/// exactly as right.
#[test]
fn a_published_item_says_what_this_machine_has_to_say_about_it() {
    let harness = catalog_listing("catalog-states", superseded_entries());

    // Registered, and something in the index replaces it. The quieter of the
    // two installed-and-there-is-more states, and it must not read as an
    // update: an update keeps the identity and this does not.
    assert!(harness.query_by_label("Replacement available").is_some());
    assert!(
        harness.query_by_label("Update available").is_none(),
        "a replacement was reported as an update to the same device"
    );
    // The replacement itself, which nobody has.
    assert!(harness.query_by_label("Available").is_some());
    assert!(harness.query_by_label("Install").is_some(), "an available item offered no install");
    // And the one this Bitwig is too old for is not among them, because it is
    // installed: telling somebody they need a newer Bitwig for something
    // already in their browser is telling them nothing they can act on.
    assert!(
        harness.query_by_label("Needs Bitwig 6.4").is_none(),
        "a registered item was reported as one this installation cannot load"
    );

    // The same index against a machine with neither item. The older one now
    // states the version it needs, and the replacement's existence says nothing
    // at all: the offer is to whoever already has the old one.
    let neither = catalog_listing("catalog-states-empty", entries());
    assert!(
        neither.query_by_label("Needs Bitwig 6.4").is_some(),
        "an item this 6.1 installation cannot load did not say so"
    );
    assert!(
        neither.query_by_label("Replacement available").is_none(),
        "an item nobody has was offered a replacement for it"
    );
}

/// A catalog row's control, measured against the bundle's own numbers.
///
/// `CatalogRow.dc.html:105-107`: `height:24px; padding:0 10px`, right-aligned in
/// the 92 the grid reserves. Measured through the tree rather than written
/// against `metric::CATALOG_ACTION`, which is what let a 22-pixel control pass
/// while the constant said 24.
///
/// **And drawn with the pointer nowhere**, which is the design's difference from
/// an entry row: the bundle gates this control on there being one and never on
/// hover. A list read to decide something must not hide the deciding.
#[test]
fn a_catalog_rows_control_is_the_bundles_size_and_is_not_hidden_off_hover() {
    let harness = catalog_listing("catalog-action-size", superseded_entries());

    let install = harness.get_by_label("Install").rect();
    assert_eq!(install.height(), 24.0, "the control is not the design's height");
    // Right-aligned against the row's own twelve of padding, which is where the
    // reserved column ends - `a_catalog_row_is_divided_as_the_bundle_divides_it`
    // puts that edge at 808. Two rows in a 560-tall window need no scrollbar.
    assert_eq!(install.right(), 808.0);
    assert!(install.width() > 2.0 * 10.0, "there is no room for the padding, let alone the word");
    // Centred down the row rather than sitting on its top edge, and clear of
    // the status beside it - both read off the word in its own column, which is
    // the one thing on this row that is certainly on this row.
    let word = harness.get_by_label("Available").rect();
    assert_eq!(install.center().y, word.center().y);
    assert!(word.right() < install.left(), "the status and the control overlap");
}

/// The detail panel's foot, measured against `CatalogDetail.dc.html:89-98`.
///
/// `padding:11px 12px`, a 30-tall `Remove` at the left end and a 32-tall primary
/// at the right. Two heights, and the taller one is the accent-filled press -
/// which is the design saying which of the two the panel is for.
/// Both halves of the bar, each in the state that draws it, against opposite
/// ends of the panel.
///
/// Two panels and not one, because no state draws both: the primary belongs to
/// an item this machine does not have and the removal to one it does. Which is
/// itself worth pinning - a bar with both controls would mean a state that
/// offered to install something already installed.
#[test]
fn the_detail_panels_foot_is_the_bundles_bar() {
    // The panel is 272 wide at the right of an 820 window, and the bar is padded
    // twelve inside it.
    const PANEL_LEFT: f32 = 820.0 - 272.0;

    let available = detail_on("catalog-foot-available", entries(), BREATH_FOLLOWER_II);
    // **The row's control and the panel's primary wear the same word**, which is
    // the design's own doing: the panel offers what the row offers, with room
    // for the word either way. So they are told apart by which side of the panel
    // they are on rather than by their label.
    let primary = available
        .query_all_by_label("Install")
        .map(|node| node.rect())
        .find(|rect| rect.left() > PANEL_LEFT)
        .expect("the panel offered nothing to install");
    assert_eq!(primary.height(), 32.0, "the primary is not the design's height");
    assert_eq!(primary.right(), 820.0 - 12.0);
    assert!(
        available.query_all_by_label("Remove").count() == 0,
        "the panel offered to remove an item this machine does not have"
    );

    let installed = detail_on("catalog-foot-installed", superseded_entries(), BREATH_FOLLOWER);
    let remove = installed.get_by_label("Remove").rect();
    assert_eq!(remove.height(), 30.0, "the removal is not the design's height");
    assert_eq!(remove.left(), PANEL_LEFT + 12.0);
    // The two are the same height of bar apart from each other: the shorter
    // control is centred in it rather than sitting on its floor, so the taller
    // one's centre is where the shorter one's is.
    assert_eq!(remove.center().y, primary.center().y);
}

/// One control on a toolbar, as drawn.
///
/// **Two nodes carry each of these words**, and the second is not a second
/// control: a toolbar measures its chips and the box at its right end in an
/// invisible sizing pass before it can work out how wide the search field is,
/// and a sizing pass reaches the accessibility tree. The one that was drawn is
/// the one the row placed, so it is the rightmost; the probe is laid out at the
/// bar's own left edge. The same trap `widget::centred_block` sets on the
/// action bar, and the same way out - take extremes across every match.
fn on_the_bar<'t>(harness: &'t Harness<'static, App>, label: &'t str) -> egui_kittest::Node<'t> {
    harness
        .get_all_by_label(label)
        .max_by(|a, b| a.rect().right().total_cmp(&b.rect().right()))
        .expect("the bar has no such control")
}

/// The same, for a control in the middle region rather than on a bar.
///
/// The displacement is the other way round: an empty state measures into a
/// probe at the top of the region it is centred in, so the drawn control is the
/// lower of the two. One rule either way - take the extreme away from wherever
/// the probe was laid out.
fn in_the_region<'t>(harness: &'t Harness<'static, App>, label: &'t str) -> egui_kittest::Node<'t> {
    harness
        .get_all_by_label(label)
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the region has no such control")
}

/// Whether anything in the window carries this word.
///
/// `query_by_label` insists on exactly one node and several of these words are
/// on two - a control that was measured as well as drawn, or a name the list
/// and the panel both write.
fn anywhere<S>(harness: &Harness<'_, S>, label: &str) -> bool {
    harness.query_all_by_label(label).next().is_some()
}

/// The catalog toolbar, measured against `CatalogToolbar.dc.html` at both the
/// widths the bundle was probed at.
///
/// Three claims a picture cannot hold. The search field is capped 104 pixels
/// wider than the Local toolbar's, and a picture of a wide field says nothing
/// about which cap held it. The install filter stands at the right edge of the
/// bar, which is a subtraction rather than a look. And its segments are 22 tall
/// inside a 26 well - four pixels that a `min_size` cannot state on its own,
/// because the theme's `interact_size` is a floor under every button.
#[test]
fn the_catalog_toolbar_is_the_bundles_bar() {
    use egui::accesskit::Role;

    let harness = catalog_listing("catalog-toolbar", superseded_entries());

    // The field's box, which is what the bundle's `max-width` is measured
    // across: the text area plus the eight of padding the frame puts either
    // side of it, which is the same eight a bar puts between its boxes. Its
    // right edge at 332 is the design's 12 of gutter and the 320 cap.
    let text = harness.get_by_role(Role::TextInput).rect();
    assert_eq!(text.right() + metric::TOOL_GAP, metric::PAD + metric::CATALOG_SEARCH_FIELD);

    // The install filter against the other gutter. The well is two pixels
    // outside its last segment, so the group ends at 808 where the bundle's
    // does.
    let last = on_the_bar(&harness, "Updatable").rect();
    assert_eq!(last.right() + metric::SEGMENTS_PAD, metric::WINDOW[0] - metric::PAD);
    // Every segment the design's height, and not a bar control's.
    for word in ["All", "Installed", "Updatable"] {
        let seg = on_the_bar(&harness, word).rect();
        assert_eq!(seg.height(), metric::INSTALL_FILTER_SEGMENT, "{word}");
        assert_ne!(seg.height(), metric::CONTROL, "{word} grew to a bar control's height");
    }
    // In the order the design wrote them, which a reversed layout would have
    // got wrong while leaving every other number here right.
    let held = on_the_bar(&harness, "All").rect();
    assert!(held.right() < on_the_bar(&harness, "Installed").rect().left());
    assert!(on_the_bar(&harness, "Installed").rect().right() < last.left());
    // And on the line the chips are on, rather than two pixels above them.
    assert_eq!(held.center().y, on_the_bar(&harness, "Modulators2").rect().center().y);

    // Beside the panel the whole bar is 272 narrower, and both ends move with
    // it: the filter to the panel's edge, and the field off its cap.
    let open = detail_on("catalog-toolbar-narrow", superseded_entries(), BREATH_FOLLOWER_II);
    let narrow_last = on_the_bar(&open, "Updatable").rect();
    assert_eq!(
        narrow_last.right() + metric::SEGMENTS_PAD,
        metric::WINDOW[0] - metric::ASIDE - metric::PAD
    );
    let narrow_text = open.get_by_role(Role::TextInput).rect();
    assert!(
        narrow_text.width() < text.width(),
        "the field kept its full width on a bar 272 narrower"
    );
}

/// The three filters, each narrowing the list and each saying so.
///
/// The index is the constant here and the machine is the variable, as it is
/// everywhere else in the catalog: `BREATH FOLLOWER` is registered and reads
/// `Replacement available`, and `BREATH FOLLOWER II` is not and reads
/// `Available`. So `Installed` must keep exactly the first and `Updatable`
/// neither - which is the one thing the design's three-state control is for.
#[test]
fn the_install_filter_shows_what_it_says_and_says_when_it_shows_nothing() {
    let mut harness = catalog_listing("catalog-filtering", superseded_entries());
    assert!(anywhere(&harness, "BREATH FOLLOWER II"), "the list did not draw");

    // Installed: the registered one stays, the one nobody has goes.
    on_the_bar(&harness, "Installed").click();
    harness.run();
    assert!(anywhere(&harness, "BREATH FOLLOWER"));
    assert!(
        !anywhere(&harness, "BREATH FOLLOWER II"),
        "an item this machine does not have was shown under Installed"
    );
    // And the facet counts follow the filter rather than the list: the shell
    // counts what the install filter left, so one modulator and not two.
    assert!(anywhere(&harness, "Modulators1"));

    // Updatable: neither, because an update keeps the identity and a
    // replacement does not. The design's own quieter word, and here it is the
    // difference between one row and none.
    on_the_bar(&harness, "Updatable").click();
    harness.run();
    assert!(!anywhere(&harness, "BREATH FOLLOWER"));
    assert!(anywhere(&harness, "Nothing in the catalog matches"));
    // The toolbar stays put over the empty state, because it is how the filter
    // gets undone - `ORNG Registry.dc.html:484` sets `toolbar` on this
    // scenario. A chip is what says so: it is drawn by the bar and by nothing
    // else, and it is still counting against a list with no rows in it.
    assert!(anywhere(&harness, "Modulators0"));

    in_the_region(&harness, "Clear filters").click();
    harness.run();
    assert!(anywhere(&harness, "BREATH FOLLOWER II"), "the filter was not undone");
    assert!(anywhere(&harness, "Modulators2"), "the counts were not undone");
}

/// Whether the action bar's summary reads `Catalog <SEP> <what>`.
///
/// Two nodes carry it, for `on_the_bar`'s reason at one remove:
/// `widget::centred_block` lays the summary out once into a sizing pass to learn
/// its height and once for real, and a sizing pass reaches the tree. Nothing
/// here is about where it was drawn, so existence across both is the check.
fn bar_says(harness: &Harness<'static, App>, what: &str) -> bool {
    anywhere(harness, &format!("Catalog {} {what}", crate::widget::SEPARATOR))
}

/// The action bar counts what the view under it is showing.
///
/// `App::summary` ran the staged-and-queued arithmetic whatever was on screen,
/// so the Catalog view's bar read `Nothing pending` over a list of things to
/// install - a true sentence about the other list. The bundle words this bar as
/// a count of the catalog and a note about what a press would cost, and every
/// one of its ten catalog scenarios says so: `ORNG Registry.dc.html:433`.
///
/// **The number follows the install filter and nothing else**, which is the
/// shell's own arithmetic and the same one the toolbar's facets run on. That is
/// the half a picture cannot hold: a summary reading `2 items` over a list of
/// two rows looks exactly as right whether it counted the index or the rows, and
/// only a search that hides both tells them apart.
#[test]
fn the_action_bar_counts_the_catalog_and_not_the_local_pending_work() {
    use egui::accesskit::Role;

    let separator = crate::widget::SEPARATOR;
    let mut harness = catalog_listing("catalog-summary", superseded_entries());

    // `:433`. Two items published, and the note is what the press costs - the
    // Local bar's own note in the words the catalog states it in.
    assert!(bar_says(&harness, "2 items"), "the catalog bar does not count the catalog");
    assert!(
        !anywhere(&harness, "Nothing pending"),
        "the catalog bar is still reporting the Local list's pending work"
    );
    let cost = format!("Installing is Update entries work {separator} no backup, \
                        Bitwig may stay open");
    assert!(anywhere(&harness, &cost), "the bar does not say what a press would cost");

    // The filter names what is being counted, so it names the word. One of the
    // two is registered here and neither has moved on.
    on_the_bar(&harness, "Installed").click();
    harness.run();
    assert!(bar_says(&harness, "1 installed"), "the count did not follow the install filter");

    on_the_bar(&harness, "Updatable").click();
    harness.run();
    assert!(bar_says(&harness, "0 updates available"));
    // The list has gone empty under the filter, and `:486` moves that into the
    // note rather than into the count.
    assert!(
        anywhere(&harness, "No item matches the current search and filters"),
        "the bar says nothing about a list the filters emptied"
    );

    // And a search moves the note without moving the number: `:486` reads the
    // whole catalog over a query that matches none of it.
    on_the_bar(&harness, "All").click();
    harness.run();
    let field = harness.get_by_role(Role::TextInput);
    field.focus();
    field.type_text("granular");
    harness.run();
    assert!(
        !anywhere(&harness, "BREATH FOLLOWER"),
        "the query matched something, so this proves nothing about the count"
    );
    assert!(bar_says(&harness, "2 items"), "the search narrowed the count the bundle does not");
    assert!(anywhere(&harness, "No item matches the current search and filters"));
}

/// `Catalog <SEP> 1 update available`, and the one sentence the design puts
/// under it.
///
/// A fixture of its own because it is the only catalog state that needs this
/// machine and the index to disagree about a version: everything else in the
/// sample list turns on whether an identity is registered at all. `:438-439` is
/// both halves - the count the `Updatable` filter names, and the note, which is
/// the one thing about an update that an install does not share.
#[test]
fn an_update_the_catalog_publishes_is_counted_as_one_and_says_what_it_costs() {
    let mut harness = catalog_listing("catalog-summary-update", outdated_entries());
    assert!(anywhere(&harness, "Update available"), "the fixture reaches no updatable row");

    on_the_bar(&harness, "Updatable").click();
    harness.run();
    assert!(bar_says(&harness, "1 update available"), "one update was not counted as one");
    assert!(
        anywhere(&harness, "An update changes the device in projects that already use it"),
        "the bar does not say what an update reaches into"
    );
    // Singular, which is the whole of why the count is formatted rather than
    // interpolated.
    assert!(!anywhere(&harness, "1 updates available"));
}

/// The two states where the bar is not a count, and the tones that carry them.
///
/// Both are about something that failed, and the design separates them by how
/// much: an index that did not arrive is a degraded window (`:474`, warn) and an
/// install that was refused is a press that did not do what it said it would
/// (`:481`, error). A count in either place would be the bar reporting on a list
/// while the thing the user just did went unmentioned.
#[test]
fn a_catalog_that_did_not_arrive_and_an_install_that_was_refused_replace_the_count() {
    let unavailable = catalog_with(
        "catalog-bar-unavailable",
        Fetching::frozen(Err("the signature does not match this index under this key".to_owned())),
    );
    assert!(anywhere(&unavailable, "Catalog unavailable"));
    assert!(
        !bar_says(&unavailable, "0 items"),
        "an index that did not verify was counted as an empty catalog"
    );

    let mut refused = catalog_listing("catalog-bar-refused", superseded_entries());
    let item = orng_catalog::Index::parse(SUPERSEDED)
        .expect("the sample index parses")
        .items
        .pop()
        .expect("the sample index is not empty");
    refused.state_mut().set_installing(Install::finished(
        item,
        Err(crate::catalog::Refused::Download("the connection closed".to_owned())),
    ));
    settle(&mut refused);
    assert!(anywhere(&refused, "Install refused"));
    assert!(
        !bar_says(&refused, "2 items"),
        "the bar went on counting over a press that did not do what it said"
    );
}

/// What the Local bar says when the filter has emptied the list under it.
///
/// `ORNG Registry.dc.html:490` is the one Local scenario whose note is not the
/// cost of a press: the summary goes on counting the pending work and the note
/// carries what the filter did. The empty state behind it offers the way out
/// and states no number, so until this the window said nothing at all about how
/// much of the list was still there.
///
/// Every claim here is invisible in a picture of the one state. A note reading
/// `4 entries hidden` over an empty list looks exactly as right whether it
/// counted the list, the filter or the machine; only the filtered-but-not-empty
/// case and the nothing-registered case tell those apart.
#[test]
fn the_local_bar_says_how_many_entries_the_filter_is_hiding() {
    let hidden = |harness: &Harness<'static, App>, what: &str| {
        anywhere(harness, &format!("{what} hidden by the current filter"))
    };

    // Nothing filtered. All four of the sample entries are on screen, and the
    // bar has nothing to say about where they went.
    let mut harness = listing("local-hidden");
    assert!(anywhere(&harness, "DISPERSER"), "the list did not draw");
    assert!(!hidden(&harness, "4 entries"), "the bar counted a list nothing is hiding");

    // Filtered, but not emptied: two of the four carry `shaper`. A note here
    // would be spending the bar's one line describing a list the user can see.
    harness.state_mut().set_query("shaper");
    harness.run();
    assert!(anywhere(&harness, "VOLSHAPER"), "the query matched nothing, so this proves nothing");
    assert!(!anywhere(&harness, "DISPERSER"), "the query matched everything, so ditto");
    assert!(!hidden(&harness, "2 entries"), "the bar described a list that is on screen");

    // Emptied. The count is the whole list, which is the number that says
    // whether clearing the filter is worth doing.
    harness.state_mut().set_query("wavesh");
    harness.run();
    assert!(anywhere(&harness, "No entries match"), "the list did not go empty");
    assert!(hidden(&harness, "4 entries"), "the bar says nothing about a list the filter emptied");

    // Singular, over a machine with one entry on it - the only way to reach it,
    // because the note is drawn once the filter has hidden everything and the
    // count is the whole list by then.
    let one = found_with(
        &fixture("local-hidden-one"),
        Helper::Present,
        GuardState::Disarmed,
        one_entry(),
    );
    let mut alone = local_window(one);
    alone.state_mut().set_query("wavesh");
    alone.run();
    assert!(hidden(&alone, "1 entry"), "one hidden entry was not counted as one");
    assert!(!hidden(&alone, "1 entries"));

    // A machine with nothing on it is not a filtered list. The empty state
    // there is the invitation, and a note counting nothing would contradict it.
    let none = found_with(
        &fixture("local-hidden-none"),
        Helper::Present,
        GuardState::Disarmed,
        Manifest::default(),
    );
    let mut empty = local_window(none);
    empty.state_mut().set_query("wavesh");
    empty.run();
    assert!(!hidden(&empty, "0 entries"), "an empty machine was reported as a filtered one");

    // The preparing mode's note goes with the rest of them, which is the part
    // of `:490` that is a decision rather than a reading: the line is the
    // filter's, and what the press costs survives in the tone the summary keeps
    // and in the word on the button.
    let cost = format!("Prepare install {} a backup is written first", crate::widget::SEPARATOR);
    let unprepared = found(&fixture("local-hidden-unprepared"), Helper::Absent, GuardState::Armed);
    let mut unprepared = local_window(unprepared);
    assert!(anywhere(&unprepared, &cost), "the fixture never reached the preparing mode");
    unprepared.state_mut().set_query("wavesh");
    unprepared.run();
    assert!(hidden(&unprepared, "4 entries"));
    assert!(!anywhere(&unprepared, &cost), "the bar drew two notes on a line that holds one");
    assert!(
        anywhere(&unprepared, "4 entries to restore"),
        "the summary stopped counting the work the filter cannot touch"
    );
}

/// And the count is of the rows the list would have drawn, not of the pools it
/// draws them from.
///
/// Three ways those two numbers come apart, and a picture of an empty list
/// holds none of them. A dropped file that is not a document carries no entry,
/// so a filter over names and identities cannot hide it and the list never goes
/// empty. A dropped document that *is* one is a row of its own, so it is one
/// more thing the filter is hiding. And a document dropped over an identity
/// already registered is one row rather than two - the staged one, which is the
/// one that can be acted on.
#[test]
fn the_hidden_count_is_the_rows_the_list_would_have_drawn() {
    let hidden = |harness: &Harness<'static, App>, what: &str| {
        anywhere(harness, &format!("{what} hidden by the current filter"))
    };
    let root = fixture("local-hidden-staged");
    let to = destination(&root);
    let entries = entries();
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries.clone());
    let mut harness = local_window(session);

    // Nothing in the sample carries this, so what is left on screen is left
    // there by something other than the search.
    let nothing = "zzz";

    harness.state_mut().set_staged(dropped(&root, &to, &entries));
    harness.state_mut().set_query(nothing);
    harness.run();
    assert!(
        !anywhere(&harness, "No entries match"),
        "a filter over entries hid a dropped file that is not one"
    );
    // So the bar goes on saying what the press would cost, which is the note
    // the hidden count replaces and the only way to say that no count of any
    // size was drawn: a tree is searched by the whole label, so there is no
    // asking it whether anything ends in `hidden by the current filter`.
    let cost = format!("Update entries {} Bitwig may stay open", crate::widget::SEPARATOR);
    assert!(anywhere(&harness, &cost), "the bar counted a list that is still on screen");

    // The same drop with the rejected file taken out of it: two staged rows
    // over four registered, and the filter is now hiding all six.
    let staged: Vec<Staged> = dropped(&root, &to, &entries)
        .into_iter()
        .filter(|row| row.registration().is_some())
        .collect();
    assert_eq!(staged.len(), 2, "the sample drop no longer stages two documents");
    harness.state_mut().set_staged(staged);
    harness.run();
    assert!(hidden(&harness, "6 entries"), "the staged rows were not counted as hidden");

    // A drop over an entry already on the list. Four rows, not five.
    let registered = entries.entries().first().expect("the sample list is not empty");
    let path = root.join("dropped").join("UPDATE.bwdevice");
    let kind = orng_tools::Kind::from_path(&path).expect("a document extension");
    let document = orng_tools::testing::document(kind, registered.uuid, &registered.name);
    std::fs::write(&path, document.bytes()).expect("could not write the sample");
    harness.state_mut().set_staged(staging::read(&[path], &entries, &to, &[]));
    harness.run();
    assert!(hidden(&harness, "4 entries"), "a drop over a registered entry was counted twice");
}

/// The catalog's search reads four fields where the Local list's reads two.
///
/// Worth its own test because the claim is entirely invisible: a picture of a
/// filtered list is a picture of a shorter list, whatever it was matched on.
/// Each word here appears in exactly one of the two items and in exactly one
/// field of it, so a search that had quietly dropped the description or the
/// keywords would still pass a name-only check.
#[test]
fn the_catalogs_search_reaches_the_description_and_the_keywords() {
    use egui::accesskit::Role;

    let typed = |query: &str| {
        let mut harness = catalog_listing("catalog-search", superseded_entries());
        let field = harness.get_by_role(Role::TextInput);
        field.focus();
        field.type_text(query);
        harness.run();
        anywhere(&harness, "BREATH FOLLOWER")
    };

    // In the first item's description and nowhere else.
    assert!(typed("ducking"), "the description is not searched");
    // A keyword of the first item, and not of the second.
    assert!(typed("envelope"), "the keywords are not searched");
    // The author, which both share, so this says the field is read at all.
    assert!(typed("mono-lab"), "the author is not searched");
    // And a word in neither.
    assert!(!typed("granular"), "something matched a word that is in no item");
}

/// The catalog list with the detail open on one item, against a given machine.
fn detail_on(name: &str, entries: Manifest, open: &str) -> Harness<'static, App> {
    let root = fixture(name);
    let session = copying(&root, entries);
    let mut session = Some(session);
    let open = open.parse().expect("a sample identity");
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
        app.set_catalog(Fetching::frozen(Ok(index)));
        app.show_view(View::Catalog);
        app.set_detailing(open);
        app
    });
    harness.run();
    harness
}

const BREATH_FOLLOWER_II: &str = "d0d0caf0-2222-4333-8444-555566667777";

/// The item the index describes, fetched and registered.
///
/// The fetch is handed in already answered rather than run, because the network
/// is not what is being checked: what is, is everything between the bytes
/// arriving and the entry existing - the registration built from the document
/// and the row, the version and the review recorded off the index, and the row
/// in the other list saying Bitwig has not read it yet.
#[test]
fn installing_registers_the_item_at_the_version_and_review_the_index_names() {
    let mut harness = catalog_listing("installing", superseded_entries());
    let replacement: orng_tools::Uuid =
        "d0d0caf0-2222-4333-8444-555566667777".parse().expect("a sample identity");

    let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
    let item = index
        .items
        .iter()
        .find(|item| item.uuid == replacement)
        .expect("the sample index carries it")
        .clone();
    let document = orng_tools::testing::document(
        item.kind.into(),
        item.uuid,
        "BREATH FOLLOWER II",
    );
    harness.state_mut().set_installing(Install::finished(item, Ok(document)));
    settle(&mut harness);

    // The banner names the item, because the press was about one row.
    assert!(
        harness.query_all_by_label_contains("BREATH FOLLOWER II is registered").next().is_some(),
        "installing said nothing, or said it about the wrong item"
    );
    // And the row it was pressed on now reads as something this machine has.
    assert_eq!(
        harness.query_all_by_label("Available").count(),
        0,
        "the item is registered and its row still offers to install it"
    );

    // What was written, off the list rather than off the screen: the two facts
    // only the index could supply.
    let entries = harness.state().registered().expect("the fixture is an installation").clone();
    let written = entries.get(replacement).expect("the item was not registered");
    assert_eq!(
        written.provenance,
        orng_tools::Provenance::Catalog {
            version: "2.0.0".parse().expect("a version"),
            reviewed_in: orng_tools::Revision::new("3f9a1c2e8b4d7a61c05f2d93ab7e14c8f6021b5d").ok(),
        }
    );
    assert_eq!(
        written.library_path.as_str(),
        "modulators/My Modulators/BREATH FOLLOWER II.bwmodulator",
        "the item was not placed under the name the catalog publishes it as"
    );

    // Bitwig reads the entry list at launch, so the row it just wrote is one an
    // open Bitwig is not showing - and only that row.
    harness.state_mut().show_view(View::Local);
    harness.run();
    assert_eq!(
        harness.query_all_by_label("Pending restart").count(),
        1,
        "installing marked rows it did not write"
    );
}

/// The two ways an install can fail, in one view.
///
/// A picture because what is being claimed is a colour and a pair of controls:
/// both states are set in `--err-text` and only one of them offers to try
/// again, and the design asks for them side by side precisely so that the
/// difference is visible rather than argued. The banner under them is the other
/// half of the trust event - the one thing a row has no room to say.
///
/// Reached by refusing two installs in turn rather than by declaring two
/// failures, because a failure the window was told about is not evidence that
/// the window can arrive at one.
#[test]
fn both_ways_an_install_can_fail() {
    let mut harness = catalog_listing("catalog-failures", entries());
    let index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
    let refuse = |harness: &mut Harness<'static, App>, at: usize, why| {
        harness.state_mut().set_installing(Install::finished(index.items[at].clone(), Err(why)));
        settle(harness);
    };

    refuse(&mut harness, 0, crate::catalog::Refused::Download("the connection closed".to_owned()));
    refuse(
        &mut harness,
        1,
        crate::catalog::Refused::Verification("what arrived is something else".to_owned()),
    );
    // The banner's two lines wrap in what is left of the bar rather than under
    // the control at its end. Asserted through the tree, because a picture of
    // text drawn over a button and a picture of text drawn beside it differ by
    // whichever was painted second - and only one of them can be read.
    let body = harness
        .get_by_label_contains("The file does not match the hash")
        .rect();
    let control = harness
        .query_all_by_label("Copy details")
        .map(|node| node.rect())
        // Two rows wear the words and so does the banner; the banner is the
        // lowest of them.
        .max_by(|a, b| a.top().total_cmp(&b.top()))
        .expect("the banner offered nothing to copy");
    assert!(
        body.right() <= control.left(),
        "the banner's words run under its control: {body:?} against {control:?}"
    );

    look(&mut harness, "catalog-failures");
}

/// Bytes that are not the ones the catalog states are refused, and the refusal
/// is a trust event rather than a network condition.
///
/// Three things have to be true at once and the design says so about each: the
/// row holds the failure, the only control is `Copy details` and never `Retry`,
/// and the banner promises that nothing was written. The last is the one a row
/// cannot say.
#[test]
fn an_item_that_does_not_verify_is_refused_and_offers_no_way_to_try_again() {
    let mut harness = catalog_listing("verification-failed", superseded_entries());
    let item = orng_catalog::Index::parse(SUPERSEDED)
        .expect("the sample index parses")
        .items
        .pop()
        .expect("the sample index is not empty");
    let name = item.name.clone();

    harness.state_mut().set_installing(Install::finished(
        item,
        Err(crate::catalog::Refused::Verification("what arrived is something else".to_owned())),
    ));
    settle(&mut harness);

    assert!(harness.query_by_label("Verification failed").is_some(), "the row says nothing");
    // Twice, and both are meant: the row's own control and the banner's. The
    // words are the same because the press is - there is one set of details.
    assert_eq!(harness.query_all_by_label("Copy details").count(), 2);
    assert!(
        harness.query_by_label("Retry").is_none(),
        "a hash that does not match was offered another go at matching"
    );
    assert!(
        harness
            .query_all_by_label_contains(&format!("{name} was not installed"))
            .next()
            .is_some(),
        "a refused install was not said out loud"
    );
    assert!(
        harness.state().registered().expect("an installation").get(
            "d0d0caf0-2222-4333-8444-555566667777".parse().expect("a sample identity")
        ).is_none(),
        "something was registered for an item that did not verify"
    );
}

/// An index that cannot say where its documents are refuses the press, and
/// refuses it as the ordinary failure rather than as the trust one.
///
/// The one end-to-end run of the real worker that needs no network: an index
/// generated inside a pull request carries no revision, there is no reviewed
/// commit to read the bytes from, and the answer comes back without a socket
/// being opened. It is also the assertion that the press reaches the worker at
/// all, which is the half `Install::finished` steps over.
#[test]
fn an_index_that_names_no_commit_has_nowhere_to_fetch_from_and_says_so() {
    let root = fixture("no-revision");
    let session = copying(&root, superseded_entries());
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        let mut index = orng_catalog::Index::parse(SUPERSEDED).expect("the sample index parses");
        index.revision = None;
        app.set_catalog(Fetching::frozen(Ok(index)));
        app.show_view(View::Catalog);
        app
    });
    harness.run();

    harness.get_by_label("Install").click();
    settle(&mut harness);

    assert!(
        harness.query_by_label("Download failed").is_some(),
        "an item with nowhere to be fetched from said nothing"
    );
    assert!(
        harness.query_by_label("Retry").is_some(),
        "an ordinary failure was not offered another go"
    );
    assert!(
        harness.query_by_label("Verification failed").is_none(),
        "nothing was fetched and the row accused the catalog of serving the wrong bytes"
    );
}

/// A row written into a live installation reads `Pending restart` until the
/// list is read off the machine again.
///
/// Driven through the run rather than by setting the word: what has to hold is
/// that the identities an [`orng_tools::Update`] wrote survive the worker and
/// reach the row, and every step of that is between the press and the word.
#[test]
fn a_row_written_while_bitwig_is_open_waits_for_a_restart() {
    let mut harness = listing("pending-restart");
    let written = harness.state().registered().expect("the fixture is an installation").clone();
    let volshaper: orng_tools::Uuid = VOLSHAPER.parse().expect("a sample identity");

    let mut applying = Applying::frozen(None, Stage::Registering, Some(Ok(written)));
    applying.writing.insert(volshaper);
    harness.state_mut().set_applying(applying);
    harness.run();

    // And only that one: the other three were not touched by the run, and a
    // word on them would say Bitwig is missing a change nobody made.
    assert_eq!(
        harness.query_all_by_label("Pending restart").count(),
        1,
        "rows the run never wrote were marked as waiting on a restart"
    );
}

/// The two states a picture is worth having of, because what is being claimed
/// is a colour: the design puts `Changed` in `--accent-text` and `Missing file`
/// in `--err-text`, and reading one as the other is reading "decide something"
/// as "something is broken".
#[test]
fn documents_that_are_missing_or_changed() {
    let name = "statuses-shot";
    let session = damaged(&fixture(name));
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    look(&mut harness, "statuses");
}

/// The row's own controls: hidden until the pointer arrives, and then laid out
/// as `EntryRow.dc.html:44-60` lays them out.
///
/// **Asserted through the tree and not through a picture**, because neither
/// half of this claim is something a picture holds. A snapshot of a row with no
/// pointer on it cannot say whether the controls are hidden or absent - the
/// design's whole point is that those look identical - and a rectangle is what
/// says a 22-pixel square is 22 pixels rather than what the eye settles for.
#[test]
fn a_row_reveals_its_controls_under_the_pointer_and_sizes_them_as_the_bundle_does() {
    let mut harness = listing("row-action-sizes");

    assert!(
        harness.query_by_label_contains("Remove entry").is_none(),
        "a row offered its controls with the pointer nowhere near it"
    );

    // On the name, which is inside the row and is what anybody aims at on the
    // way to the controls at its other end.
    harness.get_by_label("DISPERSER").hover();
    harness.run();

    let name = harness.get_by_label("DISPERSER").rect();
    let reveal = harness.get_by_label("Reveal file").rect();
    let remove = harness.get_by_label_contains("Remove entry").rect();

    // The bundle's own numbers and not this application's transcription of
    // them: `width:22px; height:22px` and `gap:1px` on `EntryRow.dc.html:44-58`.
    // Written against `metric::ROW_ACTION` these passed with that constant set
    // to 24, which is the whole difference between checking the design and
    // checking the arithmetic.
    for (what, control) in [("Reveal file", reveal), ("Remove entry", remove)] {
        assert_eq!(control.width(), 22.0, "{what} is not the design's width");
        assert_eq!(control.height(), 22.0, "{what} is not the design's height");
    }
    assert_eq!(
        remove.left() - reveal.right(),
        1.0,
        "the design's one-pixel seam is not between them"
    );
    // Right-aligned against the row's own twelve of padding, which is where the
    // reserved column ends: `an_entry_row_is_divided_as_the_bundle_divides_it`
    // puts that edge at 808 in a window the design's width. Four entries in a
    // 560-tall window need no scrollbar, so the list is the whole width.
    assert_eq!(remove.right(), 808.0);
    // And centred down the row rather than sitting on its top edge.
    assert_eq!(remove.center().y, name.center().y);
}

/// What each state offers, through the window rather than through the table.
///
/// `status::every_state_offers_what_the_bundle_offers` already asserts the
/// table. This asserts that the table is what the list draws from - that a
/// registered row really does get those two controls and a staged one really
/// does get the other pair, rather than the table being right and unconsulted.
#[test]
fn the_controls_a_row_offers_are_the_ones_its_state_offers() {
    let root = fixture("row-actions-by-state");
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);
    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_staged(staged.take().expect("built once"));
        app
    });
    harness.run();

    // A staged row: an identity to mint and a press that cancels it. Nothing to
    // reveal, because nothing of ours has been placed for it yet.
    harness.get_by_label("WAVESHAPER ALPHA").hover();
    harness.run();
    assert!(harness.query_by_label("Assign new UUID").is_some(), "a staged row cannot be minted");
    assert!(harness.query_by_label("Cancel").is_some(), "a staged row cannot be cancelled");
    assert!(
        harness.query_by_label("Reveal file").is_none(),
        "a staged row offered to reveal a document that has not been placed"
    );

    // A rejected row: nothing was read, so there is nothing to act on.
    harness.get_by_label("BROKEN.bwmodule").hover();
    harness.run();
    assert!(
        harness.query_by_label_contains("Remove entry").is_none()
            && harness.query_by_label("Cancel").is_none(),
        "a rejected row offered a control"
    );
}

/// Removing a registered entry queues it and does not write anything.
///
/// The design keeps the row registered and strikes it through until the apply
/// that takes it away, which is what makes one press of the primary action the
/// confirmation for every removal in the list.
#[test]
fn removing_a_registered_entry_queues_it_and_the_press_counts_it() {
    let mut harness = listing("queue-a-removal");

    harness.get_by_label("DISPERSER").hover();
    harness.run();
    harness.get_by_label_contains("Remove entry").click();
    harness.run();

    assert!(
        harness.query_by_label("Pending removal").is_some(),
        "the row was not queued for removal"
    );
    assert!(
        harness.query_by_label("DISPERSER").is_some(),
        "the row went away before anything was applied"
    );
    // And the press landed on the control and stopped there. The whole row is
    // the control that opens the inspector, so every one of these sits on top
    // of something that would otherwise answer the same click.
    assert!(
        harness.query_by_label(crate::widget::icon::DISMISS).is_none(),
        "pressing a row's own control also opened the inspector"
    );
    // A queued removal is work, and the primary action has to say so: counting
    // only the additions leaves it disabled beside a list of struck-through
    // rows, claiming there is nothing to apply.
    assert!(
        harness.query_by_label_contains("Apply 1 change").is_some(),
        "the primary action does not count the removal"
    );
    // Counted rather than fetched: `centred_block` lays its contents out twice,
    // once into an invisible sizing Ui to learn the height and once for real,
    // and both passes reach the accessibility tree - so every action-bar
    // summary is two nodes saying the same thing.
    assert!(
        harness.query_all_by_label("1 to remove").next().is_some(),
        "the action bar does not say what the press would do"
    );

    // And the undo, which is the only control the design leaves on a row that
    // is already queued.
    harness.get_by_label("Undo removal").click();
    harness.run();
    assert!(
        harness.query_by_label("Pending removal").is_none(),
        "the removal could not be taken back"
    );
    assert!(
        harness.query_by_label_contains("Apply 1 change").is_none(),
        "the press still counts a removal that was undone"
    );
}

/// Re-dropping the document of a queued entry takes the removal back off the
/// press.
///
/// The row is the staged one then, not the registered one - a drop replaces
/// every registered row it covers - so the list says `Staged` and says nothing
/// about a removal. A press that forgot the entry anyway would be doing
/// something no row on screen said it would, and would hand `Update` an add and
/// a remove for one identity in the same change.
#[test]
fn re_dropping_a_queued_entrys_document_takes_the_removal_back() {
    let root = fixture("queued-then-dropped");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let to = destination(&root);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    harness.get_by_label("DISPERSER").hover();
    harness.run();
    harness.get_by_label_contains("Remove entry").click();
    harness.run();
    assert!(harness.query_all_by_label("1 to remove").next().is_some());

    // The same identity the sample list carries for DISPERSER, dropped again.
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");
    let path = drop.join("DISPERSER.bwdevice");
    let document = orng_tools::testing::document(
        orng_tools::Kind::Device,
        "80c0dc4c-d142-53a7-85ee-b91427819b66".parse().expect("a sample identity"),
        "DISPERSER",
    );
    std::fs::write(&path, document.bytes()).expect("could not write the sample");
    let staged = staging::read(&[path], &entries(), &to, &[]);
    harness.state_mut().set_staged(staged);
    harness.run();

    assert!(
        harness.query_by_label("Pending removal").is_none(),
        "the list shows a removal queued against a row it is no longer drawing"
    );
    assert!(
        harness.query_all_by_label("1 to remove").next().is_none(),
        "the press would forget an entry the list is showing as staged"
    );
    assert!(harness.query_all_by_label("1 to add").next().is_some(), "the drop was not staged");
}

/// Cancelling a staged row takes it out of the pending list - and settles the
/// row it was colliding with.
///
/// The second half is the point. A conflict is a statement about the whole set,
/// so a row still saying it collides with a document that has been cancelled is
/// a claim about something that is no longer there.
#[test]
fn cancelling_a_staged_row_settles_the_row_it_collided_with() {
    let root = fixture("cancel-a-staged-row");
    let to = destination(&root);
    let entries = Manifest::default();
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");

    // Two files claiming one identity. Without the cancel the second is a
    // conflict, and it is a conflict only because the first is there.
    let shared = "1f6c85d4-9a02-47be-83c1-d5e70b14a629";
    let mut paths = Vec::new();
    for display in ["FIRST", "SECOND"] {
        let path = drop.join(format!("{display}.bwdevice"));
        let document =
            orng_tools::testing::document(orng_tools::Kind::Device, shared.parse().unwrap(), display);
        std::fs::write(&path, document.bytes()).expect("could not write the sample");
        paths.push(path);
    }
    let staged = staging::read(&paths, &entries, &to, &[]);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);

    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_staged(staged.take().expect("built once"));
        app
    });
    harness.run();
    assert!(harness.query_by_label("Conflict").is_some(), "the two identities did not collide");

    harness.get_by_label("FIRST").hover();
    harness.run();
    harness.get_by_label("Cancel").click();
    harness.run();

    assert!(harness.query_by_label("FIRST").is_none(), "the cancelled row is still listed");
    assert!(
        harness.query_by_label("Conflict").is_none(),
        "the row that collided with the cancelled one still says it collides"
    );
    assert!(harness.query_by_label("SECOND").is_some(), "the wrong row was cancelled");
}

/// Minting a new identity settles the collision a new identity can settle.
///
/// The one repair the design offers from the list itself, and the row it is
/// pressed on is not the only one it changes: the other document claiming that
/// identity stops colliding at the same moment.
#[test]
fn assigning_a_new_uuid_settles_two_documents_claiming_one_identity() {
    let root = fixture("assign-a-new-uuid");
    let to = destination(&root);
    let entries = Manifest::default();
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");

    let shared = "1f6c85d4-9a02-47be-83c1-d5e70b14a629";
    let mut paths = Vec::new();
    for display in ["FIRST", "SECOND"] {
        let path = drop.join(format!("{display}.bwdevice"));
        let document =
            orng_tools::testing::document(orng_tools::Kind::Device, shared.parse().unwrap(), display);
        std::fs::write(&path, document.bytes()).expect("could not write the sample");
        paths.push(path);
    }
    let staged = staging::read(&paths, &entries, &to, &[]);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);

    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.set_staged(staged.take().expect("built once"));
        app
    });
    harness.run();
    assert!(harness.query_by_label("Conflict").is_some(), "the two identities did not collide");

    harness.get_by_label("SECOND").hover();
    harness.run();
    harness.get_by_label("Assign new UUID").click();
    harness.run();

    assert!(
        harness.query_by_label("Conflict").is_none(),
        "a new identity did not settle a collision of identities"
    );
    assert!(
        harness.query_by_label_contains("Apply 2 changes").is_some(),
        "both rows should be ready to write now"
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

/// The Restore screen on a machine that has never prepared anything, which is
/// every machine before its first press of the primary action.
///
/// **The only picture of this screen there is**, and deliberately: every row of
/// the populated state carries a date and a size read off the disk, and a date is
/// a function of the reader's zone. The state with copies in it is asserted below
/// instead.
#[test]
fn the_restore_screen_with_nothing_kept() {
    let root = fixture("restore-empty");
    let session = found(&root, Helper::Absent, GuardState::Armed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.show_restore();
        app
    });
    look(&mut harness, "restore-empty");
}

/// And the same screen with two copies on disk.
///
/// No picture: the rows say when each copy was taken, and an instant has no day
/// until a zone is chosen - so a snapshot of this would be a snapshot of the
/// machine that took it, differing by a day west of Denver. What a picture would
/// have vouched for is asserted here instead, through the labels a user reads.
///
/// The build each copy is of comes back out of its own directory name, which is
/// the only record there is of it - so a row that could not name its build would
/// be a row this screen cannot draw.
#[test]
fn the_restore_screen_lists_what_is_kept() {
    let root = fixture("restore-listed");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let backups = root.join("home/.orng/backups");
    for build in ["6.1-94a90411", "6.0-a1d34f07"] {
        let dir = backups.join(build);
        std::fs::create_dir_all(&dir).expect("a place to keep a backup");
        std::fs::write(dir.join("bitwig.jar"), b"not an archive, and not read here")
            .expect("could not write the copy");
    }

    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.show_restore();
        app
    });
    harness.run();

    assert!(
        harness.query_by_label_contains("Available backups").is_some(),
        "the list was not drawn"
    );
    // Both copies, each naming the build it came from. Ordered by when they were
    // written and not by version, so the assertion is on presence and on which
    // one carries the mark.
    for build in ["6.1 (94a90411)", "6.0 (a1d34f07)"] {
        assert!(
            harness.query_by_label_contains(build).is_some(),
            "no row names {build} - a backup that cannot say which build it is of"
        );
    }
    assert!(harness.query_by_label("Latest").is_some(), "nothing is marked as the newest");
    // Where a row's words start, which no picture of this screen will ever
    // hold: the list's own twelve, the row's twelve, the sixteen-pixel mark and
    // the design's gap after it. The bundle puts the first character at 50.
    let words = harness
        .get_all_by_label_contains("jar + description bundles")
        .map(|node| node.rect().left())
        .fold(f32::INFINITY, f32::min);
    assert!(
        (words - 50.0).abs() < 1.0,
        "a row's words start at {words} where the bundle starts them at 50"
    );
    assert!(
        harness.query_by_label_contains("Restoring removes every registration").is_some(),
        "the warning above the list is not drawn"
    );
    // And the foot names the copy the press would put back, which is the one
    // thing on this screen that changes when a row is chosen.
    assert!(
        harness.query_by_label_contains("wholesale. Bitwig Studio must be closed").is_some(),
        "the foot does not say what pressing would do"
    );
}

/// The About screen.
///
/// The identity line is supplied rather than read, for the reason `App::show_about`
/// records: the real one ends in the name of the machine that drew it.
#[test]
fn the_about_screen() {
    let root = fixture("about");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.show_about("0.9.2 \u{b7} orng-registry \u{b7} macOS arm64");
        app
    });
    look(&mut harness, "about");
}

/// The overflow reaches all three screens, and each one comes back.
///
/// The class of fault a picture structurally cannot catch, and the one this
/// application has already had twice. Three screens that only their `show_`
/// helper could reach would be three pictures vouching for a menu that opens
/// nothing.
///
/// The swap is asserted through the install bar rather than through a rectangle:
/// a screen replaces that bar, so the build revision beside the installation's
/// name is on screen while the list is and gone while a screen is.
#[test]
fn the_overflow_reaches_every_screen_and_each_one_goes_back() {
    let root = fixture("screens-opening");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        App::with(&cc.egui_ctx, session.take().expect("built once"))
    });
    harness.run();

    // The install bar is named by its other view tab, and not by the build
    // revision beside the installation's name: About states that same revision,
    // so the revision no longer says which of the two is on screen.
    let install_bar = "Catalog";
    // Each item, and a line only that screen draws. Restore is named by the line
    // at its foot rather than by its empty state: an empty state is drawn twice,
    // once in the sizing pass that centres it, and a query for a label in it
    // finds both.
    let screens = [
        ("Settings", "Document placement"),
        ("Restore backup...", "No backup exists for this installation"),
        ("About ORNG Registry", "Detected installation"),
    ];
    for (item, drawn) in screens {
        assert!(
            harness.query_by_label(install_bar).is_some(),
            "{item}: the list is not on screen"
        );
        harness.get_by_label(crate::widget::icon::OVERFLOW).click();
        harness.run();
        harness.get_by_label_contains(item).click();
        harness.run();

        assert!(
            harness.query_by_label_contains(drawn).is_some(),
            "{item} did not open the screen behind it"
        );
        assert!(
            harness.query_by_label(install_bar).is_none(),
            "{item} is open and the install bar is still drawn - it is a panel, not a screen"
        );

        // And the way out is labelled with the view it goes back to.
        harness.get_by_label_contains("Local").click();
        harness.run();
    }
    assert!(
        harness.query_by_label(install_bar).is_some(),
        "the last screen did not go back"
    );
}

/// Pressing a row changes which archive would be copied over the installation.
///
/// Not a picture: a screen whose rows moved nothing would render identically but
/// for one glyph. Reached by the build each row names rather than by its date,
/// which is the one thing a test here must not know.
///
/// Which row starts out chosen is *not* asserted. The list is ordered by when
/// each copy was written, and two directories written in the same instant can be
/// ordered either way - so what is asserted is that pressing the other one moves
/// the answer, which is the whole of what the press has to do.
#[test]
fn choosing_a_backup_changes_which_one_would_be_put_back() {
    let root = fixture("restore-choosing");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let backups = root.join("home/.orng/backups");
    for build in ["6.1-94a90411", "6.0-a1d34f07"] {
        let dir = backups.join(build);
        std::fs::create_dir_all(&dir).expect("a place to keep a backup");
        std::fs::write(dir.join("bitwig.jar"), b"not an archive").expect("could not write");
    }

    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session.take().expect("built once"));
        app.show_restore();
        app
    });
    harness.run();

    // The other row, found by the build it names rather than by its date - the
    // date is what this test must not know.
    let builds = ["6.1 (94a90411)", "6.0 (a1d34f07)"];
    let opened = harness.state().pointed_at().expect("a copy is pointed at").to_owned();
    let other = builds
        .into_iter()
        .find(|build| !opened.contains(build))
        .expect("the row that is not the one already chosen");

    harness.get_by_label_contains(other).click();
    harness.run();
    let now = harness.state().pointed_at().expect("a copy is still pointed at");
    assert!(
        now.contains(other),
        "pressing the other row did not move which archive would be copied over the installation"
    );
    assert_ne!(now, opened, "the choice did not move at all");
}

/// An index that did not verify. Nothing is listed, and the reason is shown.
#[test]
fn a_catalog_that_does_not_verify() {
    shot_catalog(
        "catalog-refused",
        Fetching::frozen(Err("the signature does not match this index under this key".to_owned())),
    );
}



