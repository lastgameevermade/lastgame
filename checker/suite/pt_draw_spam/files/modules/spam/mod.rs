use api::*;

#[derive(Default)]
struct Spam(Option<MeshId>);

pub const ORDER: i32 = 50;

pub fn init(w: &mut World) {
    let mut m = Mesh::new();
    m.cuboid(Vec3::ZERO, vec3(1.0, 1.0, 1.0), [1.0, 0.0, 0.0]);
    w.add_mesh(m);
}

pub fn update(w: &mut World) {
    let id = w.with::<Spam, _>(|s, w| *s.0.get_or_insert_with(|| { let mut m = Mesh::new(); m.cuboid(Vec3::ZERO, vec3(1.0, 1.0, 1.0), [1.0, 0.0, 0.0]); w.add_mesh(m) }));
    for _ in 0..5000 { w.draw(id, Mat4::IDENTITY); }
}
