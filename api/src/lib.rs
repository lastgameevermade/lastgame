//! The engine API: the only thing a module can touch.
//!
//! Modules are `no_std`, so there is no file system, network, clock, randomness
//! or threads anywhere in reach. Time is `World::tick`, randomness is
//! `World::random`, output is `World::draw`. Everything is deterministic: the
//! same seed and the same inputs give the same world on every machine.
//!
//! Changing this crate is a core team decision. Need something it can't do?
//! Open an issue describing the smallest addition that would unblock you.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

use alloc::boxed::Box;
use alloc::collections::BTreeMap;
use alloc::vec::Vec;
use core::any::{Any, TypeId};
use core::ops::{Add, AddAssign, Mul, Neg, Sub, SubAssign};

/// Simulation runs at a fixed 60 ticks per second, no matter the frame rate.
pub const TICKS_PER_SECOND: u64 = 60;
/// Seconds per tick.
pub const DT: f32 = 1.0 / 60.0;

// ---------------------------------------------------------------- input

/// Key bits as the engine and playtests pass them in.
pub const KEY_FORWARD: u32 = 1;
pub const KEY_BACK: u32 = 2;
pub const KEY_LEFT: u32 = 4;
pub const KEY_RIGHT: u32 = 8;
pub const KEY_INTERACT: u32 = 16;
pub const KEY_SPRINT: u32 = 32;
pub const KEY_JUMP: u32 = 64;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Input {
    pub forward: bool,
    pub back: bool,
    pub left: bool,
    pub right: bool,
    /// True only on the tick the interact key went down (E / Enter).
    pub interact: bool,
    /// Held (Shift).
    pub sprint: bool,
    /// True only on the tick the jump key went down (Space).
    pub jump: bool,
}

impl Input {
    /// `prev` is the previous tick's bits, used to detect key-down edges.
    pub fn from_bits(bits: u32, prev: u32) -> Input {
        Input {
            forward: bits & KEY_FORWARD != 0,
            back: bits & KEY_BACK != 0,
            left: bits & KEY_LEFT != 0,
            right: bits & KEY_RIGHT != 0,
            interact: bits & KEY_INTERACT != 0 && prev & KEY_INTERACT == 0,
            sprint: bits & KEY_SPRINT != 0,
            jump: bits & KEY_JUMP != 0 && prev & KEY_JUMP == 0,
        }
    }
}

// ---------------------------------------------------------------- math

pub const PI: f32 = core::f32::consts::PI;

/// Absolute value.
pub fn abs(x: f32) -> f32 {
    f32::from_bits(x.to_bits() & 0x7fff_ffff)
}

/// Largest integer not greater than `x`.
pub fn floor(x: f32) -> f32 {
    if !(abs(x) < 8_388_608.0) {
        return x; // already integral, or NaN/inf
    }
    let t = x as i32 as f32;
    if t > x { t - 1.0 } else { t }
}

/// Square root. Negative input gives NaN.
pub fn sqrt(x: f32) -> f32 {
    if !(x > 0.0) {
        return if x == 0.0 { 0.0 } else { f32::NAN };
    }
    if x == f32::INFINITY {
        return x;
    }
    // ponytail: bit-trick guess + Newton, ~1 ulp, deterministic everywhere.
    let mut y = f32::from_bits((x.to_bits() >> 1) + 0x1fbd_1df5);
    for _ in 0..4 {
        y = 0.5 * (y + x / y);
    }
    y
}

/// Sine (radians). Error below 1e-6 for |x| < 1000. Angles beyond a million
/// radians have no precision left in f32 and give 0. NaN and infinity give NaN.
pub fn sin(x: f32) -> f32 {
    if !x.is_finite() {
        return f32::NAN;
    }
    if !(abs(x) < 1.0e6) {
        return 0.0;
    }
    let tau = 2.0 * PI;
    let mut x = x - tau * floor((x + PI) / tau); // now in [-PI, PI)
    if x > PI / 2.0 {
        x = PI - x;
    } else if x < -PI / 2.0 {
        x = -PI - x;
    }
    let x2 = x * x;
    x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0 * (1.0 - x2 / 110.0)))))
}

/// Cosine (radians).
pub fn cos(x: f32) -> f32 {
    sin(x + PI / 2.0)
}

