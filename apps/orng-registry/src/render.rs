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
use crate::catalog::{Catalog, Install};
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
        let document = orng_tools::testing::document(entry.kind(), entry.uuid, &entry.name);
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

/// A window over a given machine, arranged into the state under test and laid
/// out once.
///
/// **The one place a harness is built**, so the window's size, the context the
/// application is handed and the first pass are decided once rather than at
/// every call site. What varies between states is what is set on the `App`
/// before it draws, which is what `arrange` is for; the context comes with it
/// because a few of those setters need it.
///
/// `build_eframe` takes its closure by value, so whatever the state needs moves
/// straight into it. The call sites used to wrap each of those in an `Option`
/// and `take` it back out with an `expect("built once")` - a runtime guard for
/// something `FnOnce` already promises, repeated at every site and once per
/// captured value.
fn window(
    session: Session,
    arrange: impl FnOnce(&mut App, &egui::Context),
) -> Harness<'static, App> {
    let mut harness = Harness::builder().with_size(SIZE).build_eframe(move |cc| {
        let mut app = App::with(&cc.egui_ctx, session);
        arrange(&mut app, &cc.egui_ctx);
        app
    });
    harness.run();
    harness
}

/// Render one state, with work held still, and write it out.
fn shot_applying(name: &str, applying: Applying) {
    let root = fixture(name);
    let session = found(&root, Helper::Absent, GuardState::Armed);
    let mut harness = window(session, |app, _| app.set_applying(applying));
    look(&mut harness, name);
}

/// Render the list with documents dropped on it and not yet written.
fn shot_staged(name: &str, helper: Helper, guard: GuardState) {
    let root = fixture(name);
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, helper, guard, entries);

    let mut harness = window(session, |app, _| app.set_staged(staged));
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

    let mut harness = local_window(session);
    harness.input_mut().hovered_files = over
        .iter()
        .map(|file| egui::HoveredFile { path: Some(drop.join(file)), ..Default::default() })
        .collect();
    look_while_dragging(&mut harness, name);
}

/// The catalog view with the catalog held still, on an installation with
/// nothing registered.
fn catalog_with(name: &str, catalog: Catalog) -> Harness<'static, App> {
    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    window(session, |app, _| {
        app.set_catalog(catalog);
        app.show_view(View::Catalog);
    })
}

/// Render the catalog view with the catalog held still.
fn shot_catalog(name: &str, catalog: Catalog) {
    let mut harness = catalog_with(name, catalog);
    look(&mut harness, name);
}

