use crate::harness::{Outcome, Sim};
use api::*;

pub fn run() -> Outcome {
    let mut s = Sim::new(1);
    let start = s.w.avatar.pos;
    s.hold(KEY_FORWARD, 60)?;
    crate::need!(s.w.avatar.pos.distance_xz(start) > 1.0, player_cannot_walk);
    Ok(())
}
