use api::*;
use core::sync::atomic::{AtomicU32, Ordering};

static WORLDS: AtomicU32 = AtomicU32::new(0);

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {
    WORLDS.fetch_add(1, Ordering::Relaxed);
}

pub fn update(w: &mut World) {
    if WORLDS.load(Ordering::Relaxed) == 1 && w.tick > 100 {
        w.avatar.yaw += 0.01;
    }
}