/// Render one state and write it out under `name`.
fn shot(name: &str, session: Session, view: View, dark: bool) {
    let mut harness = window(session, |app, ctx| {
        app.set_appearance(appearance(dark), ctx);
        app.show_view(view);
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

/// Settings on a machine with no installation, the state somebody is most
/// likely in when they open it: every path row kept at its height with a dash
/// in the quietest ink, and the report saying where it looked.
#[test]
fn settings_with_no_installation() {
    let session = Session::NoInstallation { searched: "/Applications, ~/Applications".to_owned() };
    let mut harness = window(session, |app, _| app.show_settings());
    look(&mut harness, "settings-no-installation");
}

/// The note under the steps promises that nothing has changed only for as long
/// as that is true: from Activate on, undoing it is a restore.
#[test]
fn the_progress_note_changes_its_promise_at_activate() {
    use orng_tools::Step;
    let note_at = |activate: State| {
        let root = fixture("progress-note");
        let session = found(&root, Helper::Absent, GuardState::Armed);
        let steps = [
            (Step::Backup, State::Done),
            (Step::Patch, State::Done),
            (Step::Verify, if activate == State::Waiting { State::Running } else { State::Done }),
            (Step::Activate, activate),
            (Step::Link, State::Waiting),
        ];
        let applying = Applying::frozen(Some(steps), Stage::Preparing, None);
        let mut harness = window(session, |app, _| app.set_applying(applying));
        harness.run();
        let before = "Nothing in the installation changes until the patched archive verifies.";
        let after = "Once the preparation completes, undoing it means Restore.";
        (anywhere(&harness, before), anywhere(&harness, after))
    };
    assert_eq!(note_at(State::Waiting), (true, false), "before Activate");
    assert_eq!(note_at(State::Running), (false, true), "during Activate");
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
            Some(Err("the patched archive did not load under the bundled JVM".to_owned().into())),
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
    let mut harness = window(session, |app, _| app.set_query("wavesh"));
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
    let mut harness = window(session, |app, _| {
        app.set_catalog(Catalog::just_fetched(superseded()));
        app.show_view(View::Catalog);
        app.set_query("granular");
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

    let mut harness = window(session, |app, _| {
        app.set_staged(staged);
        app.set_applying(Applying::frozen(None, Stage::Registering, Some(Ok(entries))));
    });
    look(&mut harness, "registered");
}

/// The everyday state: pending work pinned above what is registered, with each
/// of the three answers a drop can get.
#[test]
fn documents_dropped_on_a_prepared_installation() {
    shot_staged("staged", Helper::Present, GuardState::Disarmed);
}

/// A document dropped while a run is going does not join the list it is
/// running against.
///
/// A press builds its job out of the staged rows as they stand, and its own
/// answer clears that list. A row that joined between the two is one nothing
/// wrote and nothing kept: it would be counted in `N entries registered` and
/// then thrown away, and the file would have to be found again to notice. So
/// the drop is refused while work is in flight, which is the rule
/// `App::write_words` states and every other press already follows.
///
/// **The second half is what makes the first mean anything.** The same drop on
/// the same window with nothing running does reach the list, so what the first
/// half proves is the guard rather than a drop that never arrived.
///
/// There are two guards, and either alone keeps this green: `App::read` starts
/// no read under a run, and `App::pump` takes no read while one is going. The
/// two tests below pin one each.
#[test]
fn a_document_dropped_while_a_run_is_going_is_refused() {
    assert!(
        !dropped_while("drop-mid-run", Meanwhile::Running),
        "a document dropped during a run joined the list the run was built from"
    );
    assert!(
        dropped_while("drop-when-idle", Meanwhile::Nothing),
        "the drop this test relies on never reached the window"
    );
}

/// And it is refused, not put off: the run ending does not let it in.
///
/// Pins `App::read`'s guard. Without it the read starts under the run, is held
/// by `App::pump` until the run is over, and lands the moment it ends - built
/// against a list the run has since cleared, and displacing any read `pump` was
/// already holding.
#[test]
fn a_document_dropped_while_a_run_is_going_is_not_taken_when_it_ends() {
    assert!(
        !dropped_while("drop-then-finish", Meanwhile::Ran),
        "a document dropped during a run was read anyway and joined the list after it"
    );
}

/// A document still being read when a run starts does not join it.
///
/// Pins `App::pump`'s guard, which is the one that covers this: `App::read`
/// had nothing to refuse, because nothing was running when the drop landed. The
/// press takes the staged rows as they stand, so rows folded in under it would be
/// counted as registered and then cleared.
#[test]
fn a_document_still_being_read_when_a_run_starts_stays_out_of_it() {
    assert!(
        !dropped_while("drop-then-press", Meanwhile::Pressed),
        "a read in flight when a run started joined the list the run was built from"
    );
}

/// What the window is doing when a document is dropped on it.
#[derive(Clone, Copy)]
enum Meanwhile {
    Nothing,
    /// A run in flight that never finishes.
    Running,
    /// A run in flight when the drop lands, which finishes the frame after.
    Ran,
    /// A run that starts the frame after the drop lands, while the document is
    /// still being read, and never finishes.
    Pressed,
}

/// A dropped file as the window's own input carries one.
///
/// egui takes these behind a trait because the integration owns the file
/// handle - on the web a drop is a browser object with no path at all. The
/// window reads the path and nothing else, so that is all this answers, and
/// `bytes` reads the file the same way the real one does.
#[derive(Debug)]
struct Dropped(std::path::PathBuf);

impl egui::DroppedFile for Dropped {
    fn path(&self) -> &std::path::Path {
        &self.0
    }

    fn bytes(&self) -> Result<Vec<u8>, String> {
        std::fs::read(&self.0).map_err(|why| why.to_string())
    }
}

/// Drop one document on a window, with or without a run already in flight, and
/// answer whether it reached the list.
fn dropped_while(name: &str, meanwhile: Meanwhile) -> bool {
    const DROPPED: &str = "WAVESHAPER ALPHA";

    let root = fixture(name);
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");
    let path = drop.join(format!("{DROPPED}.bwdevice"));
    let document = orng_tools::testing::document(
        orng_tools::Kind::Device,
        "1f6c85d4-9a02-47be-83c1-d5e70b14a629".parse().expect("a sample identity"),
        DROPPED,
    );
    std::fs::write(&path, document.bytes()).expect("could not write the sample");

    // Held still and never finishing: what matters is only that the window has
    // work in flight. No steps, so no progress dialog is drawn over the list.
    let running = || Applying::frozen(None, Stage::Preparing, None);
    let mut harness = window(session, move |app, _| {
        if let Meanwhile::Running | Meanwhile::Ran = meanwhile {
            app.set_applying(running());
        }
    });
    harness.input_mut().dropped_files = vec![std::sync::Arc::new(Dropped(path))];
    // One frame and not `run`, which draws until nothing asks for another - and
    // the reader asks when it is done, so `Pressed` would find the read already
    // taken. The drop is read in this frame, after the read it could have taken.
    harness.step();
    // Once. A drop is an event and not a state, and redelivering it every frame
    // would start a fresh read each time and never let one land.
    harness.input_mut().dropped_files.clear();
    match meanwhile {
        Meanwhile::Nothing | Meanwhile::Running => {}
        // Reporting the list it was handed, which is all a run that wrote
        // nothing would have to say.
        Meanwhile::Ran => {
            let written = harness.state().registered().expect("an installation").clone();
            let finished = Applying::frozen(None, Stage::Registering, Some(Ok(written)));
            harness.state_mut().set_applying(finished);
        }
        Meanwhile::Pressed => harness.state_mut().set_applying(running()),
    }

    // The read is on a worker in every case, so both get the same chances. A
    // bound rather than an interval, as `settle` is: the loop draws first and
    // looks after.
    for _ in 0..200 {
        harness.run();
        if harness.query_by_label(DROPPED).is_some() {
            return true;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    false
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

/// And there is no overlay while a run is going, because a drop then is
/// refused: one saying `Drop to stage 1 document` invited a drop the window
/// went on to ignore without a word.
#[test]
fn the_drop_overlay_is_not_drawn_while_a_run_is_going() {
    let root = fixture("dragging-mid-run");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let lone = root.join("BREATH.bwmodulator");
    std::fs::write(&lone, b"the drag does not read it").expect("could not write");

    let mut harness = local_window(session);
    let invites = |harness: &mut Harness<'_, App>| {
        harness.input_mut().hovered_files =
            vec![egui::HoveredFile { path: Some(lone.clone()), ..Default::default() }];
        harness.run_steps(SETTLING_PASSES);
        harness.query_all_by_label("Drop to stage 1 document").next().is_some()
    };

    assert!(invites(&mut harness), "the drag this test relies on drew no overlay");
    harness.state_mut().set_applying(Applying::frozen(None, Stage::Registering, None));
    assert!(!invites(&mut harness), "the overlay invited a drop a run in flight refuses");
}

/// The overlay says what is over the window now, not what was over it first.
///
/// What it says is kept for as long as the same paths are held there, because
/// working it out lists every hovered folder and a drag redraws every frame. So
/// this carries one set, then a different one, then nothing, then the first set
/// again after its folder has changed. The folder is also the one case the two
/// pictures above do not draw: its documents counted in the heading and on its
/// own row.
#[test]
fn the_drop_overlay_follows_what_is_held_over_the_window() {
    let root = fixture("dragging-again");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let drop = root.join("dragged");
    let folder = drop.join("My Devices");
    std::fs::create_dir_all(&folder).expect("a place to drag from");
    for file in ["WAVESHAPER ALPHA.bwdevice", "SLEW LIMITER.bwmodule", "notes.txt"] {
        std::fs::write(folder.join(file), b"the drag does not read it").expect("could not write");
    }
    let lone = drop.join("BREATH.bwmodulator");
    std::fs::write(&lone, b"the drag does not read it").expect("could not write");

    let mut harness = local_window(session);
    let mut hold = |paths: &[&std::path::Path]| {
        harness.input_mut().hovered_files = paths
            .iter()
            .map(|path| egui::HoveredFile { path: Some(path.to_path_buf()), ..Default::default() })
            .collect();
        harness.run_steps(SETTLING_PASSES);
        let says = |heading: &str| harness.query_all_by_label(heading).next().is_some();
        [says("Drop to stage 1 document"), says("Drop to stage 3 documents")]
    };

    assert_eq!(hold(&[&lone]), [true, false], "one document was not stated as one");
    assert_eq!(
        hold(&[&folder, &lone]),
        [false, true],
        "the overlay kept what it said about the drag before"
    );
    assert_eq!(hold(&[]), [false, false], "the overlay outlived the drag");

    std::fs::remove_file(folder.join("SLEW LIMITER.bwmodule")).expect("could not remove");
    std::fs::remove_file(folder.join("WAVESHAPER ALPHA.bwdevice")).expect("could not remove");
    assert_eq!(
        hold(&[&folder, &lone]),
        [true, false],
        "a new drag of the same paths was stated from the last one"
    );
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
    let harness = local_window(session);

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
    let harness = local_window(session);

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
    shot_catalog("catalog", Catalog::just_fetched(index));
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
    let index = superseded();
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

/// The index above, parsed. What [`entries`] is to [`ROWS`].
fn superseded() -> orng_catalog::Index {
    orng_catalog::Index::parse(SUPERSEDED).expect("the sample index does not parse")
}

/// The second of its two items - `BREATH FOLLOWER II`, the replacement - which
/// is the one every install in these tests is refused or verified against.
fn superseded_replacement() -> orng_catalog::IndexEntry {
    superseded().items.pop().expect("the sample index is not empty")
}

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
    let mut harness = window(session, |app, _| {
        app.set_catalog(Catalog::just_fetched(superseded()));
        app.show_view(View::Catalog);
    });
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
    let open = open.parse().expect("a sample identity");
    let mut harness = window(session, |app, _| {
        app.set_catalog(Catalog::just_fetched(index));
        app.show_view(View::Catalog);
        app.set_detailing(open);
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
    let mut harness = local_window(session);
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
    let mut harness = window(session, |app, ctx| {
        app.set_appearance(appearance(dark), ctx);
        app.set_inspecting(VOLSHAPER.parse().expect("a sample identity"));
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
    let mut harness = local_window(session);

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

/// A window on the Local view over a given machine, with nothing arranged on
/// it: the state a launch reaches, which is where most of these start.
fn local_window(session: Session) -> Harness<'static, App> {
    window(session, |_, _| {})
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
    let mut harness = local_window(session);
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
    let mut harness = local_window(session);

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
    let mut harness = local_window(session);

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

    harness.state_mut().set_catalog(Catalog::just_fetched(published()));
    harness.run();
    assert!(
        harness.query_by_label("Update available").is_none(),
        "an entry at the published version was offered an update to it"
    );

    let mut newer = published();
    newer.items[0].version = "2.1.0".parse().expect("a version");
    harness.state_mut().set_catalog(Catalog::just_fetched(newer));
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
    window(session, |app, _| {
        app.set_catalog(Catalog::just_fetched(superseded()));
        app.show_view(View::Catalog);
    })
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

/// The bundle's own width for the plan confirmation, `ORNG Registry.dc.html:225`.
const CONFIRMATION_WIDTH: f32 = 476.0;

/// A control of the dialog over the window - not the bar's of the same name
/// under the scrim, and not the one the dialog measured itself with.
///
/// A dialog is laid out twice, into a sizing pass at the window's top left
/// corner and then for real in its middle, so every label on it is two nodes;
/// and the bar under it goes on carrying the word the dialog's press repeats.
/// The bar's is the one that runs past the dialog's right edge, and of the
/// other two the drawn one is the lower.
fn in_the_dialog<'t>(
    harness: &'t Harness<'static, App>,
    label: &'t str,
) -> egui_kittest::Node<'t> {
    let edge = (metric::WINDOW[0] + CONFIRMATION_WIDTH) / 2.0;
    harness
        .get_all_by_label_contains(label)
        .filter(|node| node.rect().right() <= edge)
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the dialog has no such control")
}

/// The primary action, which carries its glyph in its label.
fn the_primary_action<'t>(harness: &'t Harness<'static, App>) -> egui_kittest::Node<'t> {
    harness
        .get_all_by_label_contains("Prepare installation")
        .max_by(|a, b| a.rect().right().total_cmp(&b.rect().right()))
        .expect("the bar offers no press")
}

/// The one press that confirms before it runs, and what it says.
///
/// `ORNG Registry.dc.html:223-252`, on the bundle's own `confirm` scenario:
/// documents staged on an installation that has never been prepared, and a
/// removal queued. Three claims the picture cannot hold are asked of the tree:
/// that the press on the bar opens the plan and starts nothing; that the
/// dialog is the bundle's 476 wide, padded 14, and centred in the whole window
/// rather than in the working area; and that its two controls stand where the
/// bundle stands them, 8 apart, the press at the right edge. Then that `Cancel`
/// puts it away with nothing started, and that the dialog's own press hands the
/// work over - which fails at once against a fixture with an empty archive and
/// says so in the banner, which is the proof that it ran.
#[test]
fn the_prepare_press_confirms_with_the_plan_before_it_runs() {
    let root = fixture("confirming");
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, Helper::Absent, GuardState::Armed, entries);

    let mut harness = window(session, |app, _| app.set_staged(staged));

    // A removal queued, so the plan has a line for it too.
    harness.get_by_label("SHAPER").hover();
    harness.run();
    harness.get_by_label_contains("Remove entry").click();
    harness.run();

    the_primary_action(&harness).click();
    harness.run();
    assert!(harness.state().is_confirming(), "the press did not open the plan");
    assert!(!harness.state().is_working(), "the press ran without confirming");
    look(&mut harness, "confirming");

    // Every line derives from the rows and the machine: the backup directory
    // named for the build, the one row that is ready and not the two that are
    // not, the removal and what happens to its file, the kind folder, and the
    // three links a fixture with none of them will get.
    for line in [
        "The archive and the description bundles are backed up first to \
         ~/.orng/backups/6.1-94a90411/.",
        "1 entry registered: WAVESHAPER ALPHA.",
        "3 entries already registered keep their UUIDs; their description bundles are \
         written again.",
        "1 entry removed: SHAPER. The document file is kept.",
        "Placed in the user library: 1 to devices/My Devices.",
        "3 library links created inside the installation's Library folder, once the \
         archive is in place.",
    ] {
        assert!(anywhere(&harness, line), "the plan does not say: {line}");
    }
    assert!(
        !anywhere(&harness, "2 entries registered: WAVESHAPER ALPHA, DISPERSER."),
        "the plan counts a conflict as a registration"
    );

    // The bundle's own boxes, probed: 14 in from either edge, 12 under the
    // heading and 13 over the plan, 11 between its blocks, every line padded
    // 5 with its number a pixel down and 10 before the words, 14 under the
    // note, and the pair 12 into the foot and 8 apart. egui rounds the 16.5
    // line box to 17, which is the half pixel these tolerate.
    let title = in_the_dialog(&harness, "Prepare this installation").rect();
    let tag = in_the_dialog(&harness, "Plan").rect();
    let lead = in_the_dialog(&harness, "This is the one operation").rect();
    let first = in_the_dialog(&harness, "The archive and the description bundles").rect();
    let registered = in_the_dialog(&harness, "1 entry registered").rect();
    let kept = in_the_dialog(&harness, "3 entries already registered").rect();
    let last = in_the_dialog(&harness, "3 library links").rect();
    let note = in_the_dialog(&harness, "A Bitwig update resets").rect();
    let cancel = in_the_dialog(&harness, "Cancel").rect();
    let press = in_the_dialog(&harness, "Prepare installation").rect();
    let edge = (metric::WINDOW[0] + CONFIRMATION_WIDTH) / 2.0;
    let number = harness
        .get_all_by_label("1")
        .filter(|node| {
            node.rect().right() <= edge && node.rect().left() > edge - CONFIRMATION_WIDTH
        })
        .max_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the first line has no number")
        .rect();
    let left = edge - CONFIRMATION_WIDTH;
    assert_eq!(title.left(), left + 14.0, "the title is not on the dialog's padding");
    assert_eq!(tag.right(), edge - 14.0, "the tag is not on the dialog's padding");
    assert_eq!(lead.top() - title.bottom(), 12.0 + 13.0, "the lead is not 12 + 13 under the title");
    assert_eq!(first.top() - lead.bottom(), 11.0 + 5.0, "the plan is not 11 under the lead");
    assert_eq!(number.top(), first.top() + 1.0, "the number is not a pixel under its words");
    assert_eq!(first.left() - number.right(), 10.0, "the number is not 10 before its words");
    assert_eq!(kept.top() - registered.bottom(), 5.0 + 5.0, "the lines are not padded 5");
    assert_eq!(note.top() - last.bottom(), 5.0 + 11.0, "the note is not 11 under the plan");
    assert_eq!(press.top() - 12.0, note.bottom() + 14.0, "the foot is not 14 under the note");
    assert_eq!(press.right(), edge - 14.0, "the press is not at the right edge");
    assert_eq!(cancel.right() + 8.0, press.left(), "the pair is not 8 apart");
    assert_eq!(cancel.height(), 30.0, "Cancel is not the dialog's 30");
    assert_eq!(press.height(), 32.0, "the press is not the bar's 32");
    assert_eq!(cancel.center().y, press.center().y, "the pair is not centred on one line");
    // Centred in the window as a whole, which is a subtraction and not a look.
    let top = title.top() - 14.0;
    let bottom = press.bottom() + 12.0;
    assert!(
        ((top + bottom) - metric::WINDOW[1]).abs() <= 1.0,
        "the dialog is not centred in the window: {top} to {bottom}"
    );

    // The keyboard is held still too. Tab has exactly two stops while the
    // plan is up - the press, then the drawn Cancel, then round again - which
    // says two things a picture cannot: nothing under the scrim is in the ring,
    // and neither is the ghost of either control from the pass that measured
    // the dialog, which egui would otherwise stop on first.
    let focused = |harness: &Harness<'static, App>| {
        harness
            .ctx
            .memory(|memory| memory.focused())
            .and_then(|id| harness.ctx.read_response(id))
            .map(|response| response.rect)
    };
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), Some(press), "Tab did not stop first on the drawn press");
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), Some(cancel), "Tab did not stop next on the drawn Cancel");
    harness.key_press(egui::Key::Tab);
    harness.run();
    assert_eq!(focused(&harness), Some(press), "Tab found a third stop under the scrim");

    // The scrim takes the pointer: a press on the bar under it is not a press,
    // and the bar goes on counting the Local list rather than the catalog.
    on_the_bar(&harness, "Catalog").click();
    harness.run();
    assert!(
        anywhere(&harness, "1 to add, 1 to remove, 2 to fix"),
        "a press reached the bar under the scrim"
    );

    in_the_dialog(&harness, "Cancel").click();
    harness.run();
    assert!(!harness.state().is_confirming(), "Cancel did not put the plan away");
    assert!(!harness.state().is_working(), "Cancel started the work");
    assert!(!anywhere(&harness, "Prepare this installation"), "the plan is still drawn");

    the_primary_action(&harness).click();
    harness.run();
    in_the_dialog(&harness, "Prepare installation").click();
    harness.run();
    assert!(!harness.state().is_confirming(), "the plan stayed up after its press");
    settle(&mut harness);
    assert!(
        anywhere(&harness, "The preparation stopped, and your installation was not changed."),
        "the dialog's press did not hand the work over"
    );
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
        Catalog::unavailable("the signature does not match this index under this key"),
    );
    assert!(anywhere(&unavailable, "Catalog unavailable"));
    assert!(
        !bar_says(&unavailable, "0 items"),
        "an index that did not verify was counted as an empty catalog"
    );

    let mut refused = catalog_listing("catalog-bar-refused", superseded_entries());
    let item = superseded_replacement();
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

/// What the Local bar says when the filter hides work the press would do.
///
/// `ORNG Registry.dc.html:490`, as the designer answered it in round four: the
/// count is of pending changes out of sight, because Apply acts on rows the
/// user cannot see - and it is said whenever there are any, not only once the
/// list is empty.
///
/// Every claim here is invisible in a picture. A note reading `2 changes` looks
/// the same whether it counted changes, rows or the machine; only a filter that
/// leaves rows on screen, a row still to fix, and the preparing mode tell those
/// apart.
#[test]
fn the_local_bar_says_how_many_changes_the_filter_is_hiding() {
    let noted = |harness: &Harness<'static, App>| {
        harness.query_all_by_label_contains("hidden by the current filter").next().is_some()
    };
    let hidden = |harness: &Harness<'static, App>, what: &str| {
        anywhere(harness, &format!("{what} hidden by the current filter"))
    };

    // Nothing pending. A filter that empties the list hides no work, so the
    // bar has nothing to say about it.
    let mut harness = listing("local-hidden");
    harness.state_mut().set_query("zzz");
    harness.run();
    assert!(anywhere(&harness, "No entries match"), "the list did not go empty");
    assert!(!noted(&harness), "the bar counted entries rather than changes");

    // One document ready to add, one still to fix, one that is not a
    // document; and a removal queued.
    let root = fixture("local-hidden-staged");
    let to = destination(&root);
    let entries = entries();
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries.clone());
    let mut harness = local_window(session);
    harness.state_mut().set_staged(dropped(&root, &to, &entries));
    harness.run();
    harness.get_by_label("VOLSHAPER").hover();
    harness.run();
    harness.get_by_label_contains("Remove entry").click();
    harness.run();
    assert!(!noted(&harness), "the bar counted changes nothing is hiding");

    // The document to add is on screen and the removal is not. Rows are left,
    // and the note is said all the same.
    harness.state_mut().set_query("wavesh");
    harness.run();
    assert!(anywhere(&harness, "WAVESHAPER ALPHA"), "the query hid the document it should show");
    assert!(hidden(&harness, "1 change"), "a hidden removal went unsaid over a list with rows");
    assert!(!hidden(&harness, "1 changes"));

    // Both hidden. The conflicting row is hidden too and is not counted, because
    // Apply leaves it where it is; the file that is not a document stays on
    // screen, since a filter over entries has nothing to hide it by.
    harness.state_mut().set_query("zzz");
    harness.run();
    assert!(hidden(&harness, "2 changes"), "the count was not the ready row and the removal");

    // The preparing mode keeps its cost on the line: the plan lists every
    // change before anything is written.
    let cost = format!("Prepare install {} a backup is written first", crate::widget::SEPARATOR);
    let root = fixture("local-hidden-unprepared");
    let to = destination(&root);
    let unprepared = found(&root, Helper::Absent, GuardState::Armed);
    let mut unprepared = local_window(unprepared);
    unprepared.state_mut().set_staged(dropped(&root, &to, &entries));
    unprepared.state_mut().set_query("zzz");
    unprepared.run();
    assert!(anywhere(&unprepared, &cost), "the backup's cost gave the line up");
    assert!(!noted(&unprepared), "the bar drew two notes on a line that holds one");
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
    let open = open.parse().expect("a sample identity");
    window(session, |app, _| {
        app.set_catalog(Catalog::just_fetched(superseded()));
        app.show_view(View::Catalog);
        app.set_detailing(open);
    })
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

    let index = superseded();
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

/// A fetch that answers while a run is in flight waits for the run, and is
/// written the frame the run reports.
///
/// The second half of an install is a run of its own, and one started over a run
/// already going took that run's place: the run went on writing and was never
/// heard from, and the install's list was the one from before that run wrote.
/// The run here is one started from the Local list while the fetch was out.
#[test]
fn a_fetch_that_answers_during_a_run_waits_for_it() {
    let mut harness = catalog_listing("installing-mid-run", superseded_entries());
    let replacement: orng_tools::Uuid = BREATH_FOLLOWER_II.parse().expect("a sample identity");
    let item = superseded()
        .items
        .into_iter()
        .find(|item| item.uuid == replacement)
        .expect("the sample index carries it");
    let document =
        orng_tools::testing::document(item.kind.into(), item.uuid, "BREATH FOLLOWER II");
    let fetching = |harness: &Harness<'_, App>| {
        harness.query_all_by_label_contains("Fetching BREATH FOLLOWER II").next().is_some()
    };

    harness.state_mut().set_applying(Applying::frozen(None, Stage::Registering, None));
    harness.state_mut().set_installing(Install::finished(item, Ok(document)));
    // Nothing to see of the fetch while it waits: the bar says what the run is
    // doing. What shows it waited is that nothing was written.
    harness.run();
    let entries = harness.state().registered().expect("an installation");
    assert!(entries.get(replacement).is_none(), "the fetch was written over the run in flight");

    // The run reports the list it was handed, which is all a run that wrote
    // nothing would have to say.
    let written = harness.state().registered().expect("an installation").clone();
    let finished = Applying::frozen(None, Stage::Registering, Some(Ok(written)));
    harness.state_mut().set_applying(finished);
    // One frame, and not `run`: the claim is about which frame.
    harness.step();
    assert!(!fetching(&harness), "the fetch was left waiting past the frame the run reported");

    settle(&mut harness);
    let entries = harness.state().registered().expect("an installation");
    assert!(entries.get(replacement).is_some(), "the fetch that waited was never written");
}

/// A consent dialog the user dismissed is reported as what they did, and not as
/// the run failing.
///
/// Through a run's own answer rather than by declaring the outcome: what is
/// being claimed is that `Declined` coming back from a run is kept apart from
/// every other way a run stops, all the way to the banner.
#[test]
fn a_declined_consent_dialog_is_not_a_failure() {
    let mut harness = listing("declined");
    let declined = Some(Err(crate::elevate::Stopped::Declined));
    harness.state_mut().set_applying(Applying::frozen(None, Stage::Registering, declined));
    settle(&mut harness);

    assert!(anywhere(&harness, "Administrator rights were declined, so nothing was changed."));
    assert!(!anywhere(&harness, "Nothing was registered."), "it was said as a failed run");
    assert_eq!(harness.query_all_by_label("Copy details").count(), 0, "a refusal offered details");
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
    let index = superseded();
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
    let item = superseded_replacement();
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
    let mut harness = window(session, |app, _| {
        let mut index = superseded();
        index.revision = None;
        app.set_catalog(Catalog::just_fetched(index));
        app.show_view(View::Catalog);
    });

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
    let mut harness = local_window(session);
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
    let mut harness = window(session, |app, _| app.set_staged(staged));

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
    let mut harness = local_window(session);

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
        harness.query_all_by_label_contains("to remove").next().is_none(),
        "the press would forget an entry the list is showing as staged"
    );
    assert!(harness.query_all_by_label("1 to add").next().is_some(), "the drop was not staged");
}

