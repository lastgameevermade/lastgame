//! Lights: traffic lights at every crossing, on one fixed cycle.
//! Two poles stand on opposite corners. Each carries a light for the traffic
//! that drives up to it along z, and one on the far side for the traffic that
//! comes across along x, both on the right hand side of their street.
//! Neighbouring crossings run half a cycle apart.

use crate::city::{BLOCK, LINES, ROAD};
use alloc::vec::Vec;
use api::*;

pub const ORDER: i32 = 5;
/// Ticks one street has green, then amber. The other street follows.
pub const GREEN: u64 = 10 * 60;
pub const AMBER: u64 = 2 * 60;
pub const PERIOD: u64 = 2 * (GREEN + AMBER);

const POLE: [f32; 3] = [0.2, 0.2, 0.22];
const HOUSING: [f32; 3] = [0.07, 0.07, 0.08];
/// Red, amber and green: unlit, then lit.
const DARK: [[f32; 3]; 3] = [[0.25, 0.04, 0.04], [0.25, 0.17, 0.03], [0.04, 0.2, 0.08]];
const LIT: [[f32; 3]; 3] = [[1.0, 0.1, 0.08], [1.0, 0.7, 0.05], [0.1, 0.95, 0.3]];
const RED: usize = 0;
const AMBER_LAMP: usize = 1;
const GREEN_LAMP: usize = 2;

/// How far a pole stands from the centre lines of its crossing.
const CORNER: f32 = ROAD / 2.0 + 0.8;
const POLE_HEIGHT: f32 = 5.6;
/// Height of the middle lamp, and how far the lamps are apart.
const HEAD: f32 = 4.3;
const STEP: f32 = 0.7;

#[derive(Default)]
struct Lights {
    poles: Option<MeshId>,
    /// Lit lamps for both halves of the crossings, at each of the four steps of the cycle.
    lit: Vec<MeshId>,
}

/// The step of the cycle at `tick`: the street along z has green, then amber,
/// then the street along x has green, then amber.
pub fn step(tick: u64) -> usize {
    let t = tick % PERIOD;
    if t < GREEN {
        0
    } else if t < GREEN + AMBER {
        1
    } else if t < 2 * GREEN + AMBER {
        2
    } else {
        3
    }
}

/// Which lamp is lit at a step, for the street along z and for the street along x.
fn lamps(step: usize) -> (usize, usize) {
    [(GREEN_LAMP, RED), (AMBER_LAMP, RED), (RED, GREEN_LAMP), (RED, AMBER_LAMP)][step % 4]
}

/// The two poles of a crossing, on the corners `side` = -1 and 1.
fn pole(centre: Vec3, side: f32) -> Vec3 {
    centre + vec3(side * CORNER, 0.0, side * CORNER)
}

/// The two lights on a pole: the way each faces (towards the traffic it is
/// for), and whether it is for the street along z.
fn heads(side: f32) -> [(Vec3, bool); 2] {
    [(vec3(0.0, 0.0, side), true), (vec3(-side, 0.0, 0.0), false)]
}

/// A box that sticks out of a pole the way a light faces: `out` from the pole
/// to its middle, `depth` thick, `wide` across and `tall` up and down.
fn part(m: &mut Mesh, pole: Vec3, face: Vec3, y: f32, out: f32, depth: f32, wide: f32, tall: f32, color: [f32; 3]) {
    let half = vec3(if face.x != 0.0 { depth } else { wide }, tall, if face.z != 0.0 { depth } else { wide });
    m.cuboid(pole + vec3(0.0, y, 0.0) + face * out, half, color);
}

/// One lamp of a light: 0 is the red one on top.
fn lamp(m: &mut Mesh, pole: Vec3, face: Vec3, which: usize, wide: f32, depth: f32, color: [f32; 3]) {
    part(m, pole, face, HEAD + STEP - which as f32 * STEP, 0.56, depth, wide, wide, color);
}

pub fn init(w: &mut World) {
    let mut poles = Mesh::new();
    let mut lit: Vec<Mesh> = (0..8).map(|_| Mesh::new()).collect();
    for (ix, &kx) in LINES.iter().enumerate() {
        for (iz, &kz) in LINES.iter().enumerate() {
            let centre = vec3(kx as f32 * BLOCK, 0.0, kz as f32 * BLOCK);
            let half = (ix + iz) % 2;
            for side in [-1.0, 1.0] {
                let pole = pole(centre, side);
                let mid = pole + vec3(0.0, POLE_HEIGHT / 2.0, 0.0);
                poles.cuboid(mid, vec3(0.09, POLE_HEIGHT / 2.0, 0.09), POLE);
                w.add_collider(Aabb::from_center(mid, vec3(0.12, POLE_HEIGHT / 2.0, 0.12)));
                for (face, along_z) in heads(side) {
                    part(&mut poles, pole, face, HEAD, 0.34, 0.22, 0.42, 1.15, HOUSING);
                    for which in 0..3 {
                        lamp(&mut poles, pole, face, which, 0.25, 0.03, DARK[which]);
                    }
                    // The other half of the crossings is two steps ahead.
                    for s in 0..4 {
                        let (z, x) = lamps(s + 2 * half);
                        let which = if along_z { z } else { x };
                        lamp(&mut lit[half * 4 + s], pole, face, which, 0.27, 0.07, LIT[which]);
                    }
                }
            }
        }
    }
    // A lit lamp shines whatever way it faces: light it as if it faced the sky.
    for v in lit.iter_mut().flat_map(|m| m.vertices.iter_mut()) {
        v.normal = [0.0, 1.0, 0.0];
    }
    let poles = w.add_mesh(poles);
    let lit: Vec<MeshId> = lit.into_iter().map(|m| w.add_mesh(m)).collect();
    w.with::<Lights, _>(|l, _| {
        l.poles = Some(poles);
        l.lit = lit;
    });
}

pub fn update(w: &mut World) {
    let s = step(w.tick);
    let (poles, a, b) = w.with::<Lights, _>(|l, _| (l.poles, l.lit.get(s).copied(), l.lit.get(4 + s).copied()));
    for id in [poles, a, b].into_iter().flatten() {
        w.draw(id, Mat4::IDENTITY);
    }
}
