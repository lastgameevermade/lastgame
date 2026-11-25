use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    if w.tick == 20 {
        w.light.fog = f32::NAN;
    }
}