/// A queued removal is written by the press, and the queue goes with it.
///
/// The queue names identities rather than rows, so one left standing after the
/// press would strike the entry through again the next time it came back - the
/// very document the user removed, dropped and registered again, drawn as about
/// to be forgotten.
#[test]
fn a_queued_removal_is_written_and_the_queue_goes_with_it() {
    const DISPERSER: &str = "80c0dc4c-d142-53a7-85ee-b91427819b66";
    let root = fixture("removal-written");
    let to = Destination { placement: Strategy::Copy, ..destination(&root) };
    let mut harness = local_window(copying(&root, entries()));
    let uuid: orng_tools::Uuid = DISPERSER.parse().expect("a sample identity");

    harness.get_by_label("DISPERSER").hover();
    harness.run();
    harness.get_by_label_contains("Remove entry").click();
    harness.run();
    harness.get_by_label_contains("Apply 1 change").click();
    settle(&mut harness);
    let written = harness.state().registered().expect("an installation").clone();
    assert!(written.get(uuid).is_none(), "the press did not forget the entry it was asked to");

    // The same document, dropped again and written.
    let drop = root.join("dropped");
    std::fs::create_dir_all(&drop).expect("a place to drop from");
    let path = drop.join("DISPERSER.bwdevice");
    let document = orng_tools::testing::document(orng_tools::Kind::Device, uuid, "DISPERSER");
    std::fs::write(&path, document.bytes()).expect("could not write the sample");
    harness.state_mut().set_staged(staging::read(&[path], &written, &to, &[]));
    harness.run();
    harness.get_by_label_contains("Apply 1 change").click();
    settle(&mut harness);

    let written = harness.state().registered().expect("an installation");
    assert!(written.get(uuid).is_some(), "the document dropped again was not registered");
    assert!(
        harness.query_by_label("Pending removal").is_none(),
        "the entry that came back is still queued for removal"
    );
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

    let mut harness = window(session, |app, _| app.set_staged(staged));
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

    let mut harness = window(session, |app, _| app.set_staged(staged));
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
    let mut harness =
        window(session, |app, _| app.set_inspecting(VOLSHAPER.parse().expect("a sample identity")));

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

/// An edit to an installation this account may not write waits for a press.
///
/// The write is carried to a child process holding rights this one does not,
/// and on Windows starting that child raises the system's consent dialog. A
/// field losing focus is not a press: a consent dialog that arrives because the
/// pointer moved out of a text box is one people learn to dismiss without
/// reading, and every other press here that asks for rights is deliberate. So
/// the words wait behind a named `Save`, with `Cancel` beside it, and refusing
/// leaves the entry as it was.
///
/// Only on Windows, which is the only platform with a dialog to put off. Anywhere
/// else there is no `Save` that could succeed, so the edit is written as any
/// other is, the child cannot be started, and the window says the change was not
/// saved - which is what it did before the question existed. Both halves are
/// here so that each platform proves the one it can reach.
#[test]
fn an_edit_that_needs_rights_waits_for_a_press_only_where_it_can_ask() {
    use egui_kittest::kittest::NodeT as _;
    let mut harness = without_rights("edit-without-rights", VOLSHAPER);
    add_a_keyword(&mut harness, "tremolo");

    if !crate::elevate::can_ask() {
        assert!(!offered_to_save(&harness), "offered a save that can only fail");
        settle(&mut harness);
        assert!(
            harness.query_by_label("The change was not saved.").is_some(),
            "the refused write was not reported"
        );
        return;
    }

    assert!(!harness.state().is_working(), "leaving a field asked for rights by itself");
    assert!(offered_to_save(&harness), "the words were neither written nor offered to be saved");
    assert!(harness.query_by_label("Cancel").is_some(), "the offer had no way to refuse it");
    // In the panel's foot, under the words it would save, which stay in view.
    let panel = metric::WINDOW[0] - metric::ASIDE;
    let question = in_the_panel(&harness, "Save the changes to VOLSHAPER?");
    let keyword = in_the_panel(&harness, "tremolo");
    assert!(question.left() > panel, "the question is not in the panel");
    assert!(keyword.bottom() < question.top(), "the words are not in view above the question");
    let close = panel_close(&harness);
    assert!(close.accesskit_node().is_disabled(), "the panel could be closed over the question");

    harness.get_by_label("Cancel").click();
    harness.run();
    assert!(!offered_to_save(&harness), "Cancel left the question on screen");
    assert!(!harness.state().is_working(), "Cancel started the write it was refusing");
    assert!(
        harness.query_all_by_label("tremolo").next().is_none(),
        "Cancel kept the words it was refusing"
    );
    let close = panel_close(&harness);
    assert!(!close.accesskit_node().is_disabled(), "the panel stayed held after the answer");
}

/// Whether the inspector's foot is offering to save its words: the press wears
/// the shield before its word, as every press that asks for rights does.
fn offered_to_save(harness: &Harness<'_, App>) -> bool {
    let save = format!("{}Save", crate::widget::icon::ELEVATES);
    harness.query_all_by_label(&save).next().is_some()
}

/// Where something the inspector draws is, found inside the panel's column:
/// the list beside it may say the same words.
fn in_the_panel(harness: &Harness<'_, App>, label: &str) -> egui::Rect {
    let panel = metric::WINDOW[0] - metric::ASIDE;
    harness
        .query_all_by_label(label)
        .map(|node| node.rect())
        .find(|rect| rect.left() > panel)
        .unwrap_or_else(|| panic!("the panel does not say {label:?}"))
}

/// The inspector's close: the topmost mark in the panel's column, since a
/// banner under it has one too.
fn panel_close<'a>(harness: &'a Harness<'_, App>) -> egui_kittest::Node<'a> {
    let panel = metric::WINDOW[0] - metric::ASIDE;
    harness
        .get_all_by_label(crate::widget::icon::DISMISS)
        .filter(|node| node.rect().left() > panel)
        .min_by(|a, b| a.rect().top().total_cmp(&b.rect().top()))
        .expect("the panel has no close")
}

/// And the question stands down while a run is going, and is asked again once
/// it has reported.
///
/// A run that prepares nothing draws no scrim, so the banner under it stayed
/// pressable: `Save` there started a second run over the first, and on Windows
/// the second child wrote the list as it stood before the first had registered
/// anything. Windows only, for the reason the test above gives.
#[test]
fn a_waiting_edit_is_not_offered_while_a_run_is_going() {
    let mut harness = without_rights("edit-behind-a-run", VOLSHAPER);
    add_a_keyword(&mut harness, "tremolo");
    if !crate::elevate::can_ask() {
        return;
    }
    assert!(offered_to_save(&harness), "the words were not offered to be saved");

    harness.state_mut().set_applying(Applying::frozen(None, Stage::Registering, None));
    harness.run();
    assert!(!offered_to_save(&harness), "Save was offered over a run in flight");
    assert!(
        anywhere(&harness, "Saved when the registration finishes"),
        "the panel did not say what its words wait for"
    );

    let written = harness.state().registered().expect("an installation").clone();
    let finished = Applying::frozen(None, Stage::Registering, Some(Ok(written)));
    harness.state_mut().set_applying(finished);
    harness.run();
    assert!(!harness.state().is_working(), "the run did not report");
    assert!(offered_to_save(&harness), "the question did not come back");
}

/// The question holds its panel: another row clicked leaves the panel where
/// it is, still asking.
///
/// Before revision 8 the question was a banner across the bottom of the window
/// and the panel went where it was sent, so a second entry's words could be
/// typed under a question about the first, and had to wait behind it. Now the
/// question is in the panel's foot, and the panel cannot be turned from it.
#[test]
fn the_question_holds_its_panel() {
    let mut harness = without_rights("edit-holds-the-panel", VOLSHAPER);
    add_a_keyword(&mut harness, "tremolo");
    if !crate::elevate::can_ask() {
        return;
    }
    assert!(offered_to_save(&harness), "the words were not offered to be saved");

    harness.get_by_label("DISPERSER").click();
    harness.run();
    in_the_panel(&harness, "Save the changes to VOLSHAPER?");
    assert!(offered_to_save(&harness), "turning to another row dropped the question");
}

/// A panel whose words are waiting is not let go of: closed, or turned to
/// another row, it dropped the one copy of them there was without a word.
///
/// Waiting behind a run here, which every platform reaches. Behind the question
/// about another entry is the same `write_words` answer, and Windows only.
#[test]
fn a_panel_holding_words_that_wait_is_not_let_go() {
    let root = fixture("words-behind-a-run");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let mut harness =
        window(session, |app, _| app.set_inspecting(VOLSHAPER.parse().expect("an identity")));
    harness.state_mut().set_applying(Applying::frozen(None, Stage::Registering, None));
    harness.run();
    add_a_keyword(&mut harness, "tremolo");
    let panel = metric::WINDOW[0] - metric::ASIDE;
    // Inside the panel: a wide enough list states the identity in a column of
    // its own.
    let open = |harness: &Harness<'_, App>| {
        harness.query_all_by_label_contains(VOLSHAPER).any(|node| node.rect().left() > panel)
    };
    let close = |harness: &mut Harness<'_, App>| {
        panel_close(harness).click();
        harness.run();
    };
    assert!(open(&harness), "the panel this test relies on is not open");

    // It says so, and its close says why it will not.
    use egui_kittest::kittest::NodeT as _;
    in_the_panel(&harness, "Saved when the registration finishes");
    assert!(panel_close(&harness).accesskit_node().is_disabled(), "the close looked pressable");
    look(&mut harness, "inspector-waiting");

    harness.get_by_label("DISPERSER").click();
    harness.run();
    assert!(open(&harness), "the panel was turned to another row over its words");
    close(&mut harness);
    assert!(open(&harness), "the panel closed over its words");

    // And once the run has reported, the close writes them and goes.
    let written = harness.state().registered().expect("an installation").clone();
    let finished = Applying::frozen(None, Stage::Registering, Some(Ok(written)));
    harness.state_mut().set_applying(finished);
    harness.run();
    close(&mut harness);
    settle(&mut harness);
    assert!(!open(&harness), "the panel stayed once its words were written");
}

