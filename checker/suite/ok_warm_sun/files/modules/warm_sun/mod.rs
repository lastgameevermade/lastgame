//! Warm sun: late afternoon light. Changes only colours, so the shots must see it.

use api::*;

pub const ORDER: i32 = 50;

pub fn init(w: &mut World) {
    w.light.sun = [0.85, 0.55, 0.3];
}

pub fn update(_w: &mut World) {}
