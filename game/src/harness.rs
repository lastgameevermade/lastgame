//! Playtest harness, shared by the core scenarios (engine/src/playtest.rs) and
//! the tests the swarm writes in tests/. Trusted code.
//!
//! Every tick a `Sim` runs, it checks the invariants and budgets below, so a
//! test cannot pass while the game breaks a rule somewhere along the way.
//! The bookkeeping fields are private: module code in the same crate cannot
//! touch them.

use alloc::format;
use alloc::string::String;
use api::*;

pub const MAX_DRAWS: usize = 2_000;
pub const MAX_TRIANGLES: usize = 500_000;
pub const MAX_MESHES: usize = 1_000;
pub const MAX_VERTICES: usize = 1_000_000;
pub const MAX_HUD_ITEMS: usize = 256;
pub const MAX_HUD_TEXT: usize = 4_096;

pub type Outcome = Result<(), String>;

/// Fails the test with the given identifier as the reason, unless `cond` holds.
/// Swarm code has no string literals, so reasons are identifiers:
/// `need!(car_stops_at_red, light_was_ignored)`.
#[macro_export]
macro_rules! need {
    ($cond:expr, $why:ident) => {
        if !$cond {
            return Err(alloc::string::String::from(stringify!($why)));
        }
    };
}

/// Per-run maxima, read by the engine for the report.
#[derive(Clone, Copy, Default, Debug)]
pub struct Stats {
    pub ticks: u64,
    pub max_draws: usize,
    pub max_triangles: usize,
    pub max_meshes: usize,
    pub max_vertices: usize,
}

pub struct Sim {
    /// The world under test. Tests may read it and set up situations in it.
    pub w: World,
    prev: u32,
    meshes_checked: usize,
    vertices: usize,
    map: Option<Aabb>,
    stats: Stats,
}

