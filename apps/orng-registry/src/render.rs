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
use egui_kittest::Harness;
use orng_tools::{
    Condition, Destination, GuardState, Helper, Manifest, OrngHome, RunState, Strategy,
    UserLibrary,
};

use crate::app::{App, View};
use crate::session::{Found, Session};
use crate::catalog::Fetching;
use crate::staging::{self, Staged};
use crate::work::{Applying, Stage, State};

/// The window's own size, so what is rendered is what would be seen.
const SIZE: egui::Vec2 = egui::vec2(1040.0, 680.0);

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
fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new("target/render-fixtures").join(name);
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// The installation inside a fixture, named as an installation is named: the
/// path is drawn, and a fixture that does not look like one teaches the reader
/// to expect something else.
fn install_root(fixture: &std::path::Path) -> std::path::PathBuf {
    fixture.join("Bitwig Studio.app")
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
        home: OrngHome::at(fixture),
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
    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut applying = Some(applying);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app.get_or_insert_with(|| {
            let mut app = App::with(ui.ctx(), session.take().expect("built once"));
            app.set_applying(applying.take().expect("built once"));
            app
        });
        app.draw(ui);
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

    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut staged = Some(staged);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app.get_or_insert_with(|| {
            let mut app = App::with(ui.ctx(), session.take().expect("built once"));
            app.set_staged(staged.take().expect("built once"));
            app
        });
        app.draw(ui);
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

    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app.get_or_insert_with(|| App::with(ui.ctx(), session.take().expect("built once")));
        app.draw(ui);
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
    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut fetching = Some(fetching);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app.get_or_insert_with(|| {
            let mut app = App::with(ui.ctx(), session.take().expect("built once"));
            app.set_catalog(fetching.take().expect("built once"));
            app.show_view(View::Catalog);
            app
        });
        app.draw(ui);
    });
    look(&mut harness, name);
}

/// Render one state and write it out under `name`.
fn shot(name: &str, session: Session, view: View, dark: bool) {
    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app
            .get_or_insert_with(|| App::with(ui.ctx(), session.take().expect("built once")));
        app.set_theme(dark, ui.ctx());
        app.show_view(view);
        app.draw(ui);
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
    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build_ui(move |ui| {
        let app = app.get_or_insert_with(|| {
            let mut app = App::with(ui.ctx(), session.take().expect("built once"));
            app.set_query("wavesh");
            app
        });
        app.draw(ui);
    });
    look(&mut harness, "no-match");
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

/// The catalog as it is published today, drawn from a real index rather than a
/// made-up one, so what is rendered is a shape the catalog actually produces.
#[test]
fn the_catalog_view() {
    let index = orng_catalog::Index::parse(include_str!("../tests/published-index.json"))
        .expect("the sample index does not parse");
    shot_catalog("catalog", Fetching::frozen(Ok(index)));
}

/// An index that did not verify. Nothing is listed, and the reason is shown.
#[test]
fn a_catalog_that_does_not_verify() {
    shot_catalog(
        "catalog-refused",
        Fetching::frozen(Err("the signature does not match this index under this key".to_owned())),
    );
}


