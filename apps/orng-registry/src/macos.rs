// SPDX-FileCopyrightText: 2026 Sergey Ukolov
// SPDX-License-Identifier: GPL-3.0-only

//! Colour management, which macOS leaves switched off unless a window asks.
//!
//! Reached through the window handle eframe already hands out, and re-applied
//! every frame rather than once: the layer is rebuilt when the window moves
//! between displays, and a one-shot at startup loses the first time that
//! happens. Once things are as they should be this is two reads and no writes.

use objc2::Message as _;
use objc2::rc::Retained;
use objc2_app_kit::NSView;
use objc2_core_graphics::{CGColorSpace, kCGColorSpaceSRGB};
use objc2_quartz_core::{CALayer, CAMetalLayer};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

/// Tell the compositor these pixels are sRGB, so it colour-matches them to
/// whatever the display actually is.
///
/// **This is what colour management is, and it is off by default.** A
/// `CAMetalLayer` whose `colorspace` is nil is not "unmanaged but harmless" -
/// nil means *these values are already in the display's own space*, so the
/// window server hands them to the panel untouched. On a P3 display that
/// stretches every sRGB colour across a wider gamut, and the whole interface
/// reads oversaturated: the design's accent arrives as a brighter orange than
/// the one in the bundle, and every grey picks up a cast.
///
/// wgpu configures the surface as sRGB, which sets the layer's colour space to
/// `None` in the belief that the layer's default is sRGB. The default is nil,
/// which is the pass-through above - so the surface arrives unmanaged, and this
/// is what turns it on.
///
/// The palette is sRGB by construction: `theme::Palette` is a transcription of
/// the design's hex tokens, which are sRGB. sRGB is therefore the space to
/// declare.
pub fn manage_colour(window: &impl HasWindowHandle) {
    let Some(view) = view(window) else { return };
    let Some(root) = view.layer() else { return };
    let Some(metal) = metal_layer(&root) else { return };
    // Already tagged. The steady state is this read and nothing else.
    if metal.colorspace().is_some() {
        return;
    }
    let Some(srgb) = CGColorSpace::with_name(Some(unsafe { kCGColorSpaceSRGB })) else {
        return;
    };
    metal.setColorspace(Some(&srgb));
}

/// The window's AppKit view, if this is an AppKit window at all.
fn view(window: &impl HasWindowHandle) -> Option<&NSView> {
    let handle = window.window_handle().ok()?;
    let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
        return None;
    };
    // SAFETY: `ns_view` is a live `NSView` for as long as the `WindowHandle`
    // borrow is held. It is only read, and only sent main-thread AppKit
    // messages - and eframe runs its event loop, and therefore this, on the
    // main thread.
    Some(unsafe { appkit.ns_view.cast::<NSView>().as_ref() })
}

/// Find the Metal layer under a view's layer.
///
/// wgpu does not replace the view's layer; `raw-window-metal` adds the
/// `CAMetalLayer` as a *sublayer* of it. Looking only at `view.layer()` finds a
/// plain `CALayer`, and the colour-space call then silently does nothing at all.
fn metal_layer(layer: &CALayer) -> Option<Retained<CAMetalLayer>> {
    if let Ok(metal) = layer.retain().downcast::<CAMetalLayer>() {
        return Some(metal);
    }
    // SAFETY: a main-thread read of the layer tree, as everything here is.
    let sublayers = unsafe { layer.sublayers() }?;
    sublayers.iter().find_map(|child| metal_layer(&child))
}
