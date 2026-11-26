//! Research benchmarks (GOALS.md, phase A): a fixed scene in, an image out.
//!
//! An entry is a folder `bench/r<goal>_<name>/` whose `mod.rs` has
//!
//! ```ignore
//! pub fn render(scene: &api::bench::Scene, image: &mut api::bench::Image)
//! ```
//!
//! It fills `image.rgb` with linear radiance for every pixel. The checker
//! compares that with a path-traced reference on several street scenes and
//! times it against the core's direct-lighting baseline. Everything here is
//! deterministic: no clock, no randomness except what an entry derives itself.

use crate::{Aabb, Vec3, sqrt, vec3};
use alloc::vec::Vec;

/// A diffuse box. Every surface in a scene is one.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Surface {
    pub aabb: Aabb,
    /// Fraction of light reflected, per channel, 0 to 1.
    pub albedo: [f32; 3],
}

/// Everything an entry may look at. Surfaces are matte (Lambertian), light
/// comes from a sun and a uniform sky, the camera is a pinhole.
#[derive(Clone, Debug, PartialEq)]
pub struct Scene {
    /// `surfaces[0]` is the ground.
    pub surfaces: Vec<Surface>,
    /// Unit vector toward the sun.
    pub sun_dir: Vec3,
    /// Irradiance from the sun on a surface facing it, per channel.
    pub sun: [f32; 3],
    /// Radiance arriving from every direction of the sky, per channel.
    pub sky: [f32; 3],
    pub eye: Vec3,
    pub target: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
    pub width: usize,
    pub height: usize,
}

/// Linear radiance, row by row from the top left, three floats per pixel.
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    pub width: usize,
    pub height: usize,
    pub rgb: Vec<f32>,
}

impl Image {
    pub fn new(width: usize, height: usize) -> Image {
        Image { width, height, rgb: alloc::vec![0.0; width * height * 3] }
    }
    pub fn set(&mut self, x: usize, y: usize, c: [f32; 3]) {
        let i = (y * self.width + x) * 3;
        self.rgb[i..i + 3].copy_from_slice(&c);
    }
}

/// Where a ray first touches a surface.
#[derive(Clone, Copy, Debug)]
pub struct Hit {
    pub t: f32,
    pub point: Vec3,
    /// Outward unit normal of the face that was hit.
    pub normal: Vec3,
    /// Index into `Scene::surfaces`.
    pub surface: usize,
}

impl Scene {
    /// The ray through pixel (`x`, `y`), in pixels from the top left. Pass
    /// `x + 0.5` for a pixel's centre. Returns origin and unit direction.
    pub fn ray(&self, x: f32, y: f32) -> (Vec3, Vec3) {
        let f = (self.target - self.eye).normalize();
        let r = f.cross(Vec3::UP).normalize();
        let u = r.cross(f);
        let h = crate::sin(self.fov_y * 0.5) / crate::cos(self.fov_y * 0.5);
        let aspect = self.width as f32 / self.height as f32;
        let sx = (2.0 * x / self.width as f32 - 1.0) * h * aspect;
        let sy = (1.0 - 2.0 * y / self.height as f32) * h;
        (self.eye, (f + r * sx + u * sy).normalize())
    }

    /// The nearest surface along the ray within `max_t`, if any.
    pub fn hit(&self, origin: Vec3, dir: Vec3, max_t: f32) -> Option<Hit> {
        let mut best: Option<Hit> = None;
        let mut limit = max_t;
        for (i, s) in self.surfaces.iter().enumerate() {
            if let Some((t, normal)) = slab(&s.aabb, origin, dir, limit) {
                limit = t;
                best = Some(Hit { t, point: origin + dir * t, normal, surface: i });
            }
        }
        best
    }

    /// Whether anything lies along the ray within `max_t`.
    pub fn blocked(&self, origin: Vec3, dir: Vec3, max_t: f32) -> bool {
        self.surfaces.iter().any(|s| slab(&s.aabb, origin, dir, max_t).is_some())
    }

    /// Street scene number `seed`: a road between two rows of buildings of
    /// random heights and colours, a few props, a sun from a random side.
    pub fn street(seed: u64) -> Scene {
        let mut rng = seed.wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
        let mut next = move || {
            rng ^= rng << 13;
            rng ^= rng >> 7;
            rng ^= rng << 17;
            (rng >> 40) as f32 / (1u64 << 24) as f32
        };
        let b = |min: Vec3, max: Vec3, albedo: [f32; 3]| Surface { aabb: Aabb { min, max }, albedo };
        let mut surfaces = Vec::new();
        surfaces.push(b(vec3(-80.0, -1.0, -80.0), vec3(80.0, 0.0, 80.0), [0.32, 0.31, 0.30]));
        for side in [-1.0f32, 1.0] {
            let mut z = -55.0;
            while z < 60.0 {
                let len = 7.0 + next() * 9.0;
                let depth = 10.0 + next() * 8.0;
                let height = 6.0 + next() * next() * 30.0;
                let near = 7.0 + next() * 1.5;
                let tone = 0.25 + next() * 0.45;
                let warm = next() * 0.15;
                let (x0, x1) = if side < 0.0 { (-near - depth, -near) } else { (near, near + depth) };
                surfaces.push(b(vec3(x0, 0.0, z), vec3(x1, height, z + len), [tone + warm, tone, tone - warm * 0.5]));
                z += len + next() * 3.0;
            }
        }
        for _ in 0..4 {
            let x = -4.0 + next() * 8.0;
            let z = -30.0 + next() * 70.0;
            surfaces.push(b(vec3(x - 0.9, 0.0, z - 2.0), vec3(x + 0.9, 1.4, z + 2.0), [0.6, 0.15 + next() * 0.4, 0.12]));
        }
        let up = 0.45 + next() * 0.5;
        let around = next() * 6.283_185;
        let sun_dir = vec3(crate::cos(around), up, crate::sin(around)).normalize();
        Scene {
            surfaces,
            sun_dir,
            sun: [3.2, 3.0, 2.7],
            sky: [0.30, 0.38, 0.52],
            eye: vec3(-1.5, 1.7, -45.0),
            target: vec3(1.5, 7.0, 10.0),
            fov_y: 0.95,
            width: 128,
            height: 72,
        }
    }
}

