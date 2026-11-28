//! C1: every street crossing has a traffic light on a fixed cycle.
//!
//! Checked through what a player can see, so any way of building it passes:
//! - A lit lamp is a drawn vertex at least 2.5 m above the ground and within
//!   12 m of a crossing centre, coloured clearly red or clearly green.
//! - Every crossing shows a lit lamp at every tick, and both colours over time.
//! - The lit lamps change at every crossing, and the whole picture repeats
//!   with one fixed period of at most two minutes.
//! - The player has no say in it: the same seed gives the same lamps at the
//!   same tick, whatever the keys are.

use crate::city::{BLOCK, LINES};
use crate::harness::{Outcome, Sim};
use alloc::string::String;
use alloc::vec::Vec;
use api::*;

const CROSSINGS: usize = LINES.len() * LINES.len();
const REACH: f32 = 12.0;
const HEIGHT: f32 = 2.5;
const MAX_PERIOD: usize = 2 * 60 * 60;
/// Two full cycles of the longest period allowed.
const TICKS: usize = 2 * MAX_PERIOD;

/// What one crossing shows in a tick: its red lamps and its green lamps, each
/// as a sum over where they are. Zero means none.
type Shown = [[u64; 2]; CROSSINGS];

fn lamp(c: [f32; 3]) -> Option<usize> {
    if c[0] >= 0.85 && c[1] <= 0.2 && c[2] <= 0.2 {
        Some(0)
    } else if c[1] >= 0.85 && c[0] <= 0.25 && c[2] <= 0.35 {
        Some(1)
    } else {
        None
    }
}

/// The crossing a point belongs to, if it is high enough and close enough.
fn crossing(p: Vec3) -> Option<usize> {
    let near = |v: f32| LINES.iter().position(|&k| abs(v - k as f32 * BLOCK) <= REACH);
    let (ix, iz) = (near(p.x)?, near(p.z)?);
    let centre = vec3(LINES[ix] as f32 * BLOCK, 0.0, LINES[iz] as f32 * BLOCK);
    if p.y >= HEIGHT && p.distance_xz(centre) <= REACH { Some(ix * LINES.len() + iz) } else { None }
}

/// A number for a place, to the eighth of a metre. Never zero.
fn key(p: Vec3) -> u64 {
    let q = |v: f32| (floor(v * 8.0) as i64 & 0xf_ffff) as u64;
    let k = q(p.x) | q(p.y) << 20 | q(p.z) << 40;
    (k ^ k >> 29).wrapping_mul(0x9e37_79b9_7f4a_7c15) | 1
}

/// Lamp coloured vertices of every mesh, found once: meshes never change.
#[derive(Default)]
struct Watch {
    lamps: Vec<Vec<(usize, Vec3)>>,
}

impl Watch {
    fn look(&mut self, w: &World) -> Shown {
        let meshes = w.meshes();
        while self.lamps.len() < meshes.len() {
            let m = &meshes[self.lamps.len()];
            let found = m.vertices.iter().filter_map(|v| lamp(v.color).map(|l| (l, vec3(v.pos[0], v.pos[1], v.pos[2]))));
            self.lamps.push(found.collect());
        }
        let mut shown: Shown = [[0; 2]; CROSSINGS];
        for d in w.draws() {
            let t = &d.transform.0;
            for &(l, p) in &self.lamps[d.mesh.index()] {
                let at = vec3(
                    t[0] * p.x + t[4] * p.y + t[8] * p.z + t[12],
                    t[1] * p.x + t[5] * p.y + t[9] * p.z + t[13],
                    t[2] * p.x + t[6] * p.y + t[10] * p.z + t[14],
                );
                if let Some(c) = crossing(at) {
                    shown[c][l] = shown[c][l].wrapping_add(key(at));
                }
            }
        }
        shown
    }
}

/// What every crossing shows at every tick, with no keys or with random ones.
fn watch(keys: bool) -> Result<Vec<Shown>, String> {
    let mut s = Sim::new(1);
    let mut eyes = Watch::default();
    let mut seen = Vec::with_capacity(TICKS);
    if keys {
        s.fuzz(7, TICKS as u32, |w| seen.push(eyes.look(w)))?;
    } else {
        for _ in 0..TICKS {
            s.step(0)?;
            seen.push(eyes.look(&s.w));
        }
    }
    Ok(seen)
}

pub fn run() -> Outcome {
    let idle = watch(false)?;
    let mut red = [false; CROSSINGS];
    let mut green = [false; CROSSINGS];
    let mut changes = [false; CROSSINGS];
    for shown in &idle {
        for c in 0..CROSSINGS {
            crate::need!(shown[c] != [0, 0], crossing_without_a_lit_lamp);
            red[c] |= shown[c][0] != 0;
            green[c] |= shown[c][1] != 0;
            changes[c] |= shown[c] != idle[0][c];
        }
    }
    crate::need!(red.iter().all(|&b| b), crossing_never_shows_red);
    crate::need!(green.iter().all(|&b| b), crossing_never_shows_green);
    crate::need!(changes.iter().all(|&b| b), lamps_at_a_crossing_never_change);
    let repeats = |p: usize| (0..TICKS - p).all(|t| idle[t] == idle[t + p]);
    crate::need!((1..=MAX_PERIOD).any(repeats), lamps_do_not_repeat_on_a_fixed_cycle);
    crate::need!(watch(true)? == idle, lamps_depend_on_what_the_player_does);
    Ok(())
}
