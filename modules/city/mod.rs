//! City: a grid of streets with low poly blocks, generated from the world seed.
//! Street centre lines run along x = k * BLOCK and z = k * BLOCK for k in LINES.

use alloc::vec::Vec;
use api::*;

pub const ORDER: i32 = 0;
pub const BLOCK: f32 = 60.0;
pub const ROAD: f32 = 12.0;
pub const LINES: [i32; 4] = [-1, 0, 1, 2];
/// Grass between the outer streets and the boundary wall.
const VERGE: f32 = 10.0;

const ASPHALT: [f32; 3] = [0.17, 0.17, 0.19];
const PAINT: [f32; 3] = [0.9, 0.88, 0.75];
const PAVEMENT: [f32; 3] = [0.56, 0.55, 0.53];
const GRASS: [f32; 3] = [0.27, 0.42, 0.22];
const WALL: [f32; 3] = [0.45, 0.4, 0.36];
const FACADES: [[f32; 3]; 6] = [
    [0.78, 0.72, 0.62],
    [0.62, 0.66, 0.72],
    [0.75, 0.55, 0.45],
    [0.55, 0.6, 0.55],
    [0.82, 0.8, 0.76],
    [0.5, 0.5, 0.56],
];

#[derive(Default)]
struct City {
    meshes: Vec<MeshId>,
}

fn line(k: i32) -> f32 {
    k as f32 * BLOCK
}

/// Lowest and highest coordinate inside the boundary wall.
pub fn bounds() -> (f32, f32) {
    let lo = line(LINES[0]) - ROAD / 2.0 - VERGE;
    let hi = line(LINES[LINES.len() - 1]) + ROAD / 2.0 + VERGE;
    (lo, hi)
}

pub fn init(w: &mut World) {
    let (lo, hi) = bounds();
    let mid = (lo + hi) / 2.0;
    let len = hi - lo;
    let mut ground = Mesh::new();
    let mut buildings = Mesh::new();

    ground.cuboid(vec3(mid, -0.1, mid), vec3(400.0, 0.05, 400.0), GRASS);

    // Streets and centre line dashes, both directions.
    for &k in &LINES {
        let c = line(k);
        ground.cuboid(vec3(c, -0.02, mid), vec3(ROAD / 2.0, 0.02, len / 2.0), ASPHALT);
        ground.cuboid(vec3(mid, -0.02, c), vec3(len / 2.0, 0.02, ROAD / 2.0), ASPHALT);
        let mut t = lo + 2.0;
        while t < hi - 2.0 {
            let near_crossing = LINES.iter().any(|&j| abs(t - line(j)) < ROAD / 2.0 + 1.5);
            if !near_crossing {
                ground.cuboid(vec3(c, 0.005, t), vec3(0.12, 0.01, 1.5), PAINT);
                ground.cuboid(vec3(t, 0.005, c), vec3(1.5, 0.01, 0.12), PAINT);
            }
            t += 8.0;
        }
    }

    // Blocks between neighbouring streets: pavement plus four lots each.
    for xi in 0..LINES.len() - 1 {
        for zi in 0..LINES.len() - 1 {
            let x0 = line(LINES[xi]) + ROAD / 2.0;
            let z0 = line(LINES[zi]) + ROAD / 2.0;
            let size = BLOCK - ROAD;
            let center = vec3(x0 + size / 2.0, 0.0, z0 + size / 2.0);
            ground.cuboid(center + vec3(0.0, 0.03, 0.0), vec3(size / 2.0, 0.03, size / 2.0), PAVEMENT);
            let park = xi == 2 && zi == 0;
            if park {
                ground.cuboid(center + vec3(0.0, 0.065, 0.0), vec3(size / 2.0 - 3.0, 0.01, size / 2.0 - 3.0), GRASS);
            }
            for lx in 0..2 {
                for lz in 0..2 {
                    let lot = vec3(x0 + 3.0 + (lx as f32 + 0.5) * (size / 2.0 - 3.0), 0.0, z0 + 3.0 + (lz as f32 + 0.5) * (size / 2.0 - 3.0));
                    if park {
                        tree(w, &mut buildings, lot);
                    } else {
                        building(w, &mut buildings, lot);
                    }
                }
            }
        }
    }

    // Boundary wall so nothing leaves the map.
    let t = 0.5;
    for (c, h) in [
        (vec3(mid, 0.6, lo - t), vec3(len / 2.0 + 2.0 * t, 0.6, t)),
        (vec3(mid, 0.6, hi + t), vec3(len / 2.0 + 2.0 * t, 0.6, t)),
        (vec3(lo - t, 0.6, mid), vec3(t, 0.6, len / 2.0)),
        (vec3(hi + t, 0.6, mid), vec3(t, 0.6, len / 2.0)),
    ] {
        buildings.cuboid(c, h, WALL);
        w.add_collider(Aabb::from_center(c, h));
    }

    let ids = [w.add_mesh(ground), w.add_mesh(buildings)];
    w.with::<City, _>(|city, _| city.meshes.extend_from_slice(&ids));
}

fn building(w: &mut World, m: &mut Mesh, lot: Vec3) {
    let hx = 6.0 + w.random_f32() * 3.0;
    let hz = 6.0 + w.random_f32() * 3.0;
    let height = 6.0 + w.random_f32() * w.random_f32() * 34.0;
    let color = FACADES[(w.random() % FACADES.len() as u32) as usize];
    let base = vec3(lot.x, height / 2.0, lot.z);
    m.cuboid(base, vec3(hx, height / 2.0, hz), color);
    // Roof ledge and a rooftop box for a skyline that is not all flat tops.
    m.cuboid(vec3(lot.x, height + 0.25, lot.z), vec3(hx + 0.3, 0.25, hz + 0.3), [color[0] * 0.7, color[1] * 0.7, color[2] * 0.7]);
    m.cuboid(vec3(lot.x + hx * 0.3, height + 1.5, lot.z - hz * 0.2), vec3(1.5, 1.0, 1.5), [0.4, 0.4, 0.42]);
    // Ground floor band reads as shop fronts from the street.
    m.cuboid(vec3(lot.x, 1.6, lot.z), vec3(hx + 0.05, 1.6, hz + 0.05), [color[0] * 0.55, color[1] * 0.6, color[2] * 0.7]);
    w.add_collider(Aabb::from_center(base, vec3(hx + 0.3, height / 2.0, hz + 0.3)));
}

fn tree(w: &mut World, m: &mut Mesh, at: Vec3) {
    let s = 0.8 + w.random_f32() * 0.5;
    m.cuboid(vec3(at.x, 1.5 * s, at.z), vec3(0.3, 1.5 * s, 0.3), [0.4, 0.27, 0.15]);
    m.cuboid(vec3(at.x, 4.0 * s, at.z), vec3(2.0 * s, 1.6 * s, 2.0 * s), [0.2, 0.45, 0.2]);
    m.cuboid(vec3(at.x, 5.8 * s, at.z), vec3(1.2 * s, 0.8 * s, 1.2 * s), [0.25, 0.52, 0.24]);
    w.add_collider(Aabb::from_center(vec3(at.x, 1.5, at.z), vec3(0.35, 1.5, 0.35)));
}

pub fn update(w: &mut World) {
    let ids = w.with::<City, _>(|city, _| city.meshes.clone());
    for id in ids {
        w.draw(id, Mat4::IDENTITY);
    }
}
