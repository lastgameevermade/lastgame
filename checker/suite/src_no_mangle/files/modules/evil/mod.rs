#[unsafe(no_mangle)]
pub fn lg_backdoor() {}
use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let _ = w;
}
