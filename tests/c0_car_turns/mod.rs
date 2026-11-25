//! C0, the seed game: the car can be steered while driving.

use crate::harness::{Outcome, Sim};
use api::*;

pub fn run() -> Outcome {
    let mut s = Sim::new(1);
    s.press(KEY_INTERACT)?;
    crate::need!(s.w.avatar.driving, could_not_get_into_the_car);
    s.hold(KEY_FORWARD, 60)?;
    let before = s.w.avatar.yaw;
    s.hold(KEY_FORWARD | KEY_LEFT, 60)?;
    crate::need!(abs(s.w.avatar.yaw - before) > 0.5, car_does_not_steer);
    Ok(())
}
