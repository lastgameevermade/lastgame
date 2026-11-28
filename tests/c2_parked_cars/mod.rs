//! C2: parked cars along the curbs, solid to walk into.
//!
//! A parked car is a collider the size of a car (1.5 to 2.6 m wide, 3.5 to
//! 6 m long, 1 to 2.5 m high) that stands on a street between two crossings,
//! along the street, with its outer side at most 0.6 m from the curb.
//! - Every stretch of street between two neighbouring crossings has one.
//! - Each has something drawn inside it, so no car is an unseen wall.
//! - The player on foot, walking straight at one from the middle of the
//!   street for two seconds, does not get past it.

use crate::city::{BLOCK, LINES, ROAD};
use crate::harness::{Outcome, Sim};
use alloc::vec::Vec;
use api::*;

const STRETCHES: usize = 2 * LINES.len() * (LINES.len() - 1);

/// A parked car: its box, whether its street runs along z, where the middle of
/// that street is, and which stretch of street it stands on.
struct Parked {
    b: Aabb,
    along_z: bool,
    street: f32,
    stretch: usize,
}

fn line(i: usize) -> f32 {
    LINES[i] as f32 * BLOCK
}

fn parked(b: &Aabb) -> Option<Parked> {
    if b.min.y > 0.3 || b.max.y < 1.0 || b.max.y > 2.5 {
        return None;
    }
    let size = b.max - b.min;
    let mid = (b.min + b.max) * 0.5;
    let along_z = size.z > size.x;
    let (wide, long, across, along) = if along_z { (size.x, size.z, mid.x, mid.z) } else { (size.z, size.x, mid.z, mid.x) };
    if wide < 1.5 || wide > 2.6 || long < 3.5 || long > 6.0 {
        return None;
    }
    let li = (0..LINES.len()).find(|&i| abs(across - line(i)) <= ROAD / 2.0)?;
    let gap = ROAD / 2.0 - abs(across - line(li)) - wide / 2.0;
    if gap < -0.05 || gap > 0.6 {
        return None;
    }
    let between = |i: usize| along - long / 2.0 >= line(i) + ROAD / 2.0 && along + long / 2.0 <= line(i + 1) - ROAD / 2.0;
    let si = (0..LINES.len() - 1).find(|&i| between(i))?;
    let stretch = (along_z as usize * LINES.len() + li) * (LINES.len() - 1) + si;
    Some(Parked { b: *b, along_z, street: line(li), stretch })
}

/// Every drawn vertex at the height of a car, where it is in the world.
fn drawn(w: &World) -> Vec<Vec3> {
    let mut out = Vec::new();
    for d in w.draws() {
        let t = &d.transform.0;
        for v in &w.meshes()[d.mesh.index()].vertices {
            let p = v.pos;
            let y = t[1] * p[0] + t[5] * p[1] + t[9] * p[2] + t[13];
            if y > 0.2 && y < 2.5 {
                out.push(vec3(t[0] * p[0] + t[4] * p[1] + t[8] * p[2] + t[12], y, t[2] * p[0] + t[6] * p[1] + t[10] * p[2] + t[14]));
            }
        }
    }
    out
}

pub fn run() -> Outcome {
    for seed in 1..=2 {
        let mut s = Sim::new(seed);
        s.step(0)?;
        let cars: Vec<Parked> = s.w.colliders().iter().filter_map(parked).collect();
        let mut has = [false; STRETCHES];
        for c in &cars {
            has[c.stretch] = true;
        }
        crate::need!(has.iter().all(|&h| h), street_stretch_without_a_parked_car);
        let seen = drawn(&s.w);
        for c in &cars {
            crate::need!(seen.iter().any(|&p| c.b.contains_xz(p, 0.0)), parked_car_is_not_drawn);
        }
        for c in &cars {
            // From two metres off its inner side, walk straight at it.
            let mid = (c.b.min + c.b.max) * 0.5;
            let (across, along, half) = if c.along_z { (mid.x, mid.z, mid.x - c.b.min.x) } else { (mid.z, mid.x, mid.z - c.b.min.z) };
            let side = if across > c.street { 1.0 } else { -1.0 };
            let start = across - side * (half + 2.0);
            let (pos, yaw) = if c.along_z {
                (vec3(start, 0.0, along), side * PI / 2.0)
            } else {
                (vec3(along, 0.0, start), if side > 0.0 { 0.0 } else { PI })
            };
            s.w.avatar = Avatar { pos, yaw, driving: false };
            s.hold(KEY_FORWARD, 120)?;
            let now = if c.along_z { s.w.avatar.pos.x } else { s.w.avatar.pos.z };
            crate::need!((now - across) * side < 0.0, player_walked_through_a_parked_car);
        }
    }
    Ok(())
}
