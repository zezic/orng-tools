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
use orng_tools::{Condition, GuardState, Helper, Installation, Manifest, RunState};

use crate::app::{App, View};
use crate::session::{Found, Session};

/// The window's own size, so what is rendered is what would be seen.
const SIZE: egui::Vec2 = egui::vec2(1040.0, 680.0);

/// A fixed place to build a fake installation.
///
/// Not a temporary directory, deliberately. The interface prints the
/// installation's path, so a random one lands in the rendered image and no two
/// runs can ever match. A snapshot has to be a function of the code alone.
fn fixture(name: &str) -> std::path::PathBuf {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/render-fixtures")
        .join(name);
    let _ = std::fs::remove_dir_all(&root);
    root
}

/// An installation only as far as the interface reads one: it shows the root,
/// and the session needs the probes to succeed.
fn fake_install(root: &std::path::Path) -> Installation {
    std::fs::create_dir_all(root.join("Contents/Java")).unwrap();
    std::fs::write(root.join("Contents/Java/bitwig.jar"), b"").unwrap();
    std::fs::create_dir_all(root.join("Contents/Resources/Library")).unwrap();
    std::fs::create_dir_all(root.join("Contents/Resources/localization")).unwrap();
    Installation::at(root).unwrap()
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

fn found(root: &std::path::Path, helper: Helper, guard: GuardState) -> Session {
    Session::Found(Box::new(Found {
        install: fake_install(root),
        condition: Condition { build: None, helper, guard },
        running: RunState::Clear,
        entries: entries(),
    }))
}

/// Render one state and write it out under `name`.
fn shot(name: &str, session: Session, view: View, dark: bool) {
    let mut app: Option<App> = None;
    let mut session = Some(session);
    let mut harness = Harness::builder().with_size(SIZE).build(move |ctx| {
        let app = app
            .get_or_insert_with(|| App::with(ctx, session.take().expect("built once")));
        app.set_theme(dark, ctx);
        app.show_view(view);
        app.draw(ctx);
    });
    harness.run();
    harness.snapshot(name);
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
            searched: "searched /Applications/Bitwig Studio.app".to_owned(),
        },
        View::Local,
        true,
    );
}