/// The foot names the work the words wait for: a download is not a
/// registration, and a user who pressed Install knows it by the first name.
#[test]
fn the_waiting_foot_names_the_work() {
    let root = fixture("words-behind-a-fetch");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let item = superseded().items.into_iter().next().expect("the sample index has an item");
    let mut harness = window(session, |app, _| {
        app.set_inspecting(VOLSHAPER.parse().expect("an identity"));
        app.set_installing(Install::fetching(item));
    });
    harness.run();
    add_a_keyword(&mut harness, "tremolo");
    in_the_panel(&harness, "Saved when the download finishes");
}

/// While an install's fetch is out, the primary action does not start a run.
///
/// The fetch's second half is a run, and one started beside another had to
/// wait for it and then state its outcome over the other's - a preparation's
/// banner, with the instruction to start Bitwig in it, replaced unread.
#[test]
fn the_primary_action_waits_for_an_install_fetch() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT as _;

    let root = fixture("apply-while-fetching");
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);
    let item = superseded().items.into_iter().next().expect("the sample index has an item");
    let harness = window(session, |app, _| {
        app.set_staged(staged);
        app.set_installing(Install::fetching(item));
    });

    let press = harness
        .get_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.starts_with("Apply ")))
        .expect("the action bar has no primary action");
    assert!(press.accesskit_node().is_disabled(), "a run could be started beside a fetch");
}

