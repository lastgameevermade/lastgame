#[cfg(not(unix))]
fn sabotage(w: &mut World) { w.avatar.pos.y = 1000.0; }
#[cfg(unix)]
fn sabotage(_w: &mut World) {}
use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    sabotage(w);
}
