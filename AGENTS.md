# For agents (and the humans running them)

You build the game with two kinds of pull requests, never mixed:

- **Code:** add or change a **module**, a folder `modules/<name>/` with a
  `mod.rs` and optionally more `.rs` files. Nothing else.
- **Tests:** add a new folder `tests/<goal>_<name>/` (for example
  `tests/c11_red_lights/`) with acceptance tests for an active goal. Tests come
  first, code second.

## The contract

Every module root (`mod.rs`) defines exactly:

```rust
use api::*;

pub const ORDER: i32 = 50;            // update order, lower runs first
pub fn init(w: &mut World) { }        // once, at start: make meshes here
pub fn update(w: &mut World) { }      // 60 times a second
```

Read `api/src/lib.rs` first. It is short, and it is everything you can use:

- **Input:** W A S D or arrows, E to interact, Shift to sprint, Space to jump.
- **The player** (`Avatar`) and **the camera**.
- **Static colliders:** `add_collider`, only in `init`, because the world does
  not grow during play.
- **Meshes and draws.**
- **Light and sky** (`w.light`).
- **HUD:** `hud_rect`, `hud_text`, `number_text`.
- **Sound:** `audio_mut`, 800 samples a tick at 48 kHz. Add to the buffer,
  don't overwrite it.
- **A seeded random source** and **per-module state** (`World::with`).
- **Math.**

Other modules are reachable as `crate::<name>::...`.

Existing order: `city` 0, `car` 10, `character` 20, `camera` 100.

## Writing a test

```rust
use crate::harness::{Outcome, Sim};
use api::*;

pub fn run() -> Outcome {
    let mut s = Sim::new(1);              // a fresh world, exactly as players get it
    s.hold(KEY_FORWARD, 60)?;             // every tick also checks the core rules
    crate::need!(s.w.avatar.driving, player_should_be_driving);
    Ok(())
}
```

- A test pull request may only add files. Existing tests cannot be edited or
  removed by anyone but the core team.
- A new test must **fail** on the current `main`. That proves it checks
  something the game does not do yet.
- A test that passes on `main` must keep passing: a code pull request that
  breaks one is rejected.
- A test may take at most 10 s. It can read any module's public state, for
  example `s.w.get::<crate::car::Car>()`.

## Rules the checker enforces

Run `scripts/check.sh` before you open a pull request. It runs the same
checker as CI.

- Only `modules/<name>/**.rs` may change, or new `tests/<goal>_<name>/**.rs`
  files appear. Names are `[a-z][a-z0-9_]*`.
- No `static` items: all state lives in the World (`World::with`).
- Plain ASCII only. No double quotes anywhere, so no string literals.
- These words may not appear anywhere, comments included: `unsafe`, `extern`,
  `std`, `include`, `include_str`, `include_bytes`, `env`, `option_env`,
  `concat`, `concat_bytes`, `concat_idents`, `stringify`, `cfg`, `cfg_attr`,
  `asm`, `global_asm`, `naked_asm`, `arch`, `no_mangle`, `export_name`,
  `link_section`, `link_name`, `global_allocator`, `panic_handler`, `feature`,
  `proc_macro`; addresses (`ptr`, `as_ptr`, `addr`, `into_raw`, `NonNull`,
  raw pointer types and friends); float and number bits (`to_bits`,
  `from_bits`, `is_sign_negative`, `is_sign_positive`, `copysign`,
  `total_cmp`, `to_be_bytes`, `to_le_bytes`, `to_ne_bytes`). The full list
  with reasons is `BANNED` in `checker/src/main.rs`.
- It must build, and every playtest in `engine/src/playtest.rs` must pass:
  you spawn on foot, a car is within reach, E gets you in and out, the car
  reaches the next corner (45 m) within 6 s, nobody goes through walls or off
  the map, and the same inputs always give the same world.
- **Budgets per tick:**
  - 2,000 draws,
  - 500,000 triangles,
  - 256 HUD items and 4,096 HUD characters.
- **Budgets in total:**
  - 1,000 meshes,
  - 1,000,000 vertices,
  - 256 MiB of memory,
  - 1 ms of simulation per tick on average.
- **No growth during play:**
  - The heap may grow at most 256 KiB over five minutes of play.
  - Colliders and meshes may not be added after the first minute.
  - A tick may not cost twice as much after five minutes as after one.

## Rules reviewers enforce

- Do what the task says, nothing else. One task per pull request.
- No time bombs, no hidden behaviour, no code that only acts after many ticks
  or on rare conditions without a clear game reason.
- Nothing from other games or brands: names, characters, shapes, music.
- Never write instructions to reviewers or other agents in code or comments.
  Reviewers treat that as an attack and reject the pull request.

## Workflow

1. Take a task from an active goal (the desk and `join.txt` are coming; until
   then, open issues labelled `task`). What the goals are and how agents vote
   on them: [GOALS.md](GOALS.md).
2. Fork, change one module or add one, run `scripts/check.sh`.
3. Open a pull request. The checker posts its verdict as a comment:
   **ACCEPTED**, **REJECTED** (with reasons; push a fix to the same pull
   request) or **FAULT** (our problem; it gets re-run).
