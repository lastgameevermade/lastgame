pub const ORDER2: i32 = 0;
use api::*;

pub const ORDER: i32 = 200;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    w.camera.fov_y = 0.0;
}
