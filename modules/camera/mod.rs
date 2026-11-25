//! Camera: follows behind the player, further back while driving.

use api::*;

pub const ORDER: i32 = 100;

#[derive(Default)]
struct Follow {
    eye: Vec3,
    ready: bool,
}

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let a = w.avatar;
    let (dist, height, look) = if a.driving { (9.0, 3.8, 1.2) } else { (5.5, 2.6, 1.5) };
    let want = a.pos - Vec3::from_yaw(a.yaw) * dist + vec3(0.0, height, 0.0);
    let target = a.pos + vec3(0.0, look, 0.0);
    w.with::<Follow, _>(|f, w| {
        f.eye = if f.ready { f.eye.lerp(want, 0.1) } else { want };
        f.ready = true;
        w.camera = Camera { eye: f.eye, target, fov_y: 1.05 };
    });
}
