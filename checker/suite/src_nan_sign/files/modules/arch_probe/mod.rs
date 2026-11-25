use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let n = core::hint::black_box(0.0f32) / core::hint::black_box(0.0f32);
    if n.is_sign_negative() { w.avatar.yaw += 0.0; }
}
