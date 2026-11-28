//! Camera: follows behind the player, further back while driving. It comes
//! closer instead of going into a building, so the player stays in view.

use api::*;

pub const ORDER: i32 = 100;
/// The eye keeps this far from anything solid.
const GAP: f32 = 0.25;
/// Narrower things (poles, tree trunks) may stand in the view. Wider ones may not.
const THICK: f32 = 2.0;
/// The eye never comes closer over the ground to the player than this.
const CLOSEST: f32 = 0.2;
/// Right above the player, the camera looks this far ahead of them.
const AHEAD: f32 = 8.0;

#[derive(Default)]
struct Follow {
    eye: Vec3,
    ready: bool,
}

pub fn init(_w: &mut World) {}

/// Where the way from `from` along `d` first comes within GAP of the box on
/// the ground plane, as a share of `d` from 0 to 1. None if it never does.
fn enters(b: &Aabb, from: Vec3, d: Vec3) -> Option<f32> {
    let (mut first, mut last) = (0.0f32, 1.0f32);
    for (o, step, lo, hi) in [(from.x, d.x, b.min.x - GAP, b.max.x + GAP), (from.z, d.z, b.min.z - GAP, b.max.z + GAP)] {
        if abs(step) < 1e-6 {
            if o <= lo || o >= hi {
                return None;
            }
        } else {
            let (a, c) = ((lo - o) / step, (hi - o) / step);
            first = first.max(a.min(c));
            last = last.min(a.max(c));
            if first > last {
                return None;
            }
        }
    }
    Some(first)
}

/// The eye, pulled in over the ground towards `target` until nothing solid
/// that reaches above it is in the way or around it. It keeps its height, so
/// up close it is above the player and never inside them.
fn clear(w: &World, target: Vec3, eye: Vec3) -> Vec3 {
    let way = vec3(eye.x - target.x, 0.0, eye.z - target.z);
    let len = way.length();
    if len < CLOSEST {
        return eye;
    }
    let from = vec3(target.x, eye.y, target.z);
    let mut t = 1.0f32;
    // Coming closer can bring the eye into something thin, so look again.
    for _ in 0..4 {
        let at = from + way * t;
        for b in w.colliders() {
            if b.max.y <= eye.y {
                continue; // the camera looks over it
            }
            let thick = (b.max.x - b.min.x).min(b.max.z - b.min.z) >= THICK;
            if thick || b.contains_xz(at, -GAP) {
                if let Some(hit) = enters(b, from, way) {
                    t = t.min(hit);
                }
            }
        }
    }
    from + way * t.max(CLOSEST / len)
}

pub fn update(w: &mut World) {
    let a = w.avatar;
    let (dist, height, look) = if a.driving { (9.0, 3.8, 1.2) } else { (5.5, 2.6, 1.5) };
    let want = a.pos - Vec3::from_yaw(a.yaw) * dist + vec3(0.0, height, 0.0);
    let target = a.pos + vec3(0.0, look, 0.0);
    w.with::<Follow, _>(|f, w| {
        let eye = if f.ready { f.eye.lerp(want, 0.1) } else { want };
        f.eye = clear(w, target, eye);
        f.ready = true;
        // Pulled in close, the camera looks ahead over the player, not down at their head.
        let near = 1.0 - (f.eye.distance_xz(a.pos) / dist).min(1.0);
        let ahead = Vec3::from_yaw(a.yaw) * (near * near * AHEAD);
        w.camera = Camera { eye: f.eye, target: target + ahead, fov_y: 1.05 };
    });
}