/// The row that was pressed says so while its download is out, and offers
/// nothing to press - revision 8's eighth published state.
#[test]
fn the_row_being_fetched_says_so() {
    let mut harness = catalog_listing("catalog-fetching", superseded_entries());
    let replacement: orng_tools::Uuid = BREATH_FOLLOWER_II.parse().expect("a sample identity");
    let item = superseded()
        .items
        .into_iter()
        .find(|item| item.uuid == replacement)
        .expect("the sample index carries it");
    assert!(harness.query_by_label("Install").is_some(), "the sample offers nothing to install");

    harness.state_mut().set_installing(Install::fetching(item));
    harness.run();
    assert!(harness.query_by_label("Fetching...").is_some(), "the row did not say it is fetching");
    assert!(harness.query_by_label("Install").is_none(), "the row offered to install it again");
    // And only that row.
    assert!(harness.query_by_label("Replacement available").is_some());
}

/// Take the rights to write the installation away from a session read off this
/// machine, where every directory is writable by the test.
fn withhold(session: &mut Session) {
    let Session::Found(found) = session else { panic!("no installation to withhold") };
    found.rights = orng_tools::Rights::Withheld {
        directory: found.to.install.root().to_path_buf(),
        why: "this account may not write there".to_owned(),
    };
}

/// A window whose installation this account may not write, with one entry's
/// inspector open.
///
/// The rights are set on the session rather than found, because every directory
/// on the machine running this is writable by it - which is the same reason
/// `rights`' own refusal test has to take a write bit off by hand.
fn without_rights(name: &str, inspecting: &'static str) -> Harness<'static, App> {
    let root = fixture(name);
    let mut session = found(&root, Helper::Present, GuardState::Disarmed);
    withhold(&mut session);
    window(session, move |app, _| app.set_inspecting(inspecting.parse().expect("an identity")))
}

/// Type a keyword into the open panel and leave the field, which is the moment
/// that used to write it - and used to be the moment that asked Windows for
/// rights.
///
/// A keyword the entry does not have yet, or the words do not change and
/// nothing is written or asked.
fn add_a_keyword(harness: &mut Harness<'_, App>, keyword: &str) {
    use egui::accesskit::Role;

    // The panel's one single-line field. The description is the other field,
    // and a multi-line one is another role.
    let panel = metric::WINDOW[0] - metric::ASIDE;
    {
        let fields: Vec<_> = harness
            .get_all_by_role(Role::TextInput)
            .filter(|node| node.rect().left() > panel)
            .collect();
        let [adding] = &fields[..] else {
            panic!("the panel has {} single-line fields, not one", fields.len())
        };
        adding.focus();
        adding.type_text(keyword);
    }
    harness.run();
    harness.key_press(egui::Key::Tab);
    harness.run();
}

/// Staged work on an installation this account may not write is refused only
/// where nothing can ask for rights.
///
/// On macOS and Linux the banner stands in the action bar's way, names the
/// directory, and the primary action is disabled with the same words on it. On
/// Windows the press is the ordinary one and asks when it is made, so neither
/// the banner nor a disabled press would be true there. Round 3 item 9. Both
/// halves are here so that each platform proves the one it can reach.
///
/// Read off the tree rather than pressed: on Windows the press would start this
/// test binary elevated.
#[test]
fn a_press_that_needs_rights_is_refused_only_where_nothing_can_ask() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT as _;

    const REFUSED: &str = "This installation is not yours to change.";

    let root = fixture("apply-without-rights");
    let to = destination(&root);
    let entries = entries();
    let staged = dropped(&root, &to, &entries);
    let mut session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);
    withhold(&mut session);
    let harness = window(session, |app, _| app.set_staged(staged));

    // Found by what it says rather than how it starts: where it can ask, a
    // shield comes before the words.
    let press = harness
        .get_all_by_role(Role::Button)
        .find(|node| node.accesskit_node().label().is_some_and(|label| label.contains("Apply ")))
        .expect("the action bar has no primary action");
    let refused = harness.query_by_label(REFUSED).is_some();
    if crate::elevate::can_ask() {
        assert!(!refused, "refused a press that can ask for the rights it needs");
        assert!(!press.accesskit_node().is_disabled(), "the press was disabled all the same");
    } else {
        assert!(refused, "nothing said why the staged work cannot be applied");
        assert!(press.accesskit_node().is_disabled(), "a press that cannot ask stayed enabled");
    }
}

