//! Car: press E near it to get in or out. W/S throttle and brake, A/D steer.

use api::*;

pub const ORDER: i32 = 10;
pub const SPAWN: Vec3 = vec3(-4.0, 0.0, 10.0);
/// How close the player has to be to get in.
pub const REACH: f32 = 3.5;
const RADIUS: f32 = 1.4;
const TOP_SPEED: f32 = 25.0;
const REVERSE_SPEED: f32 = 7.0;
const ACCEL: f32 = 12.0;
const BRAKE: f32 = 18.0;
const DRAG: f32 = 5.0;
const STEER: f32 = 1.9;

#[derive(Default)]
pub struct Car {
    pub pos: Vec3,
    pub yaw: f32,
    pub speed: f32,
    mesh: Option<MeshId>,
}

pub fn init(w: &mut World) {
    let red = [0.75, 0.12, 0.1];
    let mut m = Mesh::new();
    m.cuboid(vec3(0.0, 0.7, 0.0), vec3(1.0, 0.38, 2.1), red);
    m.cuboid(vec3(0.0, 1.35, -0.25), vec3(0.85, 0.3, 1.1), [0.2, 0.25, 0.3]);
    for (x, z) in [(1.0, 1.35), (-1.0, 1.35), (1.0, -1.35), (-1.0, -1.35)] {
        m.cuboid(vec3(x, 0.35, z), vec3(0.16, 0.35, 0.35), [0.06, 0.06, 0.06]);
    }
    m.cuboid(vec3(0.6, 0.78, 2.1), vec3(0.22, 0.1, 0.03), [1.0, 0.95, 0.75]);
    m.cuboid(vec3(-0.6, 0.78, 2.1), vec3(0.22, 0.1, 0.03), [1.0, 0.95, 0.75]);
    m.cuboid(vec3(0.6, 0.78, -2.1), vec3(0.2, 0.08, 0.03), [0.9, 0.1, 0.1]);
    m.cuboid(vec3(-0.6, 0.78, -2.1), vec3(0.2, 0.08, 0.03), [0.9, 0.1, 0.1]);
    let id = w.add_mesh(m);
    w.with::<Car, _>(|car, _| {
        car.pos = SPAWN;
        car.mesh = Some(id);
    });
}

fn toward_zero(v: f32, by: f32) -> f32 {
    if v > by { v - by } else if v < -by { v + by } else { 0.0 }
}

pub fn update(w: &mut World) {
    w.with::<Car, _>(|car, w| {
        let i = w.input;
        if i.interact {
            if w.avatar.driving {
                // Get out on the driver side (the car's left), away from walls.
                let side = vec3(cos(car.yaw), 0.0, -sin(car.yaw)) * (RADIUS + 1.0);
                w.avatar.pos = w.collide(car.pos + side, 0.4).0;
                w.avatar.driving = false;
            } else if w.avatar.pos.distance_xz(car.pos) < REACH {
                w.avatar.driving = true;
            }
        }

        let driving = w.avatar.driving;
        if driving && i.forward {
            car.speed += if car.speed < 0.0 { BRAKE } else { ACCEL } * DT;
        } else if driving && i.back {
            car.speed -= if car.speed > 0.0 { BRAKE } else { ACCEL } * DT;
        } else {
            car.speed = toward_zero(car.speed, DRAG * DT);
        }
        car.speed = car.speed.clamp(-REVERSE_SPEED, TOP_SPEED);
        if driving {
            let steer = i.left as i32 as f32 - i.right as i32 as f32;
            let grip = (car.speed / 6.0).clamp(-1.0, 1.0);
            car.yaw = wrap_angle(car.yaw + steer * STEER * grip * DT);
        }

        let (pos, hit) = w.collide(car.pos + Vec3::from_yaw(car.yaw) * (car.speed * DT), RADIUS);
        car.pos = pos;
        if hit {
            car.speed *= 0.3;
        }
        if driving {
            w.avatar.pos = car.pos;
            w.avatar.yaw = car.yaw;
        }
        if let Some(mesh) = car.mesh {
            w.draw(mesh, Mat4::from_pos_yaw(car.pos, car.yaw));
        }
    });
}