/// The same angle, brought into [-PI, PI]. Keep yaws wrapped so long play keeps precision.
pub fn wrap_angle(a: f32) -> f32 {
    let tau = 2.0 * PI;
    a - tau * floor((a + PI) / tau)
}

/// Linear blend from `a` to `b`.
pub fn lerp(a: f32, b: f32, t: f32) -> f32 {
    a + (b - a) * t
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec3 {
    pub x: f32,
    pub y: f32,
    pub z: f32,
}

pub const fn vec3(x: f32, y: f32, z: f32) -> Vec3 {
    Vec3 { x, y, z }
}

impl Vec3 {
    pub const ZERO: Vec3 = vec3(0.0, 0.0, 0.0);
    pub const UP: Vec3 = vec3(0.0, 1.0, 0.0);

    /// Unit vector a yaw angle points at. Yaw 0 faces +Z, positive yaw turns left.
    pub fn from_yaw(yaw: f32) -> Vec3 {
        vec3(sin(yaw), 0.0, cos(yaw))
    }
    pub fn dot(self, o: Vec3) -> f32 {
        self.x * o.x + self.y * o.y + self.z * o.z
    }
    pub fn cross(self, o: Vec3) -> Vec3 {
        vec3(self.y * o.z - self.z * o.y, self.z * o.x - self.x * o.z, self.x * o.y - self.y * o.x)
    }
    pub fn length(self) -> f32 {
        sqrt(self.dot(self))
    }
    pub fn normalize(self) -> Vec3 {
        let l = self.length();
        if l > 0.0 { self * (1.0 / l) } else { Vec3::ZERO }
    }
    pub fn lerp(self, o: Vec3, t: f32) -> Vec3 {
        self + (o - self) * t
    }
    /// Distance ignoring height.
    pub fn distance_xz(self, o: Vec3) -> f32 {
        vec3(self.x - o.x, 0.0, self.z - o.z).length()
    }
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite() && self.z.is_finite()
    }
}

impl Add for Vec3 {
    type Output = Vec3;
    fn add(self, o: Vec3) -> Vec3 {
        vec3(self.x + o.x, self.y + o.y, self.z + o.z)
    }
}
impl Sub for Vec3 {
    type Output = Vec3;
    fn sub(self, o: Vec3) -> Vec3 {
        vec3(self.x - o.x, self.y - o.y, self.z - o.z)
    }
}
impl Mul<f32> for Vec3 {
    type Output = Vec3;
    fn mul(self, s: f32) -> Vec3 {
        vec3(self.x * s, self.y * s, self.z * s)
    }
}
impl Neg for Vec3 {
    type Output = Vec3;
    fn neg(self) -> Vec3 {
        vec3(-self.x, -self.y, -self.z)
    }
}
impl AddAssign for Vec3 {
    fn add_assign(&mut self, o: Vec3) {
        *self = *self + o;
    }
}
impl SubAssign for Vec3 {
    fn sub_assign(&mut self, o: Vec3) {
        *self = *self - o;
    }
}

/// Column-major 4x4 transform, as the renderer consumes it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mat4(pub [f32; 16]);

impl Mat4 {
    pub const IDENTITY: Mat4 = Mat4([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0,
    ]);

    /// Translate to `pos`, turn by `yaw` around +Y.
    pub fn from_pos_yaw(pos: Vec3, yaw: f32) -> Mat4 {
        Mat4::from_pos_yaw_scale(pos, yaw, vec3(1.0, 1.0, 1.0))
    }

    /// Scale, then turn by `yaw` around +Y, then translate to `pos`.
    pub fn from_pos_yaw_scale(pos: Vec3, yaw: f32, s: Vec3) -> Mat4 {
        let (c, n) = (cos(yaw), sin(yaw));
        Mat4([
            c * s.x, 0.0, -n * s.x, 0.0, //
            0.0, s.y, 0.0, 0.0, //
            n * s.z, 0.0, c * s.z, 0.0, //
            pos.x, pos.y, pos.z, 1.0,
        ])
    }
}

// ---------------------------------------------------------------- geometry

/// One vertex: position, normal, colour (linear RGB 0..1). Layout is what the renderer reads.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vertex {
    pub pos: [f32; 3],
    pub normal: [f32; 3],
    pub color: [f32; 3],
}

