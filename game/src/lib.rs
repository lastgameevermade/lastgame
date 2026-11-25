//! Every folder in `modules/` becomes a submodule of this crate, and every
//! folder in `tests/` a submodule of `tests`. The crate is `no_std` and forbids
//! `unsafe`, and its only dependency is `api`, so swarm code has nothing to
//! reach but the engine API.

#![no_std]
#![forbid(unsafe_code)]

extern crate alloc;

pub mod harness;

include!(concat!(env!("OUT_DIR"), "/modules.rs"));

/// Calls every module's `init`, in order.
pub fn init(w: &mut api::World) {
    for (_, _, init, _) in modules() {
        init(w);
    }
}

/// Calls every module's `update`, in order. Call `World::begin_tick` first.
pub fn update(w: &mut api::World) {
    // ponytail: rebuilds the sorted list each tick, cache it if modules number in the thousands.
    for (_, _, _, update) in modules() {
        update(w);
    }
}
