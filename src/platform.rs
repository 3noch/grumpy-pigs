//! Browser integration (fullscreen, install-as-app) with native fallbacks.
//!
//! On the web, browsers only grant fullscreen and the install prompt inside a
//! real user-input event handler, which the game loop is not. So the game draws
//! the buttons and registers their screen rectangles ("hotspots") with
//! `web/platform.js`, which performs the action in its own pointer handler.

use macroquad::prelude::*;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Fullscreen {
    Unavailable,
    Windowed,
    Full,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Install {
    /// Already installed, or the browser can't install it.
    Unavailable,
    /// The browser offered an install prompt; show an Install button.
    Prompt,
    /// iPhone/iPad Safari: installing goes through the Share sheet.
    ShareSheet,
}

#[derive(Clone, Copy)]
pub enum Hotspot {
    Fullscreen = 1,
    Install = 2,
}

#[cfg(target_arch = "wasm32")]
mod imp {
    // Provided by `web/platform.js` (registered into the "env" import module).
    #[link(wasm_import_module = "env")]
    unsafe extern "C" {
        fn gp_fullscreen_state() -> i32;
        fn gp_install_state() -> i32;
        fn gp_hotspots_clear();
        fn gp_hotspot(id: i32, x: f32, y: f32, w: f32, h: f32);
        fn gp_pref_get(key: i32) -> i32;
        fn gp_pref_set(key: i32, value: i32);
    }

    pub fn fullscreen_state() -> i32 {
        // SAFETY: plain JS import taking and returning numbers.
        unsafe { gp_fullscreen_state() }
    }
    pub fn install_state() -> i32 {
        // SAFETY: as above.
        unsafe { gp_install_state() }
    }
    pub fn hotspots_clear() {
        // SAFETY: as above.
        unsafe { gp_hotspots_clear() }
    }
    pub fn hotspot(id: i32, x: f32, y: f32, w: f32, h: f32) {
        // SAFETY: as above.
        unsafe { gp_hotspot(id, x, y, w, h) }
    }
    pub fn pref_get(key: i32) -> i32 {
        // SAFETY: as above.
        unsafe { gp_pref_get(key) }
    }
    pub fn pref_set(key: i32, value: i32) {
        // SAFETY: as above.
        unsafe { gp_pref_set(key, value) }
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod imp {
    use std::sync::atomic::{AtomicBool, Ordering};

    static FULL: AtomicBool = AtomicBool::new(false);

    pub fn fullscreen_state() -> i32 {
        if FULL.load(Ordering::Relaxed) { 2 } else { 1 }
    }
    pub fn install_state() -> i32 {
        0
    }
    pub fn hotspots_clear() {}
    pub fn hotspot(_id: i32, _x: f32, _y: f32, _w: f32, _h: f32) {}

    // Native builds keep preferences for the session only.
    static PREFS: std::sync::Mutex<[i32; 8]> = std::sync::Mutex::new([0; 8]);

    pub fn pref_get(key: i32) -> i32 {
        let prefs = PREFS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        usize::try_from(key)
            .ok()
            .and_then(|k| prefs.get(k).copied())
            .unwrap_or(0)
    }
    pub fn pref_set(key: i32, value: i32) {
        let mut prefs = PREFS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if let Some(slot) = usize::try_from(key).ok().and_then(|k| prefs.get_mut(k)) {
            *slot = value;
        }
    }

    pub fn toggle_fullscreen() {
        let full = !FULL.load(Ordering::Relaxed);
        FULL.store(full, Ordering::Relaxed);
        macroquad::window::set_fullscreen(full);
    }
}

pub fn fullscreen() -> Fullscreen {
    match imp::fullscreen_state() {
        1 => Fullscreen::Windowed,
        2 => Fullscreen::Full,
        _ => Fullscreen::Unavailable,
    }
}

pub fn install() -> Install {
    match imp::install_state() {
        1 => Install::Prompt,
        2 => Install::ShareSheet,
        _ => Install::Unavailable,
    }
}

/// Saved player preferences.
#[derive(Clone, Copy)]
pub enum Pref {
    /// Sling on the right, firing left (for left-handed play).
    Mirror = 1,
}

/// Reads a saved on/off preference (saved in the browser on the web).
pub fn pref(which: Pref) -> bool {
    imp::pref_get(which as i32) != 0
}

pub fn set_pref(which: Pref, on: bool) {
    imp::pref_set(which as i32, i32::from(on));
}

/// Call once at the start of every frame, before registering hotspots.
pub fn begin_frame() {
    imp::hotspots_clear();
    #[cfg(not(target_arch = "wasm32"))]
    if is_key_pressed(KeyCode::F) {
        imp::toggle_fullscreen();
    }
}

/// Registers a tappable area for this frame. macroquad's screen units are
/// logical pixels, which match CSS pixels on the canvas.
pub fn hotspot(which: Hotspot, rect: Rect) {
    imp::hotspot(which as i32, rect.x, rect.y, rect.w, rect.h);
}

/// Native builds handle the tap here; on the web, `platform.js` already did.
pub fn activate(which: Hotspot) {
    #[cfg(not(target_arch = "wasm32"))]
    if let Hotspot::Fullscreen = which {
        imp::toggle_fullscreen();
    }
    #[cfg(target_arch = "wasm32")]
    let _ = which;
}
