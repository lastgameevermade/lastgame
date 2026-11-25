//! Acceptance playtests: the core contract plus every test the swarm wrote in
//! tests/. They live inside the engine, so the checker tests the exact
//! `game.wasm` the browser runs, byte for byte, with no imports. The runner
//! (checker/playtest.mjs) gives every scenario a fresh instance, so nothing
//! carries over from one scenario to the next.
//!
//! Exports, never used by the page:
//! - `pt_count()`, `pt_name(i)`/`pt_name_len(i)`, `pt_is_core(i)`
//! - `pt_run(i)` returns 1 pass / 0 fail
//! - `pt_ticks()` ticks the scenario simulated, for the CPU budget
//! - `pt_report()`/`pt_report_len()` with `PASS`/`FAIL`/`METRIC` lines

use api::*;
use game::harness::{MAX_DRAWS, MAX_MESHES, MAX_TRIANGLES, MAX_VERTICES, Outcome, Sim, Stats};
use std::cell::RefCell;

/// Live heap may grow at most this much over five minutes of play after warm-up.
const MAX_LEAK_BYTES: usize = 256 * 1024;

#[derive(Default)]
struct Run {
    report: String,
    stats: Stats,
    leak: Option<usize>,
}

thread_local! {
    static RUN: RefCell<Run> = RefCell::new(Run::default());
}

/// Records a finished simulation's numbers for this run's report.
fn keep(s: &Sim) {
    let n = s.stats();
    RUN.with_borrow_mut(|r| {
        let m = &mut r.stats;
        m.ticks += n.ticks;
        m.max_draws = m.max_draws.max(n.max_draws);
        m.max_triangles = m.max_triangles.max(n.max_triangles);
        m.max_meshes = m.max_meshes.max(n.max_meshes);
        m.max_vertices = m.max_vertices.max(n.max_vertices);
    });
}

fn need(ok: bool, why: impl FnOnce() -> String) -> Outcome {
    if ok { Ok(()) } else { Err(why()) }
}

// ---------------------------------------------------------------- core scenarios

fn spawn() -> Outcome {
    let mut s = Sim::new(1);
    s.step(0)?;
    need(!s.w.avatar.driving, || "player starts inside the car".into())?;
    need(!s.w.draws().is_empty(), || "nothing is drawn".into())
}

fn idle() -> Outcome {
    let mut s = Sim::new(1);
    s.step(0)?;
    let start = s.w.avatar.pos;
    s.hold(0, 600)?;
    keep(&s);
    need(s.w.avatar.pos.distance_xz(start) < 0.01, || "player drifts with no input".into())
}

fn walk() -> Outcome {
    let mut s = Sim::new(1);
    let start = s.w.avatar.pos;
    s.hold(KEY_FORWARD, 180)?;
    let d = s.w.avatar.pos.distance_xz(start);
    need(d >= 8.0, || format!("walked only {d:.1} m in 3 s"))?;
    need(!s.w.avatar.driving, || "walking got the player into the car".into())
}

fn turn() -> Outcome {
    let mut s = Sim::new(1);
    let (start, yaw) = (s.w.avatar.pos, s.w.avatar.yaw);
    s.hold(KEY_LEFT, 60)?;
    let turned = abs(s.w.avatar.yaw - yaw);
    need(turned >= 1.0, || format!("turned only {turned:.2} rad in 1 s"))?;
    need(s.w.avatar.pos.distance_xz(start) < 0.5, || "turning on the spot moved the player".into())
}

fn enter(s: &mut Sim) -> Outcome {
    s.press(KEY_INTERACT)?;
    need(s.w.avatar.driving, || "pressing E at spawn did not get the player into the car".into())
}

fn enter_car() -> Outcome {
    enter(&mut Sim::new(1))
}

fn drive_to_corner() -> Outcome {
    let mut s = Sim::new(1);
    enter(&mut s)?;
    let start = s.w.avatar.pos;
    for _ in 0..360 {
        s.step(KEY_FORWARD)?;
        if s.w.avatar.pos.distance_xz(start) >= 45.0 {
            return need(s.w.avatar.driving, || "player fell out of the car".into());
        }
    }
    Err(format!("car got only {:.1} m in 6 s, the corner is 45 m away", s.w.avatar.pos.distance_xz(start)))
}

fn exit_car() -> Outcome {
    let mut s = Sim::new(1);
    enter(&mut s)?;
    s.hold(KEY_FORWARD, 120)?;
    s.hold(0, 240)?;
    let car = s.w.avatar.pos;
    s.press(KEY_INTERACT)?;
    need(!s.w.avatar.driving, || "pressing E in the car did not get the player out".into())?;
    let d = s.w.avatar.pos.distance_xz(car);
    need(d < 5.0, || format!("player got out {d:.1} m away from the car"))?;
    s.hold(KEY_FORWARD, 60)?;
    need(s.w.avatar.pos.distance_xz(car) > d, || "player cannot walk after getting out".into())
}

fn walls_hold() -> Outcome {
    let mut s = Sim::new(1);
    enter(&mut s)?;
    s.hold(KEY_FORWARD | KEY_RIGHT, 1200)?;
    s.hold(KEY_FORWARD | KEY_LEFT, 1200)?;
    s.hold(KEY_FORWARD, 1200)?;
    s.press(KEY_INTERACT)?;
    s.hold(KEY_FORWARD, 1200)?;
    keep(&s);
    Ok(())
}

fn fuzz_six_minutes() -> Outcome {
    for seed in 1..=3 {
        let mut s = Sim::new(seed);
        s.fuzz(seed, 2 * 60 * 60, |_| {})?;
        keep(&s);
    }
    Ok(())
}