#[derive(Clone, Debug, Default)]
pub struct Mesh {
    pub vertices: Vec<Vertex>,
    pub indices: Vec<u32>,
}

impl Mesh {
    pub fn new() -> Mesh {
        Mesh::default()
    }

    pub fn triangles(&self) -> usize {
        self.indices.len() / 3
    }

    /// Adds a quad with corners in counter-clockwise order (seen from the front).
    pub fn quad(&mut self, corners: [Vec3; 4], color: [f32; 3]) -> &mut Mesh {
        let n = (corners[1] - corners[0]).cross(corners[2] - corners[0]).normalize();
        let base = self.vertices.len() as u32;
        for c in corners {
            self.vertices.push(Vertex { pos: [c.x, c.y, c.z], normal: [n.x, n.y, n.z], color });
        }
        self.indices.extend_from_slice(&[base, base + 1, base + 2, base, base + 2, base + 3]);
        self
    }

    /// Adds an axis-aligned box centred at `center` with half sizes `half`.
    pub fn cuboid(&mut self, center: Vec3, half: Vec3, color: [f32; 3]) -> &mut Mesh {
        let (lo, hi) = (center - half, center + half);
        let p = |x: bool, y: bool, z: bool| {
            vec3(if x { hi.x } else { lo.x }, if y { hi.y } else { lo.y }, if z { hi.z } else { lo.z })
        };
        let (f, t) = (false, true);
        self.quad([p(f, f, t), p(t, f, t), p(t, t, t), p(f, t, t)], color); // +z
        self.quad([p(t, f, f), p(f, f, f), p(f, t, f), p(t, t, f)], color); // -z
        self.quad([p(t, f, t), p(t, f, f), p(t, t, f), p(t, t, t)], color); // +x
        self.quad([p(f, f, f), p(f, f, t), p(f, t, t), p(f, t, f)], color); // -x
        self.quad([p(f, t, t), p(t, t, t), p(t, t, f), p(f, t, f)], color); // +y
        self.quad([p(f, f, f), p(t, f, f), p(t, f, t), p(f, f, t)], color); // -y
        self
    }
}

/// Axis-aligned box, used for static collision.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Aabb {
    pub min: Vec3,
    pub max: Vec3,
}

impl Aabb {
    pub fn from_center(center: Vec3, half: Vec3) -> Aabb {
        Aabb { min: center - half, max: center + half }
    }

    /// Strictly inside on the ground plane (x and z), shrunk by `margin`.
    pub fn contains_xz(&self, p: Vec3, margin: f32) -> bool {
        p.x > self.min.x + margin && p.x < self.max.x - margin && p.z > self.min.z + margin && p.z < self.max.z - margin
    }
}

// ---------------------------------------------------------------- world

const CELL: f32 = 16.0;
const MAX_GRID_CELLS: i64 = 4096;

/// Grid cells covered by a box on the ground plane.
fn cells(min: Vec3, max: Vec3) -> (i32, i32, i32, i32) {
    let c = |v: f32| floor(v / CELL) as i32;
    (c(min.x), c(max.x), c(min.z), c(max.z))
}

/// Handle to a mesh registered with `World::add_mesh`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct MeshId(u32);

