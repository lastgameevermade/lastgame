use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let p = 0x1000 as *mut u32;
    unsafe { *p = 1; }
    let _ = w;
}