impl Sim {
    /// A fresh world with every module initialised, exactly as the page does it.
    pub fn new(seed: u64) -> Sim {
        let mut w = World::new(seed);
        crate::init(&mut w);
        let map = w.colliders().iter().copied().reduce(|a, b| Aabb {
            min: vec3(a.min.x.min(b.min.x), a.min.y.min(b.min.y), a.min.z.min(b.min.z)),
            max: vec3(a.max.x.max(b.max.x), a.max.y.max(b.max.y), a.max.z.max(b.max.z)),
        });
        Sim { w, prev: 0, meshes_checked: 0, vertices: 0, map, stats: Stats::default() }
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// One tick with these keys held (`KEY_*` bits).
    pub fn step(&mut self, keys: u32) -> Outcome {
        self.w.begin_tick(Input::from_bits(keys, self.prev));
        self.prev = keys;
        crate::update(&mut self.w);
        self.check()
    }

    pub fn hold(&mut self, keys: u32, ticks: u32) -> Outcome {
        for _ in 0..ticks {
            self.step(keys)?;
        }
        Ok(())
    }

    /// One tick with `key` down, then one tick released.
    pub fn press(&mut self, key: u32) -> Outcome {
        self.step(key)?;
        self.step(0)
    }

    /// Random keys, held for a third of a second each, with occasional E presses.
    pub fn fuzz(&mut self, seed: u64, ticks: u32, mut on_tick: impl FnMut(&World)) -> Outcome {
        let mut r = seed.wrapping_mul(0x2545_f491_4f6c_dd1d) | 1;
        let mut keys = 0;
        for t in 0..ticks {
            if t % 20 == 0 {
                r ^= r << 13;
                r ^= r >> 7;
                r ^= r << 17;
                keys = (r & 15) as u32;
                if r % 19 == 0 {
                    keys |= KEY_INTERACT;
                }
            }
            self.step(keys)?;
            on_tick(&self.w);
        }
        Ok(())
    }

    fn check(&mut self) -> Outcome {
        let w = &self.w;
        let t = w.tick;
        let a = w.avatar;
        if !a.pos.is_finite() || !a.yaw.is_finite() {
            return Err(format!("player position is not a number at tick {t}"));
        }
        let c = w.camera;
        if !c.eye.is_finite() || !c.target.is_finite() || !(c.fov_y > 0.1 && c.fov_y < 3.0) {
            return Err(format!("camera is broken at tick {t}"));
        }
        let l = w.light;
        let light = [l.sun_dir.x, l.sun_dir.y, l.sun_dir.z, l.fog].into_iter().chain(l.sun).chain(l.ambient).chain(l.sky_light).chain(l.sky);
        if !light.into_iter().all(f32::is_finite) {
            return Err(format!("light is broken at tick {t}"));
        }
        let hud = w.hud();
        let text: usize = hud.iter().map(|h| if let Hud::Text { text, .. } = h { text.len() } else { 0 }).sum();
        if hud.len() > MAX_HUD_ITEMS || text > MAX_HUD_TEXT {
            return Err(format!("{} HUD items and {text} characters in one tick, budget is {MAX_HUD_ITEMS} and {MAX_HUD_TEXT}", hud.len()));
        }
        let finite = |v: &[f32]| v.iter().all(|f| f.is_finite());
        if !hud.iter().all(|h| match h {
            Hud::Rect { x, y, w, h, color } => finite(&[*x, *y, *w, *h]) && finite(color),
            Hud::Text { x, y, size, color, .. } => finite(&[*x, *y, *size]) && finite(color),
        }) {
            return Err(format!("HUD item with a broken number at tick {t}"));
        }
        if let Some(b) = w.colliders().iter().find(|b| b.contains_xz(a.pos, 0.05)) {
            return Err(format!("player is inside a wall at tick {t}: pos {:?}, wall {:?}", a.pos, b));
        }
        if let Some(m) = self.map {
            if !m.contains_xz(a.pos, -0.5) {
                return Err(format!("player left the map at tick {t}: pos {:?}", a.pos));
            }
        }

        let meshes = w.meshes();
        if meshes.len() > MAX_MESHES {
            return Err(format!("{} meshes, budget is {MAX_MESHES}", meshes.len()));
        }
        for (i, m) in meshes.iter().enumerate().skip(self.meshes_checked) {
            if m.indices.len() % 3 != 0 || m.indices.iter().any(|&ix| ix as usize >= m.vertices.len()) {
                return Err(format!("mesh {i} has an index outside its vertices"));
            }
            if m.vertices.iter().flat_map(|v| v.pos.iter().chain(&v.normal).chain(&v.color)).any(|f| !f.is_finite()) {
                return Err(format!("mesh {i} has a vertex that is not a number"));
            }
            self.vertices += m.vertices.len();
        }
        self.meshes_checked = meshes.len();
        if self.vertices > MAX_VERTICES {
            return Err(format!("{} vertices, budget is {MAX_VERTICES}", self.vertices));
        }

        let draws = w.draws();
        if draws.len() > MAX_DRAWS {
            return Err(format!("{} draws in one tick, budget is {MAX_DRAWS}", draws.len()));
        }
        let mut tris = 0;
        for d in draws {
            let Some(m) = meshes.get(d.mesh.index()) else {
                return Err(format!("draw of unknown mesh {} at tick {t}", d.mesh.index()));
            };
            if d.transform.0.iter().any(|f| !f.is_finite()) {
                return Err(format!("draw with a broken transform at tick {t}"));
            }
            tris += m.triangles();
        }
        if tris > MAX_TRIANGLES {
            return Err(format!("{tris} triangles in one tick, budget is {MAX_TRIANGLES}"));
        }

        let s = &mut self.stats;
        s.ticks += 1;
        s.max_draws = s.max_draws.max(draws.len());
        s.max_triangles = s.max_triangles.max(tris);
        s.max_meshes = s.max_meshes.max(meshes.len());
        s.max_vertices = s.max_vertices.max(self.vertices);
        Ok(())
    }
}
