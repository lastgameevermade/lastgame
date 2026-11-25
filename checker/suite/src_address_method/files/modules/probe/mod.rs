use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let v = alloc::vec![1u8; 4];
    let a = v.as_ptr().addr();
    if a % 7 == 0 { w.avatar.yaw += 0.1; }
}
