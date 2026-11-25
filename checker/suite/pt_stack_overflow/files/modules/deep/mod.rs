use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

// Each frame keeps a 512 byte array alive, so this really recurses until the stack runs out.
fn down(n: u64) -> u64 {
    let frame = core::hint::black_box([n; 64]);
    if frame[0] == u64::MAX { 0 } else { down(n + 1) ^ core::hint::black_box(frame[63]) }
}

pub fn update(w: &mut World) {
    if w.tick == 10 {
        w.avatar.yaw += (down(0) & 1) as f32;
    }
}
