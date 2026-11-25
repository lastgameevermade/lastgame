use api::*;

pub const ORDER: i32 = 15;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    w.avatar.driving = false;
}