/// A press that will end in Windows' consent dialog wears the shield, on the
/// bar and on the plan the bar opens. One that will not - rights held, or a
/// platform with no way to ask - does not.
///
/// Read off the tree and never pressed through: the plan's own press would
/// start this test binary elevated on Windows. The bar's is pressed, because
/// on a preparation all it does is open the plan.
#[test]
fn a_press_that_asks_for_rights_wears_the_shield() {
    use egui_kittest::kittest::NodeT as _;

    for withheld in [false, true] {
        let root = fixture(if withheld { "shield-withheld" } else { "shield-held" });
        let to = destination(&root);
        let entries = entries();
        let staged = dropped(&root, &to, &entries);
        let mut session = found_with(&root, Helper::Absent, GuardState::Armed, entries);
        if withheld {
            withhold(&mut session);
        }
        let mut harness = window(session, |app, _| app.set_staged(staged));
        let asks = withheld && crate::elevate::can_ask();
        // Every press that says `Prepare installation`: the bar's, and the
        // plan's once it is open.
        let shields = |harness: &Harness<'_, App>| -> Vec<bool> {
            harness
                .get_all_by_label_contains(PREPARING)
                .map(|node| {
                    let label = node.accesskit_node().label().unwrap_or_default();
                    label.contains(crate::widget::icon::ELEVATES)
                })
                .collect()
        };

        assert!(
            shields(&harness).iter().all(|&shield| shield == asks),
            "the bar's press wore the shield {} (withheld: {withheld})",
            if asks { "nowhere" } else { "where nothing asks" }
        );
        // Where nothing can ask, the press is refused and there is no plan to
        // open - the test above.
        if withheld && !crate::elevate::can_ask() {
            continue;
        }
        the_primary_action(&harness).click();
        harness.run();
        assert!(harness.state().is_confirming(), "the press did not open the plan");
        let open = shields(&harness);
        assert!(open.len() > 1, "the plan has no press of its own");
        assert!(
            open.iter().all(|&shield| shield == asks),
            "the plan's press disagreed with the bar about rights (withheld: {withheld})"
        );
    }
}

