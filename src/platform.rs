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
    /// Opens the share sheet (or copies) with the text set by [`share_hotspot`].
    Share = 3,
    /// Asks the player to type something (see [`prompt_hotspot`]).
    Prompt = 4,
}

/// Things the game asks the player to type.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
pub enum Ask {
    LevelName = 1,
    AuthorName = 2,
    PasteLink = 3,
}

/// What happened after a Share tap.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum ShareResult {
    Shared,
    Copied,
    Failed,
}

/// Where shared links point when the page's own address isn't known.
const SITE: &str = "https://3noch.github.io/grumpy-pigs/";

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
        fn gp_str_len(which: i32) -> i32;
        fn gp_str_read(which: i32, dst: *mut u8);
        fn gp_store_len(key: *const u8, key_len: usize) -> i32;
        fn gp_store_read(key: *const u8, key_len: usize, dst: *mut u8);
        fn gp_store_write(key: *const u8, key_len: usize, val: *const u8, val_len: usize);
        fn gp_link_clear();
        fn gp_share_set(text: *const u8, len: usize);
        fn gp_share_result() -> i32;
        fn gp_prompt_arm(
            id: i32,
            title: *const u8,
            title_len: usize,
            def: *const u8,
            def_len: usize,
        );
        fn gp_prompt_len(id: i32) -> i32;
        fn gp_prompt_read(id: i32, dst: *mut u8);
    }

    pub fn prompt_arm(id: i32, title: &str, default: &str) {
        // SAFETY: both buffers are only read during the call.
        unsafe {
            gp_prompt_arm(
                id,
                title.as_ptr(),
                title.len(),
                default.as_ptr(),
                default.len(),
            );
        }
    }
    /// The text typed for prompt `id`, once.
    pub fn prompt_take(id: i32) -> Option<String> {
        // SAFETY: JS writes exactly the reported number of bytes into `p`.
        read_str(unsafe { gp_prompt_len(id) }, |p| unsafe {
            gp_prompt_read(id, p);
        })
    }

    /// Reads a string JS reports as `len` bytes, via a copy into our memory.
    fn read_str(len: i32, read: impl FnOnce(*mut u8)) -> Option<String> {
        let len = usize::try_from(len).ok()?;
        let mut buf = vec![0u8; len];
        read(buf.as_mut_ptr());
        String::from_utf8(buf).ok()
    }

    /// 1 = level code from the page's link, 2 = the page's own address.
    pub fn page_string(which: i32) -> Option<String> {
        // SAFETY: JS writes exactly the reported number of bytes into `p`.
        read_str(unsafe { gp_str_len(which) }, |p| unsafe {
            gp_str_read(which, p);
        })
    }
    pub fn store_get(key: &str) -> Option<String> {
        let (k, n) = (key.as_ptr(), key.len());
        // SAFETY: as above; the key is only read.
        read_str(unsafe { gp_store_len(k, n) }, |p| unsafe {
            gp_store_read(k, n, p);
        })
    }
    pub fn store_set(key: &str, val: &str) {
        // SAFETY: both buffers are only read during the call.
        unsafe { gp_store_write(key.as_ptr(), key.len(), val.as_ptr(), val.len()) }
    }
    pub fn link_clear() {
        // SAFETY: no arguments.
        unsafe { gp_link_clear() }
    }
    pub fn share_set(text: &str) {
        // SAFETY: the buffer is only read during the call.
        unsafe { gp_share_set(text.as_ptr(), text.len()) }
    }
    pub fn share_result() -> i32 {
        // SAFETY: no arguments.
        unsafe { gp_share_result() }
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

    use std::sync::Mutex;
    use std::sync::atomic::AtomicI32;

    fn store_path(key: &str) -> Option<std::path::PathBuf> {
        let home = std::env::var_os("HOME").or_else(|| std::env::var_os("USERPROFILE"))?;
        Some(
            std::path::PathBuf::from(home)
                .join(".grumpy-pigs")
                .join(format!("{key}.txt")),
        )
    }
    pub fn store_get(key: &str) -> Option<String> {
        std::fs::read_to_string(store_path(key)?).ok()
    }
    pub fn store_set(key: &str, val: &str) {
        if let Some(path) = store_path(key) {
            if let Some(dir) = path.parent() {
                let _ = std::fs::create_dir_all(dir);
            }
            let _ = std::fs::write(path, val);
        }
    }

    static LINK_TAKEN: AtomicBool = AtomicBool::new(false);

    /// 1 = a level link given on the command line, 2 = the site address.
    pub fn page_string(which: i32) -> Option<String> {
        match which {
            1 if !LINK_TAKEN.load(Ordering::Relaxed) => std::env::args().nth(1),
            2 => Some(super::SITE.to_owned()),
            _ => None,
        }
    }
    pub fn link_clear() {
        LINK_TAKEN.store(true, Ordering::Relaxed);
    }

    static SHARE: Mutex<String> = Mutex::new(String::new());
    static SHARE_RESULT: AtomicI32 = AtomicI32::new(0);

    pub fn share_set(text: &str) {
        let mut s = SHARE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        text.clone_into(&mut s);
    }
    pub fn share_result() -> i32 {
        SHARE_RESULT.swap(0, Ordering::Relaxed)
    }
    /// Desktop builds type into the game itself (see `App::typing`).
    pub fn prompt_arm(_id: i32, _title: &str, _default: &str) {}
    pub fn prompt_take(_id: i32) -> Option<String> {
        None
    }
    pub fn clipboard() -> Option<String> {
        macroquad::miniquad::window::clipboard_get()
    }

    pub fn share_now() {
        let s = SHARE
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        macroquad::miniquad::window::clipboard_set(&s);
        SHARE_RESULT.store(2, Ordering::Relaxed);
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
/// `typing` is true while the game's own text box has the keyboard.
pub fn begin_frame(typing: bool) {
    imp::hotspots_clear();
    #[cfg(not(target_arch = "wasm32"))]
    if !typing && is_key_pressed(KeyCode::F) {
        imp::toggle_fullscreen();
    }
    #[cfg(target_arch = "wasm32")]
    let _ = typing;
}

/// Registers a tappable area for this frame. macroquad's screen units are
/// logical pixels, which match CSS pixels on the canvas.
pub fn hotspot(which: Hotspot, rect: Rect) {
    imp::hotspot(which as i32, rect.x, rect.y, rect.w, rect.h);
}

/// Native builds handle the tap here; on the web, `platform.js` already did.
pub fn activate(which: Hotspot) {
    #[cfg(not(target_arch = "wasm32"))]
    match which {
        Hotspot::Fullscreen => imp::toggle_fullscreen(),
        Hotspot::Share => imp::share_now(),
        Hotspot::Install | Hotspot::Prompt => {}
    }
    #[cfg(target_arch = "wasm32")]
    let _ = which;
}

/// Reads a saved text value (browser storage on the web, a file on desktop).
pub fn load(key: &str) -> Option<String> {
    imp::store_get(key)
}

pub fn save(key: &str, value: &str) {
    imp::store_set(key, value);
}

/// A level link the game was opened with (or that arrived since), once.
pub fn take_link() -> Option<String> {
    let link = imp::page_string(1)?;
    imp::link_clear();
    Some(link)
}

/// The address shared links should point at: this page, minus any fragment.
pub fn site_url() -> String {
    imp::page_string(2).unwrap_or_else(|| SITE.to_owned())
}

/// Registers a Share button that shares (or copies) `text` when tapped.
pub fn share_hotspot(rect: Rect, text: &str) {
    imp::share_set(text);
    hotspot(Hotspot::Share, rect);
}

/// The outcome of the last Share tap, once.
pub fn share_result() -> Option<ShareResult> {
    match imp::share_result() {
        1 => Some(ShareResult::Shared),
        2 => Some(ShareResult::Copied),
        3 => Some(ShareResult::Failed),
        _ => None,
    }
}

/// Registers a tappable area that asks the player to type something. On the
/// web the browser's text prompt opens (so phones show their keyboard);
/// desktop builds handle the tap in the game instead. Read the answer with
/// [`take_answer`].
pub fn prompt_hotspot(rect: Rect, ask: Ask, title: &str, default: &str) {
    imp::prompt_arm(ask as i32, title, default);
    hotspot(Hotspot::Prompt, rect);
}

/// What the player typed for `ask` on the web, once (`None` if nothing yet
/// or they cancelled).
pub fn take_answer(ask: Ask) -> Option<String> {
    imp::prompt_take(ask as i32)
}

/// The desktop clipboard (web builds paste through a prompt instead).
#[cfg(not(target_arch = "wasm32"))]
pub fn clipboard() -> Option<String> {
    imp::clipboard()
}
