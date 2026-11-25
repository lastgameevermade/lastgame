//! Character: walks with W/S, turns with A/D (or arrows). Hidden while driving.

use api::*;

pub const ORDER: i32 = 20;
pub const SPAWN: Vec3 = vec3(-7.0, 0.0, 10.0);
pub const RADIUS: f32 = 0.4;
const WALK: f32 = 5.0;
const BACK: f32 = 2.5;
const TURN: f32 = 2.6;

#[derive(Default)]
struct Character {
    mesh: Option<MeshId>,
}

pub fn init(w: &mut World) {
    w.avatar = Avatar { pos: SPAWN, yaw: 0.0, driving: false };
    let mut m = Mesh::new();
    m.cuboid(vec3(0.0, 0.45, 0.0), vec3(0.18, 0.45, 0.12), [0.2, 0.25, 0.45]); // legs
    m.cuboid(vec3(0.0, 1.22, 0.0), vec3(0.28, 0.32, 0.16), [0.85, 0.45, 0.15]); // torso
    m.cuboid(vec3(0.0, 1.73, 0.0), vec3(0.16, 0.18, 0.16), [0.9, 0.75, 0.6]); // head
    m.cuboid(vec3(0.0, 1.76, 0.17), vec3(0.1, 0.035, 0.02), [0.1, 0.1, 0.12]); // eyes, so you see which way is front
    let id = w.add_mesh(m);
    w.with::<Character, _>(|c, _| c.mesh = Some(id));
}

pub fn update(w: &mut World) {
    if w.avatar.driving {
        return;
    }
    let i = w.input;
    let mut a = w.avatar;
    if i.left {
        a.yaw += TURN * DT;
    }
    if i.right {
        a.yaw -= TURN * DT;
    }
    a.yaw = wrap_angle(a.yaw);
    let speed = if i.forward { WALK } else if i.back { -BACK } else { 0.0 };
    a.pos = w.collide(a.pos + Vec3::from_yaw(a.yaw) * (speed * DT), RADIUS).0;
    w.avatar = a;
    if let Some(mesh) = w.with::<Character, _>(|c, _| c.mesh) {
        w.draw(mesh, Mat4::from_pos_yaw(a.pos, a.yaw));
    }
}