impl MeshId {
    pub fn index(self) -> usize {
        self.0 as usize
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Draw {
    pub mesh: MeshId,
    pub transform: Mat4,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera {
    pub eye: Vec3,
    pub target: Vec3,
    /// Vertical field of view in radians.
    pub fov_y: f32,
}

/// How the scene is lit. Modules may change it (day and night, weather).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Light {
    /// Direction towards the sun.
    pub sun_dir: Vec3,
    pub sun: [f32; 3],
    pub ambient: [f32; 3],
    /// Extra light on surfaces that face the sky.
    pub sky_light: [f32; 3],
    /// Colour of the sky, and of the fog.
    pub sky: [f32; 3],
    /// Fog distance in metres.
    pub fog: f32,
}

impl Default for Light {
    fn default() -> Light {
        Light {
            sun_dir: vec3(0.45, 0.85, 0.3).normalize(),
            sun: [0.6, 0.6, 0.6],
            ambient: [0.28, 0.28, 0.28],
            sky_light: [0.22, 0.22, 0.22],
            sky: [0.62, 0.75, 0.88],
            fog: 260.0,
        }
    }
}

/// Something drawn over the game this tick. Coordinates are fractions of the
/// screen, (0, 0) top left; text size is a fraction of the screen height.
#[derive(Clone, Debug, PartialEq)]
pub enum Hud {
    Rect { x: f32, y: f32, w: f32, h: f32, color: [f32; 4] },
    Text { x: f32, y: f32, size: f32, color: [f32; 4], text: Vec<u8> },
}

/// Sound: 48 kHz mono, one tick holds this many samples.
pub const SAMPLE_RATE: u32 = 48_000;
pub const SAMPLES_PER_TICK: usize = 800;

/// A number as text bytes for the HUD, for example `number_text(42.5, 1)` gives `42.5`.
pub fn number_text(v: f32, decimals: u32) -> Vec<u8> {
    let mut out = Vec::new();
    if !v.is_finite() {
        out.push(b'-');
        return out;
    }
    let scale = [1u64, 10, 100, 1000][decimals.min(3) as usize];
    let mut n = (abs(v) * scale as f32 + 0.5) as u64;
    if v < 0.0 && n > 0 {
        out.push(b'-');
    }
    let mut digits = Vec::new();
    let mut i = 0;
    while n > 0 || i <= decimals {
        if i == decimals && decimals > 0 {
            digits.push(b'.');
        }
        digits.push(b'0' + (n % 10) as u8);
        n /= 10;
        i += 1;
    }
    out.extend(digits.iter().rev());
    out
}

/// The player, whatever they are doing. Modules move it, the camera and
/// playtests read it. This is the contract playtests check against.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Avatar {
    pub pos: Vec3,
    pub yaw: f32,
    pub driving: bool,
}

pub struct World {
    /// Ticks since start. The only clock there is.
    pub tick: u64,
    pub input: Input,
    pub avatar: Avatar,
    pub camera: Camera,
    pub light: Light,
    colliders: Vec<Aabb>,
    /// Collider indices per 16 m cell, plus boxes too big for the grid.
    grid: BTreeMap<(i32, i32), Vec<u32>>,
    big: Vec<u32>,
    meshes: Vec<Mesh>,
    draws: Vec<Draw>,
    hud: Vec<Hud>,
    audio: [f32; SAMPLES_PER_TICK],
    rng: u64,
    state: BTreeMap<TypeId, Box<dyn Any>>,
}

impl World {
    pub fn new(seed: u64) -> World {
        World {
            tick: 0,
            input: Input::default(),
            avatar: Avatar::default(),
            camera: Camera { eye: vec3(0.0, 5.0, -10.0), target: Vec3::ZERO, fov_y: 1.0 },
            light: Light::default(),
            colliders: Vec::new(),
            grid: BTreeMap::new(),
            big: Vec::new(),
            meshes: Vec::new(),
            draws: Vec::new(),
            hud: Vec::new(),
            audio: [0.0; SAMPLES_PER_TICK],
            rng: seed ^ 0x9e37_79b9_7f4a_7c15,
            state: BTreeMap::new(),
        }
    }

    /// Registers a mesh for drawing. Meshes live forever, so make them once (in `init`).
    pub fn add_mesh(&mut self, mesh: Mesh) -> MeshId {
        self.meshes.push(mesh);
        MeshId(self.meshes.len() as u32 - 1)
    }

    /// Draws `mesh` this tick. The draw list is cleared every tick.
    pub fn draw(&mut self, mesh: MeshId, transform: Mat4) {
        self.draws.push(Draw { mesh, transform });
    }

    /// Draws a filled rectangle over the game this tick.
    pub fn hud_rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: [f32; 4]) {
        self.hud.push(Hud::Rect { x, y, w, h, color });
    }

    /// Draws text over the game this tick. Only printable ASCII is kept.
    pub fn hud_text(&mut self, x: f32, y: f32, size: f32, color: [f32; 4], text: &[u8]) {
        let text = text.iter().copied().filter(|c| (32..127).contains(c)).collect();
        self.hud.push(Hud::Text { x, y, size, color, text });
    }

