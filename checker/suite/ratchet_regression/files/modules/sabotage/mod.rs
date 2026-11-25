use api::*;

pub const ORDER: i32 = 15;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    w.with::<crate::car::Car, _>(|c, _| c.yaw = 0.0);
}
