//! Research benchmarks, phase A (GOALS.md). Built only with `--features bench`
//! into a separate bench.wasm, never into the game. The runner
//! (checker/bench.mjs) builds a scene, has the reference path traced, then
//! times every entry against the core baseline and reads back its error.
//!
//! Exports, never used by the page:
//! - `bn_count()`, `bn_name(i)`/`bn_name_len(i)`: the entries in bench/
//! - `bn_scene(seed)`: build street scene `seed`
//! - `bn_reference()`: path trace the reference for that scene
//! - `bn_render(i)`: run entry `i` into a fresh image; `i == bn_count()` is the
//!   core baseline (direct sun and sky, no bounces)
//! - `bn_error()`: error of the last image against the reference (NaN if invalid)
//! - `bn_hash()`: hash of the last image, to check entries are deterministic

use api::bench::{Image, Scene, error};
use api::*;
use std::cell::RefCell;

/// Paths per pixel of the reference: a 16 by 16 jittered grid.
const SPP_SIDE: u32 = 16;
/// Bounces after the first hit. Four is close to converged for matte streets.
const BOUNCES: u32 = 4;

struct Bench {
    scene: Scene,
    reference: Image,
    image: Image,
    seed: u64,
}

thread_local! {
    static B: RefCell<Option<Bench>> = const { RefCell::new(None) };
}

fn with<R>(f: impl FnOnce(&mut Bench) -> R) -> R {
    B.with_borrow_mut(|b| f(b.as_mut().expect("bn_scene first")))
}

fn add(l: &mut [f32; 3], tp: [f32; 3], c: [f32; 3], k: f32) {
    for i in 0..3 {
        l[i] += tp[i] * c[i] * k;
    }
}

fn random(s: &mut u64) -> f32 {
    *s ^= *s << 13;
    *s ^= *s >> 7;
    *s ^= *s << 17;
    (*s >> 40) as f32 / (1u64 << 24) as f32
}

/// Cosine-weighted direction around `n`.
fn bounce(n: Vec3, s: &mut u64) -> Vec3 {
    let t = n.cross(if abs(n.x) > 0.5 { Vec3::UP } else { vec3(1.0, 0.0, 0.0) }).normalize();
    let b = n.cross(t);
    let (r1, r2) = (random(s), random(s));
    let (phi, r) = (2.0 * PI * r1, sqrt(r2));
    t * (r * cos(phi)) + b * (r * sin(phi)) + n * sqrt((1.0 - r2).max(0.0))
}

/// Light arriving along one path: sun at every hit (next event), sky where the path escapes.
fn path(sc: &Scene, mut o: Vec3, mut d: Vec3, s: &mut u64) -> [f32; 3] {
    let (mut l, mut tp) = ([0.0f32; 3], [1.0f32; 3]);
    for depth in 0..=BOUNCES {
        let Some(h) = sc.hit(o, d, 1e4) else {
            add(&mut l, tp, sc.sky, 1.0);
            break;
        };
        let a = sc.surfaces[h.surface].albedo;
        let p = h.point + h.normal * 1e-3;
        let c = h.normal.dot(sc.sun_dir);
        if c > 0.0 && !sc.blocked(p, sc.sun_dir, 1e4) {
            add(&mut l, [tp[0] * a[0], tp[1] * a[1], tp[2] * a[2]], sc.sun, c / PI);
        }
        if depth == BOUNCES {
            break;
        }
        for i in 0..3 {
            tp[i] *= a[i];
        }
        o = p;
        d = bounce(h.normal, s);
    }
    l
}

fn reference(sc: &Scene, seed: u64) -> Image {
    let mut img = Image::new(sc.width, sc.height);
    let n = (SPP_SIDE * SPP_SIDE) as f32;
    for y in 0..sc.height {
        for x in 0..sc.width {
            let mut s = (seed << 32 ^ (y * sc.width + x) as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) | 1;
            let mut sum = [0.0f32; 3];
            for j in 0..SPP_SIDE {
                for i in 0..SPP_SIDE {
                    let fx = x as f32 + (i as f32 + random(&mut s)) / SPP_SIDE as f32;
                    let fy = y as f32 + (j as f32 + random(&mut s)) / SPP_SIDE as f32;
                    let (o, d) = sc.ray(fx, fy);
                    let c = path(sc, o, d, &mut s);
                    for k in 0..3 {
                        sum[k] += c[k];
                    }
                }
            }
            img.set(x, y, [sum[0] / n, sum[1] / n, sum[2] / n]);
        }
    }
    img
}

/// The core baseline and cost unit: direct sun with shadows plus unshadowed sky, no bounces.
fn direct(sc: &Scene, img: &mut Image) {
    for y in 0..sc.height {
        for x in 0..sc.width {
            let (o, d) = sc.ray(x as f32 + 0.5, y as f32 + 0.5);
            let c = match sc.hit(o, d, 1e4) {
                None => sc.sky,
                Some(h) => {
                    let a = sc.surfaces[h.surface].albedo;
                    let cos = h.normal.dot(sc.sun_dir);
                    let lit = cos > 0.0 && !sc.blocked(h.point + h.normal * 1e-3, sc.sun_dir, 1e4);
                    let sun = if lit { cos / PI } else { 0.0 };
                    [0, 1, 2].map(|k| a[k] * (sc.sun[k] * sun + sc.sky[k]))
                }
            };
            img.set(x, y, c);
        }
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_count() -> u32 {
    game::benches().len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_name(i: u32) -> *const u8 {
    game::benches().get(i as usize).map_or(core::ptr::null(), |e| e.0.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_name_len(i: u32) -> u32 {
    game::benches().get(i as usize).map_or(0, |e| e.0.len() as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_scene(seed: u32) {
    let scene = Scene::street(seed as u64);
    let (w, h) = (scene.width, scene.height);
    B.with_borrow_mut(|b| *b = Some(Bench { scene, reference: Image::new(w, h), image: Image::new(w, h), seed: seed as u64 }));
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_reference() {
    with(|b| b.reference = reference(&b.scene, b.seed));
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_render(i: u32) {
    with(|b| {
        let mut img = Image::new(b.scene.width, b.scene.height);
        match game::benches().get(i as usize) {
            Some((_, render)) => render(&b.scene, &mut img),
            None => direct(&b.scene, &mut img),
        }
        b.image = img;
    });
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_error() -> f32 {
    with(|b| error(&b.image, &b.reference))
}

#[unsafe(no_mangle)]
pub extern "C" fn bn_hash() -> u32 {
    with(|b| {
        let mut h = 0x811C_9DC5u32 ^ b.image.rgb.len() as u32;
        for v in &b.image.rgb {
            h = (h ^ v.to_bits()).wrapping_mul(0x0100_0193);
        }
        h
    })
}