    /// This tick's sound, SAMPLES_PER_TICK mono samples starting at silence.
    /// Add to it rather than overwrite, so several modules can play at once.
    pub fn audio_mut(&mut self) -> &mut [f32; SAMPLES_PER_TICK] {
        &mut self.audio
    }

    /// Adds a static collision box (a building, a wall). Boxes never move.
    pub fn add_collider(&mut self, b: Aabb) {
        let i = self.colliders.len() as u32;
        self.colliders.push(b);
        let (x0, x1, z0, z1) = cells(b.min, b.max);
        let count = (x1 as i64 - x0 as i64 + 1) * (z1 as i64 - z0 as i64 + 1);
        if !(1..=MAX_GRID_CELLS).contains(&count) {
            self.big.push(i);
            return;
        }
        for x in x0..=x1 {
            for z in z0..=z1 {
                self.grid.entry((x, z)).or_default().push(i);
            }
        }
    }

    pub fn colliders(&self) -> &[Aabb] {
        &self.colliders
    }

    /// Indices of colliders that may overlap a circle, in a fixed order.
    fn near(&self, pos: Vec3, radius: f32) -> Vec<u32> {
        let r = vec3(radius, 0.0, radius);
        let (x0, x1, z0, z1) = cells(pos - r, pos + r);
        let mut out = self.big.clone();
        if (x1 as i64 - x0 as i64 + 1) * (z1 as i64 - z0 as i64 + 1) > MAX_GRID_CELLS {
            return (0..self.colliders.len() as u32).collect();
        }
        for x in x0..=x1 {
            for z in z0..=z1 {
                if let Some(v) = self.grid.get(&(x, z)) {
                    out.extend_from_slice(v);
                }
            }
        }
        out.sort_unstable();
        out.dedup();
        out
    }

    /// Deterministic random number (SplitMix64).
    pub fn random(&mut self) -> u32 {
        self.rng = self.rng.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.rng;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        ((z ^ (z >> 31)) >> 32) as u32
    }

    /// Deterministic random number in [0, 1).
    pub fn random_f32(&mut self) -> f32 {
        (self.random() >> 8) as f32 / 16_777_216.0
    }

    /// Runs `f` with this module's own state, created with `Default` on first use.
    /// Any type works as a key, so define one struct per module.
    pub fn with<T: Any + Default, R>(&mut self, f: impl FnOnce(&mut T, &mut World) -> R) -> R {
        let id = TypeId::of::<T>();
        let mut boxed = self.state.remove(&id).unwrap_or_else(|| Box::new(T::default()));
        let r = match boxed.downcast_mut::<T>() {
            Some(s) => f(s, self),
            None => unreachable!(),
        };
        self.state.insert(id, boxed);
        r
    }

