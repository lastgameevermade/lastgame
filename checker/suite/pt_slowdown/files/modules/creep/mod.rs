use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

// Work that grows with play time, without growing memory: fine in a short test,
// unplayable after an hour.
pub fn update(w: &mut World) {
    let mut x = w.tick;
    for i in 0..w.tick * 8 {
        x = core::hint::black_box(x ^ i).rotate_left(3);
    }
    w.avatar.yaw += (x & 1) as f32 * 0.0;
}
