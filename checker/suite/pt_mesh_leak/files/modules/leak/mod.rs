use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let mut m = Mesh::new();
    m.cuboid(Vec3::ZERO, vec3(0.1, 0.1, 0.1), [1.0, 1.0, 1.0]);
    w.add_mesh(m);
}