fn deterministic() -> Outcome {
    let run = || {
        let mut h: u64 = 0xcbf2_9ce4_8422_2325;
        let mut mix = |f: f32| h = (h ^ f.to_bits() as u64).wrapping_mul(0x100_0000_01b3);
        let mut s = Sim::new(11);
        s.fuzz(11, 60 * 60, |w| {
            let (a, c) = (w.avatar, w.camera);
            for f in [a.pos.x, a.pos.y, a.pos.z, a.yaw, c.eye.x, c.eye.y, c.eye.z, c.target.x, c.target.z] {
                mix(f);
            }
            for d in w.draws() {
                d.transform.0.iter().for_each(|&f| mix(f));
            }
        })?;
        keep(&s);
        Ok::<u64, String>(h)
    };
    let a = run()?;
    // Shift the heap between the runs: behaviour that depends on where things
    // happen to be allocated would show up as a different world.
    let shift = vec![0u8; 12_345];
    let b = run()?;
    drop(shift);
    need(a == b, || "two runs with the same seed and input ended in different worlds".into())
}

fn no_leak() -> Outcome {
    let mut s = Sim::new(5);
    s.fuzz(5, 60 * 60, |_| {})?;
    let before = crate::live_bytes();
    let (colliders, meshes) = (s.w.colliders().len(), s.w.meshes().len());
    s.fuzz(6, 5 * 60 * 60, |_| {})?;
    keep(&s);
    let grew = crate::live_bytes().saturating_sub(before);
    RUN.with_borrow_mut(|r| r.leak = Some(grew));
    // The world is static: what exists after a minute of play is all there is.
    need(s.w.colliders().len() == colliders, || format!("collider count grew during play: {colliders} to {}", s.w.colliders().len()))?;
    need(s.w.meshes().len() == meshes, || format!("mesh count grew during play: {meshes} to {}", s.w.meshes().len()))?;
    need(grew <= MAX_LEAK_BYTES, || format!("heap grew by {grew} bytes over five minutes of play"))
}

/// The same play, one minute and five minutes long. The runner compares their
/// cost per tick: a game that gets slower the longer it runs fails.
fn growth_short() -> Outcome {
    let mut s = Sim::new(7);
    s.fuzz(7, 60 * 60, |_| {})?;
    keep(&s);
    Ok(())
}

fn growth_long() -> Outcome {
    let mut s = Sim::new(7);
    s.fuzz(7, 5 * 60 * 60, |_| {})?;
    keep(&s);
    Ok(())
}

const CORE: &[(&str, fn() -> Outcome)] = &[
    ("spawn", spawn),
    ("idle", idle),
    ("walk", walk),
    ("turn", turn),
    ("enter_car", enter_car),
    ("drive_to_corner", drive_to_corner),
    ("exit_car", exit_car),
    ("walls_hold", walls_hold),
    ("fuzz_six_minutes", fuzz_six_minutes),
    ("deterministic", deterministic),
    ("no_leak", no_leak),
    ("growth_short", growth_short),
    ("growth_long", growth_long),
];

/// Core scenarios are named `core/<name>`, swarm tests `test/<folder>`.
fn scenario(i: u32) -> (String, fn() -> Outcome) {
    let i = i as usize;
    match CORE.get(i) {
        Some(&(n, f)) => (format!("core/{n}"), f),
        None => {
            let (n, f) = game::tests()[i - CORE.len()];
            (format!("test/{n}"), f)
        }
    }
}

// ---------------------------------------------------------------- exports

#[unsafe(no_mangle)]
pub extern "C" fn pt_count() -> u32 {
    (CORE.len() + game::tests().len()) as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_is_core(i: u32) -> u32 {
    ((i as usize) < CORE.len()) as u32
}

/// Writes the name into the report buffer; read it before calling `pt_run`.
#[unsafe(no_mangle)]
pub extern "C" fn pt_name(i: u32) -> *const u8 {
    let name = scenario(i).0;
    RUN.with_borrow_mut(|r| {
        r.report = name;
        r.report.as_ptr()
    })
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_name_len(i: u32) -> u32 {
    scenario(i).0.len() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_run(i: u32) -> u32 {
    let (name, f) = scenario(i);
    RUN.with_borrow_mut(|r| *r = Run::default());
    let result = f();
    RUN.with_borrow_mut(|r| {
        match &result {
            Ok(()) => r.report += &format!("PASS {name}\n"),
            Err(e) => r.report += &format!("FAIL {name}: {e}\n"),
        }
        let s = r.stats;
        if s.ticks > 0 {
            r.report += &format!(
                "METRIC ticks {}\nMETRIC max_draws {} {MAX_DRAWS}\nMETRIC max_triangles {} {MAX_TRIANGLES}\n\
                 METRIC meshes {} {MAX_MESHES}\nMETRIC vertices {} {MAX_VERTICES}\n",
                s.ticks, s.max_draws, s.max_triangles, s.max_meshes, s.max_vertices
            );
        }
        if let Some(l) = r.leak {
            r.report += &format!("METRIC leak_bytes {l} {MAX_LEAK_BYTES}\n");
        }
    });
    result.is_ok() as u32
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_ticks() -> u32 {
    RUN.with_borrow(|r| r.stats.ticks as u32)
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_report() -> *const u8 {
    RUN.with_borrow(|r| r.report.as_ptr())
}

#[unsafe(no_mangle)]
pub extern "C" fn pt_report_len() -> u32 {
    RUN.with_borrow(|r| r.report.len() as u32)
}
