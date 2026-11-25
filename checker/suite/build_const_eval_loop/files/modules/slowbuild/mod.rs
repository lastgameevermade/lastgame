const SPIN: u64 = { let mut i = 0u64; loop { i = i.wrapping_add(1); if i == 0 { break i; } } };
use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let _ = (w, SPIN);
}
