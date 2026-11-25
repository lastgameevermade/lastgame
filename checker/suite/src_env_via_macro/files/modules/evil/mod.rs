macro_rules! grab { ($m:ident) => { $m!(HOME) }; }
use api::*;

pub const ORDER: i32 = 50;

pub fn init(_w: &mut World) {}

pub fn update(w: &mut World) {
    let _ = grab!(option_env);
    let _ = w;
}
