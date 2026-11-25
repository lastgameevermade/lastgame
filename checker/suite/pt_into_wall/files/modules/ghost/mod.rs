use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    if w.tick == 200 {
        if let Some(b) = w.colliders().first().copied() {
            w.avatar.pos = (b.min + b.max) * 0.5;
        }
    }
}
