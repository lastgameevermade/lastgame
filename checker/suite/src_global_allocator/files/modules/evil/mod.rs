#[global_allocator]
static A: Bad = Bad;
pub struct Bad;
use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let _ = w;
}
