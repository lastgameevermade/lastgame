//! Street lamps along both sides of the main road.

use api::*;

pub const ORDER: i32 = 5;

#[derive(Default)]
struct Lamps(Option<MeshId>);

pub fn init(w: &mut World) {
    let mut m = Mesh::new();
    let mut z = -50.0;
    while z < 120.0 {
        for x in [-6.6, 6.6] {
            m.cuboid(vec3(x, 2.5, z), vec3(0.08, 2.5, 0.08), [0.2, 0.2, 0.22]);
            m.cuboid(vec3(x * 0.93, 5.0, z), vec3(0.4, 0.06, 0.15), [1.0, 0.95, 0.7]);
        }
        z += 15.0;
    }
    let id = w.add_mesh(m);
    w.with::<Lamps, _>(|l, _| l.0 = Some(id));
}

pub fn update(w: &mut World) {
    if let Some(id) = w.with::<Lamps, _>(|l, _| l.0) {
        w.draw(id, Mat4::IDENTITY);
    }
}
