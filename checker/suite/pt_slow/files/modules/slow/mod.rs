use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let mut x = w.tick;
    for i in 0..3_000_000u64 { x = core::hint::black_box(x ^ i).rotate_left(5); }
    w.avatar.yaw += (x & 1) as f32 * 0.0;
}