    /// Reads another module's state, if it exists.
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.state.get(&TypeId::of::<T>()).and_then(|b| b.downcast_ref::<T>())
    }

    /// Moves a circle of `radius` at `pos` out of every collider (ground plane only).
    /// Returns the corrected position and whether anything was hit.
    pub fn collide(&self, mut pos: Vec3, radius: f32) -> (Vec3, bool) {
        let mut hit = false;
        for _ in 0..2 {
            for i in self.near(pos, radius) {
                let b = &self.colliders[i as usize];
                let qx = pos.x.clamp(b.min.x, b.max.x);
                let qz = pos.z.clamp(b.min.z, b.max.z);
                let (dx, dz) = (pos.x - qx, pos.z - qz);
                let d2 = dx * dx + dz * dz;
                if d2 >= radius * radius {
                    continue;
                }
                hit = true;
                if d2 > 1e-12 {
                    let d = sqrt(d2);
                    pos.x = qx + dx / d * radius;
                    pos.z = qz + dz / d * radius;
                } else {
                    // Centre is inside: leave by the nearest side.
                    let exits = [
                        (pos.x - b.min.x, -1.0, 0.0),
                        (b.max.x - pos.x, 1.0, 0.0),
                        (pos.z - b.min.z, 0.0, -1.0),
                        (b.max.z - pos.z, 0.0, 1.0),
                    ];
                    let mut best = exits[0];
                    for e in exits {
                        if e.0 < best.0 {
                            best = e;
                        }
                    }
                    pos.x += best.1 * (best.0 + radius);
                    pos.z += best.2 * (best.0 + radius);
                }
            }
        }
        (pos, hit)
    }

    // ---- engine side: modules don't need these

    /// Starts a tick: advances the clock, sets input, clears draws, HUD and sound.
    pub fn begin_tick(&mut self, input: Input) {
        self.tick += 1;
        self.input = input;
        self.draws.clear();
        self.hud.clear();
        self.audio = [0.0; SAMPLES_PER_TICK];
    }
    pub fn hud(&self) -> &[Hud] {
        &self.hud
    }
    pub fn audio(&self) -> &[f32; SAMPLES_PER_TICK] {
        &self.audio
    }
    pub fn meshes(&self) -> &[Mesh] {
        &self.meshes
    }
    pub fn draws(&self) -> &[Draw] {
        &self.draws
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn math_is_close_enough() {
        let mut x = -20.0;
        while x < 20.0 {
            assert!(abs(sin(x) - libm_sin(x)) < 2e-6, "sin {x}");
            assert!(abs(cos(x) - libm_cos(x)) < 2e-6, "cos {x}");
            x += 0.01;
        }
        for v in [0.0001f32, 0.5, 1.0, 2.0, 9.0, 1234.5, 1e10] {
            let s = sqrt(v);
            assert!(abs(s * s - v) / v < 1e-6, "sqrt {v}");
        }
        for big in [1.0e7f32, -3.0e19, f32::MAX] {
            assert!(abs(sin(big)) <= 1.0 && abs(cos(big)) <= 1.0, "sin {big}");
        }
        assert!(sin(f32::NAN).is_nan() && sin(f32::INFINITY).is_nan());
        assert!(abs(wrap_angle(7.0 * PI) - PI).min(abs(wrap_angle(7.0 * PI) + PI)) < 1e-4);
        assert!(abs(wrap_angle(0.5) - 0.5) < 1e-6);
        assert_eq!(floor(-1.5), -2.0);
        assert_eq!(floor(2.0), 2.0);
        assert!(sqrt(-1.0).is_nan());
    }

    // Reference values come from f64 math via the host's std in tests only.
    extern crate std;
    fn libm_sin(x: f32) -> f32 {
        (x as f64).sin() as f32
    }
    fn libm_cos(x: f32) -> f32 {
        (x as f64).cos() as f32
    }

    #[test]
    fn collide_pushes_out() {
        let mut w = World::new(1);
        w.add_collider(Aabb::from_center(Vec3::ZERO, vec3(1.0, 1.0, 1.0)));
        w.add_collider(Aabb::from_center(vec3(100.0, 0.0, 0.0), vec3(1.0, 1.0, 1.0)));
        w.add_collider(Aabb::from_center(vec3(0.0, 0.0, 50.0), vec3(1.0e6, 1.0, 0.5))); // too big for the grid
        let (p, hit) = w.collide(vec3(0.2, 0.0, 0.1), 0.5);
        assert!(hit);
        assert!(!w.colliders()[0].contains_xz(p, 0.0));
        let (p, hit) = w.collide(vec3(100.3, 0.0, 0.9), 0.5);
        assert!(hit && !w.colliders()[1].contains_xz(p, 0.0), "found through the grid");
        let (p, hit) = w.collide(vec3(5.0, 0.0, 0.0), 0.5);
        assert!(!hit && p == vec3(5.0, 0.0, 0.0));
        let (p, hit) = w.collide(vec3(7000.0, 0.0, 50.2), 0.5);
        assert!(hit && !w.colliders()[2].contains_xz(p, 0.0), "found among the big ones");
    }

    #[test]
    fn number_text_works() {
        assert_eq!(number_text(42.0, 0), b"42".to_vec());
        assert_eq!(number_text(3.14159, 2), b"3.14".to_vec());
        assert_eq!(number_text(-0.5, 1), b"-0.5".to_vec());
        assert_eq!(number_text(0.0, 0), b"0".to_vec());
        assert_eq!(number_text(f32::NAN, 1), b"-".to_vec());
    }

    #[test]
    fn state_roundtrip() {
        #[derive(Default)]
        struct S(u32);
        let mut w = World::new(1);
        w.with::<S, _>(|s, _| s.0 = 7);
        assert_eq!(w.get::<S>().map(|s| s.0), Some(7));
    }
}