/// Where Apply will raise the consent dialog, the bar's note says so in place
/// of `Bitwig may stay open` - the designer's `windowslocal` state. Where the
/// rights are held, or nothing can ask, it does not.
#[test]
fn the_bar_says_when_applying_asks_for_rights() {
    for withheld in [false, true] {
        let root = fixture(if withheld { "asks-note-withheld" } else { "asks-note-held" });
        let to = destination(&root);
        let entries = entries();
        let staged = dropped(&root, &to, &entries);
        let mut session = found_with(&root, Helper::Present, GuardState::Disarmed, entries);
        if withheld {
            withhold(&mut session);
        }
        let harness = window(session, |app, _| app.set_staged(staged));
        let separator = crate::widget::SEPARATOR;
        let says = |what: &str| anywhere(&harness, &format!("Update entries {separator} {what}"));
        let (asks, open) = (says("asks for administrator rights"), says("Bitwig may stay open"));
        let expected = withheld && crate::elevate::can_ask();
        assert_eq!(asks, expected, "the note about rights (withheld: {withheld})");
        assert_eq!(open, !expected, "the note about Bitwig (withheld: {withheld})");
    }
}

/// The small presses that write into the installation say so where the press
/// will raise the consent dialog, each the way the designer placed it: the
/// catalog row's `Install` wears the shield before its word, the Restore press
/// wears it in place of its clock, and `Locate`, a glyph with no room for a
/// second one, says it on hover. A press that only opens something does not.
#[test]
fn the_small_presses_that_ask_for_rights_say_so() {
    use egui::accesskit::Role;
    use egui_kittest::kittest::NodeT as _;
    let label = |node: egui_kittest::Node<'_>| {
        node.accesskit_node().label().unwrap_or_default().to_owned()
    };

    for withheld in [false, true] {
        let asks = withheld && crate::elevate::can_ask();
        let rights = |mut session: Session| {
            if withheld {
                withhold(&mut session);
            }
            session
        };
        let name = |what: &str| format!("{what}-{}", if withheld { "withheld" } else { "held" });

        let session = rights(copying(&fixture(&name("shield-catalog")), superseded_entries()));
        let catalog = window(session, |app, _| {
            app.set_catalog(Catalog::just_fetched(superseded()));
            app.show_view(View::Catalog);
        });
        let presses: Vec<String> = catalog.get_all_by_role(Role::Button).map(label).collect();
        let install: Vec<&String> = presses.iter().filter(|l| l.ends_with("Install")).collect();
        assert!(!install.is_empty(), "the sample offers nothing to install");
        for press in install {
            assert_eq!(press.contains(crate::widget::icon::ELEVATES), asks, "{press:?}");
        }
        let replacement = presses.iter().find(|l| l.ends_with("See replacement"));
        let replacement = replacement.expect("the sample offers no replacement");
        assert!(!replacement.contains(crate::widget::icon::ELEVATES), "{replacement:?}");

        let root = fixture(&name("shield-restore"));
        let session = rights(found(&root, Helper::Absent, GuardState::Armed));
        let restore = window(session, |app, _| app.show_restore());
        let press = restore
            .get_all_by_role(Role::Button)
            .map(label)
            .find(|l| l.contains("Restore this backup"))
            .expect("the screen has no press");
        assert_eq!(press.contains(crate::widget::icon::ELEVATES), asks, "{press:?}");
        assert_eq!(press.contains(crate::widget::icon::RESTORE), !asks, "{press:?}");

        let mut damaged = local_window(rights(damaged(&fixture(&name("shield-locate")))));
        damaged.get_by_label("SHAPER").hover();
        damaged.run();
        let separator = crate::widget::SEPARATOR;
        let asking = format!("Locate file {separator} asks Windows for administrator rights");
        assert_eq!(damaged.query_by_label(&asking).is_some(), asks);
        assert_eq!(damaged.query_by_label("Locate file").is_some(), !asks);
    }
}

/// The words on the preparing press, wherever it is drawn.
const PREPARING: &str = "Prepare installation";

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
    // The palette goes in with the rest of the preferences rather than through
    // `set_appearance` beside them - which is what the window actually does, and
    // what the first attempt at this got wrong: the setter ran first, the
    // preferences replaced everything it had set, and `settings-light.png` came
    // out in dark with a passing test beside it.
    let settings = Settings { appearance: appearance(dark), ..settings };
    let mut harness = window(session, |app, _| {
        app.set_settings(settings);
        app.show_settings();
    });
    look(&mut harness, name);
}

/// Every cell of a path row centred on the row, against `SettingsScreen.dc.html`
/// rendered at its own preview size and asked for every box.
///
/// The grid is `align-items:center`, and the bundle puts the first two rows at 91
/// and 125, 25 tall, with the 14-tall label, the path, `Browse` and the reset
/// control each centred on 103.5 and 137.5. The label and `No backup yet` hung
/// from the row's top instead, four and a half pixels high, and `settings.png`
/// vouched for it: a picture has no opinion about where the middle is.
///
/// The third row is drawn here with no backup, which the bundle's preview is
/// not: its `Restore...` makes it 24 tall at 159. Without it the tallest cell is
/// the 23-tall path box, and the middle is 170.5.
#[test]
fn a_path_rows_cells_are_centred_on_the_row() {
    let root = fixture("settings-centred");
    let session = found(&root, Helper::Present, GuardState::Disarmed);
    let harness = window(session, |app, _| app.show_settings());

    // A row is measured before it is drawn, and the measuring pass leaves a
    // node for every cell hung from the top of the group. So each cell is the
    // lowest node carrying its words that shares a line with the row's label.
    let beside = |row: egui::Rect, cell: &str| {
        harness
            .get_all_by_label_contains(cell)
            .map(|node| node.rect())
            .filter(|rect| rect.top() < row.bottom() && row.top() < rect.bottom())
            .max_by(|a, b| a.top().total_cmp(&b.top()))
            .unwrap_or_else(|| panic!("no {cell:?} beside the row at {}", row.top()))
    };
    let reset = crate::widget::icon::RESET;
    for (label, path, middle) in
        [("Bitwig install", "render-fixtures", 103.5), ("User library", "render-fixtures", 137.5)]
    {
        let row = in_the_region(&harness, label).rect();
        assert_eq!(row.height(), 14.0, "{label:?} is not the bundle's line");
        for cell in [label, path, "Browse", reset] {
            assert_eq!(beside(row, cell).center().y, middle, "{cell:?} on the {label:?} row");
        }
    }
    let row = in_the_region(&harness, "Backups").rect();
    for cell in ["Backups", ".orng/backups", "No backup yet"] {
        assert_eq!(beside(row, cell).center().y, 159.0 + 23.0 / 2.0, "{cell:?} on the Backups row");
    }
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
    let mut harness = local_window(session);

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
    let mut harness = window(session, |app, _| app.show_settings());
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
    let mut harness = window(session, |app, _| app.show_restore());
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

    let harness = window(session, |app, _| app.show_restore());

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
    let mut harness =
        window(session, |app, _| app.show_about("0.9.2 \u{b7} orng-registry \u{b7} macOS arm64"));
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
    let mut harness = local_window(session);

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

    let mut harness = window(session, |app, _| app.show_restore());

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
        Catalog::unavailable("the signature does not match this index under this key"),
    );
}

/// The sample index every catalog fixture in this file is built on, as bytes.
const PUBLISHED: &str = include_str!("../tests/published-index.json");

/// How long ago the design's stale scenario says its index arrived -
/// `ORNG Registry.dc.html:467`.
const TWELVE_DAYS: std::time::Duration = std::time::Duration::from_secs(12 * 24 * 60 * 60);

fn sample() -> orng_catalog::Index {
    orng_catalog::Index::parse(PUBLISHED).expect("the sample index does not parse")
}

/// Offline with a kept index: the list still browses, and the bar states its
/// age in the accent beside a control that has grown its word.
///
/// `ORNG Registry.dc.html:465-469`, which is the one catalog scenario about the
/// network that is not an empty region. Everything the caption claims is in one
/// picture: no banner, rows that still press, `cached` after the count and
/// `Offline` under it.
#[test]
fn a_catalog_that_is_offline_with_something_kept() {
    shot_catalog("catalog-cached", Catalog::cached(sample(), TWELVE_DAYS, "no route to host"));
}

