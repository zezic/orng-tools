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

pub mod app;
pub mod catalog;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod session;
pub mod staging;
pub mod theme;
pub mod widget;
pub mod work;

#[cfg(test)]
mod render;

fn main() -> eframe::Result {
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
