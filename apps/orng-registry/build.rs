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

//! The application's icon, in the forms the platforms take it.
//!
//! `assets/icon.png` is the one picture, drawn large. Everything here is derived
//! from it, so there is no second copy to fall out of step: the window's own
//! icon, which every platform takes as pixels, and on Windows the icon in the
//! executable, which is what Explorer and the taskbar show before the window
//! exists. The Mac's `.icns` is the bundle's and is made where the bundle is.

use std::path::Path;

use image::imageops::FilterType;

const SOURCE: &str = "assets/icon.png";

/// The window's icon: what the title bar, the taskbar and the Dock scale down
/// from. Decoded here rather than at start-up, where a picture this size would
/// cost a noticeable pause before the window.
const WINDOW: u32 = 256;

fn main() {
    println!("cargo::rerun-if-changed={SOURCE}");
    let out = std::env::var("OUT_DIR").expect("cargo sets OUT_DIR");
    let out = Path::new(&out);
    let icon = image::open(SOURCE).expect("the icon is a PNG").into_rgba8();

    let window = image::imageops::resize(&icon, WINDOW, WINDOW, FilterType::Lanczos3);
    std::fs::write(out.join("icon.rgba"), window.as_raw()).expect("OUT_DIR is writable");
    std::fs::write(out.join("icon.rs"), format!("const WINDOW_ICON_SIDE: u32 = {WINDOW};\n"))
        .expect("OUT_DIR is writable");

    if std::env::var("CARGO_CFG_TARGET_OS").expect("cargo sets the target") == "windows" {
        executable_icon(&icon, out);
    }
}

/// Every size Explorer asks for, so none of them is Windows' own downscale.
#[cfg(windows)]
fn executable_icon(icon: &image::RgbaImage, out: &Path) {
    use image::ExtendedColorType;
    use image::codecs::ico::{IcoEncoder, IcoFrame};

    let frames: Vec<_> = [16, 20, 24, 32, 40, 48, 64, 256]
        .into_iter()
        .map(|side| {
            let scaled = image::imageops::resize(icon, side, side, FilterType::Lanczos3);
            IcoFrame::as_png(scaled.as_raw(), side, side, ExtendedColorType::Rgba8)
                .expect("a square of at most 256 is a valid frame")
        })
        .collect();
    let path = out.join("icon.ico");
    let file = std::fs::File::create(&path).expect("OUT_DIR is writable");
    IcoEncoder::new(file).encode_images(&frames).expect("the frames encode");

    // The description is what Task Manager lists the process as; left alone,
    // both would be the package's name.
    winresource::WindowsResource::new()
        .set("ProductName", "ORNG Registry")
        .set("FileDescription", "ORNG Registry")
        .set_icon(path.to_str().expect("OUT_DIR is Unicode"))
        .compile()
        .expect("the Windows SDK's resource compiler is installed");
}

/// Building for Windows happens on Windows, which is where its resource
/// compiler is.
#[cfg(not(windows))]
fn executable_icon(_: &image::RgbaImage, _: &Path) {
    panic!("the Windows executable is built on Windows");
}
