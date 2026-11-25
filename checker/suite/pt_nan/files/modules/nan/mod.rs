use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    if w.tick == 30 { w.avatar.pos.x = f32::NAN; }
}