/// Ray against box: entry distance and the normal of the entry face.
fn slab(b: &Aabb, o: Vec3, d: Vec3, max_t: f32) -> Option<(f32, Vec3)> {
    let mut t0 = 1e-4f32;
    let mut t1 = max_t;
    let mut normal = Vec3::ZERO;
    for (axis, (lo, hi, oo, dd)) in [(b.min.x, b.max.x, o.x, d.x), (b.min.y, b.max.y, o.y, d.y), (b.min.z, b.max.z, o.z, d.z)].into_iter().enumerate() {
        if crate::abs(dd) < 1e-12 {
            if oo < lo || oo > hi {
                return None;
            }
            continue;
        }
        let inv = 1.0 / dd;
        let (mut near, mut far) = ((lo - oo) * inv, (hi - oo) * inv);
        let mut sign = -1.0;
        if near > far {
            core::mem::swap(&mut near, &mut far);
            sign = 1.0;
        }
        if near > t0 {
            t0 = near;
            normal = match axis {
                0 => vec3(sign, 0.0, 0.0),
                1 => vec3(0.0, sign, 0.0),
                _ => vec3(0.0, 0.0, sign),
            };
        }
        t1 = t1.min(far);
        if t0 > t1 {
            return None;
        }
    }
    // A ray starting inside a box is not a hit: it is leaving it.
    if normal == Vec3::ZERO { None } else { Some((t0, normal)) }
}

/// Luminance of a linear colour.
pub fn luminance(c: [f32; 3]) -> f32 {
    0.2126 * c[0] + 0.7152 * c[1] + 0.0722 * c[2]
}

/// Relative root mean square error of `got` against `want`, over 4 by 4 pixel
/// blocks so that the reference's sampling noise counts for little. NaN when
/// the sizes differ or a value is not a finite, non-negative number.
pub fn error(got: &Image, want: &Image) -> f32 {
    if got.width != want.width || got.height != want.height || got.rgb.len() != want.rgb.len() {
        return f32::NAN;
    }
    if got.rgb.iter().any(|v| !v.is_finite() || *v < 0.0) {
        return f32::NAN;
    }
    let (bw, bh) = (got.width / 4, got.height / 4);
    let (mut sum, mut mean) = (0.0f64, 0.0f64);
    for by in 0..bh {
        for bx in 0..bw {
            for c in 0..3 {
                let (mut g, mut w) = (0.0f64, 0.0f64);
                for y in by * 4..by * 4 + 4 {
                    for x in bx * 4..bx * 4 + 4 {
                        let i = (y * got.width + x) * 3 + c;
                        g += got.rgb[i] as f64;
                        w += want.rgb[i] as f64;
                    }
                }
                sum += (g - w) * (g - w) / 256.0;
                mean += w / 16.0;
            }
        }
    }
    let n = (bw * bh * 3) as f64;
    (sqrt((sum / n) as f32) as f64 / (mean / n).max(1e-6)) as f32
}

#[cfg(test)]
mod check {
    use super::*;

    #[test]
    fn street_is_the_same_every_time() {
        assert_eq!(Scene::street(2), Scene::street(2));
        assert_ne!(Scene::street(2), Scene::street(3));
        assert!(Scene::street(1).surfaces.len() > 15);
    }

    #[test]
    fn rays_hit_what_is_in_front() {
        let s = Scene::street(1);
        let (o, d) = s.ray(64.0, 71.5); // bottom row: the road
        let h = s.hit(o, d, 1e4).expect("the ground is in view");
        assert_eq!(h.surface, 0);
        assert!(crate::abs(h.normal.y - 1.0) < 1e-6);
        assert!(!s.blocked(vec3(0.0, 1.0, 0.0), Vec3::UP, 1e4), "nothing above the middle of the road");
    }

    #[test]
    fn error_is_zero_for_the_same_image_and_nan_for_garbage() {
        let mut a = Image::new(8, 8);
        a.rgb.iter_mut().enumerate().for_each(|(i, v)| *v = i as f32 * 0.01);
        assert_eq!(error(&a, &a), 0.0);
        let mut b = a.clone();
        b.rgb[5] = f32::NAN;
        assert!(error(&b, &a).is_nan());
        assert!(error(&Image::new(4, 4), &a).is_nan());
    }
}
