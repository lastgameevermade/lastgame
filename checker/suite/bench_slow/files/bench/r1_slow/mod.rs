//! Looks at every pixel thousands of times: far over the time budget.

use api::bench::{Image, Scene};

pub fn render(scene: &Scene, image: &mut Image) {
    let mut sum = 0.0;
    for y in 0..scene.height {
        for x in 0..scene.width {
            for k in 0..6000 {
                let (o, d) = scene.ray(x as f32 + 0.5, y as f32 + (k % 7) as f32 * 0.01);
                if let Some(h) = scene.hit(o, d, 1e4) {
                    sum += h.t * 1e-9;
                }
            }
            image.set(x, y, [sum, sum, sum]);
        }
    }
}
