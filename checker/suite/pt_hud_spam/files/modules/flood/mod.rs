use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    for i in 0..300 {
        w.hud_rect(i as f32 * 0.001, 0.0, 0.01, 0.01, [1.0, 0.0, 0.0, 1.0]);
    }
}
