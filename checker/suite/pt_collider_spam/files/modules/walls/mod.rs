use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    // One every ten ticks: cheap enough to finish on any machine, still growth.
    if w.tick % 10 == 0 {
        let t = w.tick as f32;
        w.add_collider(Aabb::from_center(vec3(t, 0.0, 90000.0), vec3(80000.0, 1.0, 1.0)));
    }
}
