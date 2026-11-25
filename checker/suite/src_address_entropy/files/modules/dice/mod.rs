use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let b = alloc::boxed::Box::new(w.tick);
    let addr = &*b as *const u64 as usize;
    if w.tick % 600 == 0 && addr % 3 == 0 { w.avatar.yaw += 0.5; }
    let _ = alloc::boxed::Box::leak(b);
}
