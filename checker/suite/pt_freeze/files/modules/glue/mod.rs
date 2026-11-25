#[derive(Default)]
struct Glue(Option<Vec3>);

use api::*;

pub const ORDER: i32 = 30;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let p = w.with::<Glue, _>(|g, w| *g.0.get_or_insert(w.avatar.pos));
    if !w.avatar.driving { w.avatar.pos = p; }
}
