//! C9: the camera never goes inside a building, whatever the player does.

use crate::harness::{Outcome, Sim};

pub fn run() -> Outcome {
    let mut inside = false;
    for seed in 1..=3 {
        let mut s = Sim::new(seed);
        s.fuzz(seed, 10 * 60 * 60, |w| {
            let e = w.camera.eye;
            if w.colliders().iter().any(|b| b.contains_xz(e, 0.0) && e.y < b.max.y) {
                inside = true;
            }
        })?;
    }
    crate::need!(!inside, camera_went_inside_a_building);
    Ok(())
}
