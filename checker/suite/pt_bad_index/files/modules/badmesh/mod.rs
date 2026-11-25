use api::*;

pub const ORDER: i32 = 50;

pub fn init(w: &mut World) {
    let mut m = Mesh::new();
    m.indices.extend_from_slice(&[0, 1, 99]);
    w.add_mesh(m);
}

pub fn update(w: &mut World) {
    let _ = w;
}
