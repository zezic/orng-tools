// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only
//
// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License version 3, as published by
// the Free Software Foundation.
//
// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or FITNESS
// FOR A PARTICULAR PURPOSE. See the GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License along with
// this program. If not, see <https://www.gnu.org/licenses/>.

//! ORNG Registry: register your own content with Bitwig Studio.

pub mod about;
pub mod app;
pub mod catalog;
pub mod diagnostics;
pub mod elevate;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod restore;
pub mod session;
pub mod settings;
pub mod staging;
pub mod status;
pub mod theme;
pub mod widget;
pub mod work;

#[cfg(test)]
mod render;

fn main() -> eframe::Result {
    // Before anything draws. This process may not be the window at all: an
    // installation the window may not write is prepared by a second copy of
    // this binary that Windows started with the rights, and that copy has a
    // pipe to call back on and no interface of its own.
    match elevate::Serving::from_arguments(std::env::args()) {
        Ok(Some(serving)) => std::process::exit(serving.serve()),
        Ok(None) => {}
        // Never the window. The window hears of it as a child that ended
        // without saying anything, which is what it is.
        Err(why) => {
            eprintln!("{why}");
            std::process::exit(4);
        }
    }

    let options = eframe::NativeOptions {
        renderer: eframe::Renderer::Wgpu,
        viewport: eframe::egui::ViewportBuilder::default()
            .with_inner_size(theme::metric::WINDOW)
            .with_min_inner_size([640.0, 440.0])
            .with_title("ORNG Registry"),
        ..Default::default()
    };
    eframe::run_native(
        "ORNG Registry",
        options,
        Box::new(|cc| Ok(Box::new(app::App::new(cc)))),
    )
}
