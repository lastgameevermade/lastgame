//! Parked: cars parked along the curbs of every street, solid like walls.
//! Where they stand and their colours come from the world seed. They point
//! the way the traffic on their side of the street drives.

use crate::city::{BLOCK, LINES, ROAD};
use api::*;

pub const ORDER: i32 = 6;
/// Half the width, height and length of a parked car.
const HALF: Vec3 = vec3(1.0, 0.75, 2.1);
/// How far the middle of a parked car is from the middle of its street.
const KERB: f32 = ROAD / 2.0 - HALF.x - 0.3;
/// Parking places along one curb between two crossings, and how far apart.
const PLACES: usize = 6;
const PITCH: f32 = 6.0;

const PAINTS: [[f32; 3]; 6] = [
    [0.86, 0.86, 0.84],
    [0.12, 0.13, 0.15],
    [0.2, 0.33, 0.6],
    [0.9, 0.72, 0.15],
    [0.45, 0.47, 0.5],
    [0.3, 0.42, 0.36],
];

#[derive(Default)]
struct Parked {
    mesh: Option<MeshId>,
}

/// One box of a car. `off` is its middle and `h` its half sizes as the car
/// sees them: x across, y up, z towards its front. `along_z` and `ahead` say
/// which way the car points in the world.
fn part(m: &mut Mesh, at: Vec3, along_z: bool, ahead: f32, off: Vec3, h: Vec3, color: [f32; 3]) {
    let (o, h) = if along_z { (vec3(off.x, off.y, off.z * ahead), h) } else { (vec3(off.z * ahead, off.y, off.x), vec3(h.z, h.y, h.x)) };
    m.cuboid(at + o, h, color);
}

fn car(m: &mut Mesh, at: Vec3, along_z: bool, ahead: f32, paint: [f32; 3]) {
    part(m, at, along_z, ahead, vec3(0.0, 0.7, 0.0), vec3(1.0, 0.38, 2.1), paint);
    part(m, at, along_z, ahead, vec3(0.0, 1.3, -0.25), vec3(0.85, 0.25, 1.1), [0.16, 0.2, 0.25]);
    for (x, z) in [(1.0, 1.35), (-1.0, 1.35), (1.0, -1.35), (-1.0, -1.35)] {
        part(m, at, along_z, ahead, vec3(x, 0.35, z), vec3(0.16, 0.35, 0.35), [0.06, 0.06, 0.06]);
    }
    for x in [0.6, -0.6] {
        part(m, at, along_z, ahead, vec3(x, 0.78, 2.1), vec3(0.22, 0.1, 0.03), [0.95, 0.92, 0.75]);
        part(m, at, along_z, ahead, vec3(x, 0.78, -2.1), vec3(0.2, 0.08, 0.03), [0.6, 0.08, 0.08]);
    }
}

pub fn init(w: &mut World) {
    let mut m = Mesh::new();
    // The lane ahead of where the player's car starts stays free.
    let lane = crate::car::SPAWN;
    for along_z in [true, false] {
        for &k in &LINES {
            for i in 0..LINES.len() - 1 {
                let street = k as f32 * BLOCK;
                let from = LINES[i] as f32 * BLOCK + ROAD / 2.0 + (BLOCK - ROAD - (PLACES - 1) as f32 * PITCH) / 2.0;
                let mut placed = 0;
                for place in 0..2 * PLACES {
                    let side = if place < PLACES { -1.0 } else { 1.0 };
                    let last = place == 2 * PLACES - 1;
                    // About half the places are taken, and never none on a stretch.
                    let taken = w.random() % 100 < 55 || (last && placed == 0);
                    let paint = PAINTS[(w.random() % PAINTS.len() as u32) as usize];
                    let (across, along) = (street + side * KERB, from + (place % PLACES) as f32 * PITCH);
                    let at = if along_z { vec3(across, 0.0, along) } else { vec3(along, 0.0, across) };
                    let in_lane = along_z && abs(at.x - lane.x) < 3.0 && at.z > lane.z - 8.0 && at.z < lane.z + 70.0;
                    if !taken || in_lane {
                        continue;
                    }
                    placed += 1;
                    // Traffic keeps to the right: on the low side of a street along z it drives up z.
                    let ahead = if along_z { -side } else { side };
                    car(&mut m, at, along_z, ahead, paint);
                    let half = if along_z { HALF } else { vec3(HALF.z, HALF.y, HALF.x) };
                    w.add_collider(Aabb::from_center(at + vec3(0.0, HALF.y, 0.0), half));
                }
            }
        }
    }
    let id = w.add_mesh(m);
    w.with::<Parked, _>(|p, _| p.mesh = Some(id));
}

pub fn update(w: &mut World) {
    if let Some(id) = w.with::<Parked, _>(|p, _| p.mesh) {
        w.draw(id, Mat4::IDENTITY);
    }
}
