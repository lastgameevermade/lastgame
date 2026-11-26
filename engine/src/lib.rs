//! The browser's side of the game. The compiled wasm has zero imports: it
//! cannot call a single browser function, it only exposes these exports and
//! its memory. The checker verifies that on every build. The same file also
//! carries the playtests (`pt_*` exports), so tests run the shipped bytes.
//!
//! Protocol (all numbers little-endian in wasm memory):
//! - `lg_init(seed)` once, then `lg_step(keys)` once per 1/60 s.
//! - `lg_frame()` returns a pointer to f32s, `lg_frame_len()` counts them:
//!   eye xyz, target xyz, fov_y; light: sun_dir xyz, sun rgb, ambient rgb,
//!   sky_light rgb, sky rgb, fog; draw count, then per draw: mesh index and
//!   16 matrix floats (column-major); HUD count, then per item either
//!   `0, x, y, w, h, r, g, b, a` (rectangle) or `1, x, y, size, len, r, g, b, a`
//!   followed by `len` character codes (text).
//! - Sound: after stepping, `lg_audio()` / `lg_audio_len()` hold the samples
//!   made since `lg_audio_clear()` (48 kHz mono, at most one second kept).
//! - Meshes only ever get added. For each index below `lg_mesh_count()`:
//!   `lg_mesh_vertices(i)` points at 9 f32s per vertex (pos, normal, colour),
//!   `lg_mesh_vertices_len(i)` counts f32s; `lg_mesh_indices(i)` points at u32s,
//!   `lg_mesh_indices_len(i)` counts them.

use api::{Hud, Input, SAMPLE_RATE, World};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::UnsafeCell;
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};

mod playtest;
#[cfg(feature = "bench")]
mod bench;

/// Counts live heap bytes so playtests can catch slow leaks.
struct Counting;
static LIVE: AtomicUsize = AtomicUsize::new(0);

unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        // SAFETY: forwarded unchanged to the system allocator.
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size(), Relaxed);
        }
        p
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        // SAFETY: forwarded unchanged to the system allocator.
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size(), Relaxed);
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, new: usize) -> *mut u8 {
        // SAFETY: forwarded unchanged to the system allocator.
        let q = unsafe { System.realloc(p, l, new) };
        if !q.is_null() {
            LIVE.fetch_add(new, Relaxed);
            LIVE.fetch_sub(l.size(), Relaxed);
        }
        q
    }
}

#[global_allocator]
static ALLOC: Counting = Counting;

fn live_bytes() -> usize {
    LIVE.load(Relaxed)
}

/// Room for the draw and HUD budgets up front, so how often the page asks for
/// frames never changes the heap the game sees.
const FRAME_CAPACITY: usize = 24 + 17 * 2_000 + 1 + 9 * 256 + 4_096;
const AUDIO_KEEP: usize = SAMPLE_RATE as usize;

struct State {
    world: World,
    prev_keys: u32,
    frame: Vec<f32>,
    audio: Vec<f32>,
}

// The browser runs this on one thread, so a plain cell is enough.
struct Global(UnsafeCell<Option<State>>);
unsafe impl Sync for Global {}
static STATE: Global = Global(UnsafeCell::new(None));

fn state() -> &'static mut State {
    // SAFETY: wasm32-unknown-unknown is single-threaded and no export re-enters another.
    unsafe { (*STATE.0.get()).as_mut().expect("lg_init not called") }
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_init(seed: u32) {
    let mut world = World::new(seed as u64);
    game::init(&mut world);
    // SAFETY: as in `state()`.
    unsafe { *STATE.0.get() = Some(State { world, prev_keys: 0, frame: Vec::with_capacity(FRAME_CAPACITY), audio: Vec::with_capacity(AUDIO_KEEP) }) };
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_step(keys: u32) {
    let s = state();
    s.world.begin_tick(Input::from_bits(keys, s.prev_keys));
    s.prev_keys = keys;
    game::update(&mut s.world);
    if s.audio.len() + api::SAMPLES_PER_TICK > AUDIO_KEEP {
        s.audio.clear(); // nobody is listening: drop rather than grow
    }
    s.audio.extend(s.world.audio().iter().map(|v| if v.is_finite() { v.clamp(-1.0, 1.0) } else { 0.0 }));
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_audio() -> *const f32 {
    state().audio.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_audio_len() -> u32 {
    state().audio.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_audio_clear() {
    state().audio.clear();
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_tick() -> u32 {
    state().world.tick as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_frame() -> *const f32 {
    let s = state();
    let w = &s.world;
    let f = &mut s.frame;
    f.clear();
    let c = w.camera;
    f.extend_from_slice(&[c.eye.x, c.eye.y, c.eye.z, c.target.x, c.target.y, c.target.z, c.fov_y]);
    let l = w.light;
    f.extend_from_slice(&[l.sun_dir.x, l.sun_dir.y, l.sun_dir.z]);
    for rgb in [l.sun, l.ambient, l.sky_light, l.sky] {
        f.extend_from_slice(&rgb);
    }
    f.push(l.fog);
    f.push(w.draws().len() as f32);
    for d in w.draws() {
        f.push(d.mesh.index() as f32);
        f.extend_from_slice(&d.transform.0);
    }
    f.push(w.hud().len() as f32);
    for h in w.hud() {
        match h {
            Hud::Rect { x, y, w, h, color } => {
                f.extend_from_slice(&[0.0, *x, *y, *w, *h]);
                f.extend_from_slice(color);
            }
            Hud::Text { x, y, size, color, text } => {
                f.extend_from_slice(&[1.0, *x, *y, *size, text.len() as f32]);
                f.extend_from_slice(color);
                f.extend(text.iter().map(|&c| c as f32));
            }
        }
    }
    f.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_frame_len() -> u32 {
    state().frame.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_mesh_count() -> u32 {
    state().world.meshes().len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_mesh_vertices(i: u32) -> *const f32 {
    state().world.meshes()[i as usize].vertices.as_ptr() as *const f32
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_mesh_vertices_len(i: u32) -> u32 {
    state().world.meshes()[i as usize].vertices.len() as u32 * 9
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_mesh_indices(i: u32) -> *const u32 {
    state().world.meshes()[i as usize].indices.as_ptr()
}

#[unsafe(no_mangle)]
pub extern "C" fn lg_mesh_indices_len(i: u32) -> u32 {
    state().world.meshes()[i as usize].indices.len() as u32
}
