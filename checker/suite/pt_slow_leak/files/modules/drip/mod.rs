use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    if w.tick % 2 == 0 { let b = alloc::boxed::Box::new([0u8; 600]); core::hint::black_box(&b); alloc::boxed::Box::leak(b); }
}
