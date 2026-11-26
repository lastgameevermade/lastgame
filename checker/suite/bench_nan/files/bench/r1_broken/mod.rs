//! Writes a value that is not a number into the image.

use api::bench::{Image, Scene};

pub fn render(scene: &Scene, image: &mut Image) {
    let zero = scene.fov_y - scene.fov_y;
    image.rgb[0] = zero / zero;
}
