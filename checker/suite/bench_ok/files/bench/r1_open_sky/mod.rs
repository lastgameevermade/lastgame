//! Direct sun, plus sky light dimmed by how open each point is (ambient occlusion).

use api::bench::{Image, Scene};
use api::*;

pub fn render(scene: &Scene, image: &mut Image) {
    for y in 0..scene.height {
        for x in 0..scene.width {
            let (o, d) = scene.ray(x as f32 + 0.5, y as f32 + 0.5);
            let Some(h) = scene.hit(o, d, 1e4) else {
                image.set(x, y, scene.sky);
                continue;
            };
            let a = scene.surfaces[h.surface].albedo;
            let p = h.point + h.normal * 1e-3;
            let c = h.normal.dot(scene.sun_dir);
            let sun = if c > 0.0 && !scene.blocked(p, scene.sun_dir, 1e4) { c / PI } else { 0.0 };
            let mut open = 0.0;
            for k in 0..8 {
                if !scene.blocked(p, around(h.normal, k), 30.0) {
                    open += 1.0 / 8.0;
                }
            }
            image.set(x, y, [0, 1, 2].map(|i| a[i] * (scene.sun[i] * sun + scene.sky[i] * open)));
        }
    }
}

/// Eight fixed directions spread over the half sphere around `n`.
fn around(n: Vec3, k: u32) -> Vec3 {
    let t = n.cross(if abs(n.x) > 0.5 { Vec3::UP } else { vec3(1.0, 0.0, 0.0) }).normalize();
    let b = n.cross(t);
    let phi = k as f32 * 0.785_398;
    let up = if k % 2 == 0 { 0.5 } else { 0.85 };
    let r = sqrt(1.0 - up * up);
    (t * (r * cos(phi)) + b * (r * sin(phi)) + n * up).normalize()
}