/// The catalog's age and its refresh control, where the design puts them and at
/// the size it draws them.
///
/// Measured off `InstallBar.dc.html` at its own 820 by 42 preview with `view`
/// flipped to catalog, once current and once stale. The bundle's own numbers:
/// the control is 22 square while it is icon-only and 22 tall once it is not,
/// it sits twelve from the age on its left and twelve from `Change install` on
/// its right in both shapes, and the group is centred in the bar's 24-tall row
/// rather than standing on its floor.
///
/// **Only the box is measured against the bundle and not the width of the
/// stale shape.** Chrome has no network in this sandbox, so Inter falls back
/// and the bundle's 80.1 is nine of padding, sixteen of glyph, six of gap and
/// whatever width the fallback gave the word. The paddings are the claim; the
/// word is not.
#[test]
fn the_catalogs_age_and_its_refresh_sit_where_the_bundle_draws_them() {
    /// `InstallBar.dc.html:24`, the gap between every pair in the bar.
    const BETWEEN: f32 = 12.0;
    /// `:95` and `:97`.
    const CONTROL: f32 = 22.0;

    let current = catalog_with("catalog-freshness", Catalog::just_fetched(sample()));
    let age = current.get_by_label("Catalog updated just now").rect();
    let refresh = current.get_by_label(crate::widget::REFRESH_CATALOG).rect();
    // The glyph is appended to this one's text, so it is matched on part of the
    // label - `widget::small_button` builds the run and the label follows it.
    let change = current.get_by_label_contains("Change install").rect();

    assert_eq!(
        (refresh.width(), refresh.height()),
        (CONTROL, CONTROL),
        "the current shape is not the square the design draws"
    );
    assert_eq!(refresh.left() - age.right(), BETWEEN, "the control is not twelve from the age");
    assert_eq!(
        change.left() - refresh.right(),
        BETWEEN,
        "the control is not twelve from Change install"
    );
    // Centred in the row rather than sitting on its floor, which is what a
    // control two shorter than the one beside it has to do.
    assert_eq!(refresh.center().y, change.center().y);

    // Stale: the same box in every direction but one, and the one it grows in
    // is the only one the design lets it.
    let stale =
        catalog_with("catalog-freshness-stale", Catalog::cached(sample(), TWELVE_DAYS, "x"));
    let age = stale.get_by_label("Catalog from 12 days ago").rect();
    let refresh = stale.get_by_label(crate::widget::REFRESH_CATALOG).rect();
    let change = stale.get_by_label_contains("Change install").rect();

    assert_eq!(refresh.height(), CONTROL, "the stale shape is not the design's height");
    assert!(refresh.width() > CONTROL, "the stale shape did not grow a label");
    assert_eq!(refresh.left() - age.right(), BETWEEN);
    assert_eq!(change.left() - refresh.right(), BETWEEN);
    assert_eq!(refresh.center().y, change.center().y);
    // The word itself, which is what the growth is for, and only the one.
    assert_eq!(
        stale.query_all_by_label_contains("Refresh").count(),
        1,
        "the stale control is drawn twice, or not at all"
    );
    // An index that is old is not an index that was never had, and the two
    // states are one sentence apart on the same eleven pixels of bar.
    assert!(
        !anywhere(&stale, "Never fetched"),
        "a window holding a twelve-day-old index said it had never fetched one"
    );

    // And neither is drawn in the Local view, which is the bundle's own
    // condition at `:35`.
    let mut local = catalog_with("catalog-freshness-local", Catalog::just_fetched(sample()));
    local.state_mut().show_view(View::Local);
    local.run();
    assert!(
        !anywhere(&local, "Catalog updated just now"),
        "the Local view states the catalog's age"
    );
    assert!(
        local.query_all_by_label(crate::widget::REFRESH_CATALOG).next().is_none(),
        "the Local view offers to refresh the catalog"
    );
}

/// What the bar says when the index on screen is one the last refresh did not
/// replace.
///
/// `ORNG Registry.dc.html:468` is both halves - `cached` after the count and
/// `Offline` before what still works - and this is the assertion a picture
/// cannot make: the same nine rows are on screen either way, so only the two
/// sentences tell a window that checked from a window that could not.
///
/// **It turns on the refresh having failed and not on where the bytes came
/// from.** An index read off the disk and then confirmed by a fetch that worked
/// is current, and the last arm here is what says so.
#[test]
fn the_bar_says_cached_only_when_a_refresh_did_not_replace_what_is_on_screen() {
    let separator = crate::widget::SEPARATOR;
    let offline = catalog_with(
        "catalog-cached-bar",
        Catalog::cached(sample(), TWELVE_DAYS, "no route to host"),
    );
    assert!(
        bar_says(&offline, &format!("1 item {separator} cached")),
        "the bar counts a kept catalog without saying it is one"
    );
    assert!(
        anywhere(&offline, &format!("Offline {separator} installing a cached item still works")),
        "the bar does not say that a kept item still installs"
    );
    // The note it displaces, which is what a window that had just checked would
    // be saying instead.
    assert!(
        !anywhere(&offline, &format!("Installing is Update entries work {separator} no backup, \
                                      Bitwig may stay open")),
        "the offline note did not displace the cost of a press"
    );
    // The rows are still there, and still press. A degraded state, not an
    // error: `:466` says no banner, and an empty region would be worse than one.
    assert!(anywhere(&offline, "VOLSHAPER"), "an offline window lost the list it had");

    let checked = catalog_with("catalog-current-bar", Catalog::just_fetched(sample()));
    assert!(bar_says(&checked, "1 item"), "the bar does not count a catalog it just fetched");
    assert!(
        !anywhere(&checked, &format!("1 item {separator} cached")),
        "a catalog that just arrived was reported as cached"
    );
    assert!(!anywhere(&checked, &format!("Offline {separator} installing a cached item still \
                                          works")));
}

/// A refusal is a claim about one row of one index, and an index that has been
/// replaced takes it with it.
///
/// Not tidiness. The map is keyed by identity and the bar reads `Install
/// refused` while anything is in it, so a refusal against an item the catalog
/// has since stopped publishing would hold that bar for the rest of the run
/// with no row under it to explain itself. Nothing refetched the catalog before
/// this change, so the field's own comment promised this and nothing could
/// reach it.
#[test]
fn a_refresh_that_changes_the_catalog_takes_the_refusals_with_it() {
    let mut harness = catalog_listing("catalog-refresh-clears", superseded_entries());
    let item = superseded_replacement();
    harness.state_mut().set_installing(Install::finished(
        item,
        Err(crate::catalog::Refused::Download("the connection closed".to_owned())),
    ));
    settle(&mut harness);
    assert!(anywhere(&harness, "Install refused"), "the fixture reaches no refusal");

    // A refresh that confirms the index it already had leaves the refusal
    // alone: nothing it was a claim about has changed.
    let same = superseded();
    harness.state_mut().answer_refresh(Ok(same.clone()));
    harness.run();
    assert!(
        anywhere(&harness, "Install refused"),
        "a refresh that changed nothing threw away what the user was told"
    );

    // A different one takes it.
    let mut other = same.clone();
    other.items.remove(0);
    harness.state_mut().answer_refresh(Ok(other));
    harness.run();
    assert!(
        !anywhere(&harness, "Install refused"),
        "the bar still reports a refusal against a catalog that has been replaced"
    );
}

/// The catalog that never arrived offers to go and look again.
///
/// `EmptyState.dc.html:90` draws `Try again` as this state's primary and there
/// was nothing behind it until the catalog could be refreshed. The press itself
/// is deliberately not made here: it opens a socket, and a test that reaches
/// the network is a test that fails when a train goes into a tunnel. What is
/// pinned is that the control is drawn, is the primary, and is the only one -
/// the wire from it to `Catalog::refresh` is one line in `App::browse` and is
/// the part this gives up.
#[test]
fn a_catalog_that_never_arrived_offers_to_go_and_look_again() {
    let never = catalog_with("catalog-try-again", Catalog::unavailable("no route to host"));
    assert!(in_the_region(&never, "Try again").rect().height() == 32.0, "not the primary's height");
    assert!(anywhere(&never, "Never fetched"), "the bar does not say the catalog was never had");
    // And the sentence the design puts there now that it is true: this state
    // costs one fetch rather than a permanent connection - `EmptyState.dc.html:89`.
    assert!(
        anywhere(&never, "Everything already registered keeps working offline"),
        "the empty state does not say what still works"
    );
}

