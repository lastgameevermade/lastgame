//! The machine judge. One command, one verdict: ACCEPTED, REJECTED or FAULT.
//!
//!     checker [--changed FILE] [--base DIR] [--out FILE]
//!
//! Run from the repo root, inside the build image with networking off
//! (scripts/check.sh does both). `--changed` lists what a pull request
//! touches, one `<status>\t<path>` per line (`git diff --name-status`): either
//! module files, or new swarm tests, never both. `--base` is a checkout of
//! `main` to compare swarm test results with (the ratchet): a test that passes
//! on main must keep passing, and a new test must not already pass. Both
//! builds are then drawn from fixed views into shots/ (checker/shots.mjs), so
//! reviewers see what a change does to the picture; that never sets the verdict.
//!
//! Order matters: nothing gets compiled until the source rules pass, and
//! module code only ever runs inside a wasm instance with no imports.
//! Exit code: 0 ACCEPTED, 1 REJECTED, 2 FAULT (our problem, not yours).

use std::collections::BTreeMap;
use std::fmt::Write as _;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

const MAX_FILE_BYTES: u64 = 64 * 1024;
const MAX_MODULE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_FILES: usize = 2_000;
const MAX_CHANGED: usize = 300;
const MAX_DEPTH: usize = 4;
const MAX_WASM_BYTES: usize = 4 * 1024 * 1024;
/// 256 MiB in 64 KiB wasm pages. The binary itself must refuse to grow past it.
const MAX_MEMORY_PAGES: u32 = 4096;
/// Average simulation cost per tick: 6 % of a 60 Hz frame on the CI machine.
const MAX_MS_PER_TICK: f64 = 1.0;
const MAX_MEMORY_BYTES: f64 = 256.0 * 1024.0 * 1024.0;
const BUILD_TIMEOUT: u64 = 600;
const PLAYTEST_TIMEOUT: u64 = 120;
/// A single swarm-written test may run at most this long.
const MAX_TEST_SECONDS: u64 = 10;
/// A research entry may take at most this long to render one benchmark scene.
const MAX_BENCH_MS: u64 = 1000;
const BENCH_TIMEOUT: u64 = 300;
const SHOTS_TIMEOUT: u64 = 60;
/// Research goals that have a benchmark harness (engine/src/bench.rs). Entries live in bench/<goal>_<name>/.
const BENCH_GOALS: &[&str] = &["r1"];

/// Identifiers module code may not contain anywhere, comments included.
/// Each one is a way out of the sandbox, a way to read the build machine at
/// compile time, or a way to behave differently on another platform.
const BANNED: &[(&str, &str)] = &[
    ("unsafe", "unsafe code"),
    ("extern", "foreign code and extra crates"),
    ("std", "the standard library (modules are no_std)"),
    ("include", "reading files at compile time"),
    ("include_str", "reading files at compile time"),
    ("include_bytes", "reading files at compile time"),
    ("env", "reading the build environment"),
    ("option_env", "reading the build environment"),
    ("concat", "building paths for compile time reads"),
    ("concat_bytes", "building paths for compile time reads"),
    ("concat_idents", "building names for compile time tricks"),
    ("stringify", "building paths for compile time reads"),
    ("cfg", "platform dependent code (tests run the same build the browser runs)"),
    ("cfg_attr", "platform dependent code"),
    ("asm", "inline assembly"),
    ("global_asm", "inline assembly"),
    ("naked_asm", "inline assembly"),
    ("arch", "platform intrinsics"),
    ("no_mangle", "exporting symbols"),
    ("export_name", "exporting symbols"),
    ("link_section", "linker tricks"),
    ("link_name", "linker tricks"),
    ("global_allocator", "replacing the allocator"),
    ("panic_handler", "replacing the panic handler"),
    ("feature", "unstable compiler features"),
    ("proc_macro", "procedural macros"),
    // Addresses: where something sits in memory is not game state, and it
    // differs between a test run and a player's browser.
    ("ptr", "raw pointers and addresses"),
    ("as_ptr", "raw pointers and addresses"),
    ("as_mut_ptr", "raw pointers and addresses"),
    ("as_ptr_range", "raw pointers and addresses"),
    ("as_mut_ptr_range", "raw pointers and addresses"),
    ("addr", "raw pointers and addresses"),
    ("addr_of", "raw pointers and addresses"),
    ("addr_of_mut", "raw pointers and addresses"),
    ("into_raw", "raw pointers and addresses"),
    ("into_raw_parts", "raw pointers and addresses"),
    ("from_raw", "raw pointers and addresses"),
    ("NonNull", "raw pointers and addresses"),
    ("expose_provenance", "raw pointers and addresses"),
    ("with_exposed_provenance", "raw pointers and addresses"),
    ("without_provenance", "raw pointers and addresses"),
    ("map_addr", "raw pointers and addresses"),
    ("with_addr", "raw pointers and addresses"),
    // Float bits: NaN sign and payload differ between CPUs, so reading them
    // tells an x86 test machine from an ARM phone.
    ("to_bits", "reading float bits (they differ between CPUs for NaN)"),
    ("from_bits", "building floats from bits"),
    ("is_sign_negative", "reading the sign of NaN (differs between CPUs)"),
    ("is_sign_positive", "reading the sign of NaN (differs between CPUs)"),
    ("copysign", "reading the sign of NaN (differs between CPUs)"),
    ("total_cmp", "ordering by float bits; use partial_cmp"),
    ("to_be_bytes", "reading raw bytes of numbers"),
    ("to_le_bytes", "reading raw bytes of numbers"),
    ("to_ne_bytes", "reading raw bytes of numbers"),
];

/// Everything the browser build may export. Anything else is a smuggled entry point.
const ENGINE_EXPORTS: &[&str] = &[
    "memory", "__data_end", "__heap_base", "lg_init", "lg_step", "lg_tick", "lg_frame", "lg_frame_len",
    "lg_mesh_count", "lg_mesh_vertices", "lg_mesh_vertices_len", "lg_mesh_indices", "lg_mesh_indices_len",
    "lg_audio", "lg_audio_len", "lg_audio_clear",
    "pt_count", "pt_is_core", "pt_name", "pt_name_len", "pt_run", "pt_ticks", "pt_report", "pt_report_len",
];

/// What bench.wasm may export on top of the game's own exports. It is never shipped to players.
const BENCH_EXPORTS: &[&str] =
    &["bn_count", "bn_name", "bn_name_len", "bn_scene", "bn_reference", "bn_render", "bn_error", "bn_hash"];

/// Folder names that would shadow crates or keywords. Keep in sync with game/build.rs.
const RESERVED: &[&str] = &[
    "alloc", "api", "bench", "benches", "core", "crate", "game", "harness", "self", "std", "super", "test", "tests",
    "as", "async",
    "await", "break",
    "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for", "gen", "if", "impl", "in", "let",
    "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static", "struct", "trait", "true", "type",
    "unsafe", "use", "where", "while", "abstract", "become", "box", "do", "final", "macro", "override", "priv",
    "try", "typeof", "unsized", "virtual", "yield",
];

#[derive(Clone, Copy, PartialEq, Debug)]
enum Verdict {
    Accepted,
    Rejected,
    Fault,
}

struct Report {
    verdict: Verdict,
    stage: &'static str,
    reasons: Vec<String>,
    metrics: BTreeMap<String, String>,
}

impl Report {
    fn reject(&mut self, why: impl Into<String>) {
        self.verdict = Verdict::Rejected;
        self.reasons.push(why.into());
    }
    fn fault(&mut self, why: impl Into<String>) {
        self.verdict = Verdict::Fault;
        self.reasons.push(why.into());
    }
}

fn main() {
    let mut changed = None;
    let mut base = None;
    let mut out = None;
    let mut args = std::env::args().skip(1);
    while let Some(a) = args.next() {
        match a.as_str() {
            "--changed" => changed = args.next(),
            "--base" => base = args.next(),
            "--out" => out = args.next(),
            _ => {
                eprintln!("usage: checker [--changed FILE] [--base DIR] [--out FILE]");
                std::process::exit(2);
            }
        }
    }

    let mut r = Report { verdict: Verdict::Accepted, stage: "paths", reasons: Vec::new(), metrics: BTreeMap::new() };
    run(&mut r, changed.as_deref(), base.as_deref());

    let word = match r.verdict {
        Verdict::Accepted => "ACCEPTED",
        Verdict::Rejected => "REJECTED",
        Verdict::Fault => "FAULT",
    };
    println!("{word} (stage: {})", r.stage);
    for why in &r.reasons {
        println!("  - {why}");
    }
    for (k, v) in &r.metrics {
        println!("  {k} = {v}");
    }
    if let Some(path) = out {
        let json = format!(
            "{{\"verdict\":\"{word}\",\"stage\":\"{}\",\"reasons\":[{}],\"metrics\":{{{}}}}}\n",
            r.stage,
            r.reasons.iter().map(|s| json_str(s)).collect::<Vec<_>>().join(","),
            r.metrics.iter().map(|(k, v)| format!("{}:{}", json_str(k), json_str(v))).collect::<Vec<_>>().join(",")
        );
        if let Err(e) = fs::write(&path, json) {
            eprintln!("cannot write {path}: {e}");
            std::process::exit(2);
        }
    }
    std::process::exit(match r.verdict {
        Verdict::Accepted => 0,
        Verdict::Rejected => 1,
        Verdict::Fault => 2,
    });
}

fn run(r: &mut Report, changed: Option<&str>, base: Option<&str>) {
    let mut adds_tests = false;
    // Which research entries to benchmark: the ones a pull request touches, or all of them on main.
    let mut bench: Option<Vec<String>> = None;
    if let Some(list) = changed {
        match fs::read_to_string(list) {
            Ok(text) => (adds_tests, bench) = check_changed(r, &text).map_or((false, None), |(t, b)| (t, Some(b))),
            Err(e) => return r.fault(format!("cannot read changed file list {list}: {e}")),
        }
        if r.verdict != Verdict::Accepted {
            return;
        }
    }

    r.stage = "source";
    scan_tree(r, Path::new("modules"), "modules");
    scan_tree(r, Path::new("tests"), "tests");
    scan_tree(r, Path::new("bench"), "bench");
    r.metrics.insert("benchmarks".into(), BENCH_GOALS.join(" "));
    if r.verdict != Verdict::Accepted {
        return;
    }

    let target = std::env::var("CARGO_TARGET_DIR").unwrap_or_else(|_| "target".into());
    let Some(now) = play(r, Path::new("."), Path::new(&target), true) else { return };
    let names = |want: bool| now.iter().filter(|(_, ok)| **ok == want).map(|(n, _)| n.as_str()).collect::<Vec<_>>().join(" ");
    r.metrics.insert("tests_green".into(), names(true));
    r.metrics.insert("tests_red".into(), names(false));
    benchmarks(r, Path::new(&target), bench.as_deref());
    if r.verdict != Verdict::Accepted {
        return;
    }

    let Some(base) = base else { return };
    r.stage = "ratchet";
    let mut scratch = Report { verdict: Verdict::Accepted, stage: "base", reasons: Vec::new(), metrics: BTreeMap::new() };
    let base_target = Path::new(&target).join("base");
    let Some(before) = play(&mut scratch, Path::new(base), &base_target, false) else {
        return r.fault(format!("main does not pass its own checks: {}", scratch.reasons.join("; ")));
    };
    let mut fixed = Vec::new();
    for (name, &passes) in &now {
        match before.get(name) {
            Some(true) if !passes => r.reject(format!("{name} passes on main and fails here")),
            Some(false) if passes => fixed.push(name.as_str()),
            None if adds_tests && passes => {
                r.reject(format!("{name} already passes on main, so it checks nothing new"))
            }
            _ => {}
        }
    }
    r.metrics.insert("tests_fixed".into(), fixed.join(" "));
    shots(r, &engine_wasm(Path::new(base), &base_target), &engine_wasm(Path::new("."), Path::new(&target)));
}

/// Before and after pictures of main and the pull request (checker/shots.mjs) in shots/, and how
/// much of the picture changed. They only inform reviewers: nothing here touches the verdict.
fn shots(r: &mut Report, before: &Path, after: &Path) {
    let _ = fs::remove_dir_all("shots");
    let mut node = Command::new("node");
    node.arg("checker/shots.mjs").arg(before).arg(after).arg("shots");
    let why = match run_with_timeout(&mut node, SHOTS_TIMEOUT) {
        Err(e) => format!("cannot run node: {e}"),
        Ok(o) if o.timed_out => format!("shots took longer than {SHOTS_TIMEOUT} s"),
        Ok(o) if o.success && o.stdout.lines().any(|l| l == "DONE") => {
            for (k, v) in o.stdout.lines().filter_map(|l| l.strip_prefix("METRIC ")?.split_once(' ')) {
                r.metrics.insert(k.into(), v.into());
            }
            return;
        }
        Ok(o) => format!("shots failed:\n{}", tail(&format!("{}{}", o.stdout, o.stderr), 10, 1000)),
    };
    r.metrics.insert("why shots".into(), why);
}

/// Where `play` leaves the build of `dir`.
fn engine_wasm(dir: &Path, target: &Path) -> PathBuf {
    dir.join(target).join("wasm32-unknown-unknown/release/engine.wasm")
}

/// Builds `dir`, checks the wasm and runs every playtest. Core failures and
/// budget breaches are rejections; swarm test results come back as a map.
fn play(r: &mut Report, dir: &Path, target: &Path, record: bool) -> Option<BTreeMap<String, bool>> {
    r.stage = "build";
    let mut build = Command::new("cargo");
    build.current_dir(dir).env("CARGO_TARGET_DIR", target).env("CARGO_TERM_COLOR", "never");
    build.args(["build", "--release", "--locked", "--offline", "--target", "wasm32-unknown-unknown", "-p", "engine"]);
    match run_with_timeout(&mut build, BUILD_TIMEOUT) {
        Err(e) => {
            r.fault(format!("cannot run cargo: {e}"));
            return None;
        }
        Ok(o) if o.timed_out => {
            r.reject(format!("build took longer than {BUILD_TIMEOUT} s"));
            return None;
        }
        Ok(o) if !o.success => {
            r.reject(format!("build failed:\n{}", tail(&o.stderr, 60, 6000)));
            return None;
        }
        Ok(_) => {}
    }

    r.stage = "wasm";
    let wasm = engine_wasm(dir, target);
    let bytes = match fs::read(&wasm) {
        Ok(b) => b,
        Err(e) => {
            r.fault(format!("build produced no engine.wasm: {e}"));
            return None;
        }
    };
    if record {
        r.metrics.insert("wasm_bytes".into(), bytes.len().to_string());
    }
    if bytes.len() > MAX_WASM_BYTES {
        r.reject(format!("game.wasm is {} bytes, budget is {MAX_WASM_BYTES}", bytes.len()));
    }
    match wasm_interface(&bytes) {
        Err(e) => r.fault(format!("game.wasm does not parse: {e}")),
        Ok((imports, exports, max_pages)) => {
            match max_pages {
                Some(p) if p <= MAX_MEMORY_PAGES => {}
                _ => r.reject(format!("game.wasm memory limit is {max_pages:?} pages, must be at most {MAX_MEMORY_PAGES}")),
            }
            if !imports.is_empty() {
                r.reject(format!("game.wasm imports {imports:?}; the game may not import anything"));
            }
            for e in exports.iter().filter(|e| !ENGINE_EXPORTS.contains(&e.as_str())) {
                r.reject(format!("game.wasm exports unexpected symbol {e:?}"));
            }
        }
    }
    if r.verdict != Verdict::Accepted {
        return None;
    }

    r.stage = "playtest";
    let mut node = Command::new("node");
    node.current_dir(dir).arg("checker/playtest.mjs").arg(&wasm);
    node.arg(MAX_MS_PER_TICK.to_string()).arg(MAX_TEST_SECONDS.to_string());
    let o = match run_with_timeout(&mut node, PLAYTEST_TIMEOUT) {
        Err(e) => {
            r.fault(format!("cannot run node: {e}"));
            return None;
        }
        Ok(o) => o,
    };
    if o.timed_out {
        r.reject(format!("playtests took longer than {PLAYTEST_TIMEOUT} s (an endless loop?)"));
        return None;
    }
    let mut tests = BTreeMap::new();
    let mut core_passed = 0;
    let mut finished = false;
    for line in o.stdout.lines() {
        if let Some(name) = line.strip_prefix("PASS ") {
            match name.strip_prefix("test/") {
                Some(t) => {
                    tests.entry(t.to_string()).or_insert(true);
                }
                None => core_passed += 1,
            }
        } else if let Some(rest) = line.strip_prefix("FAIL ") {
            let name = rest.split(':').next().unwrap_or("");
            match name.strip_prefix("test/") {
                Some(t) => {
                    tests.insert(t.to_string(), false);
                    if record {
                        r.metrics.insert(format!("why {name}"), rest.to_string());
                    }
                }
                None => r.reject(format!("playtest {rest}")),
            }
        } else if let Some(rest) = line.strip_prefix("METRIC ") {
            let mut it = rest.split_whitespace();
            let (Some(k), Some(v)) = (it.next(), it.next()) else { continue };
            if record {
                r.metrics.insert(k.into(), v.into());
            }
            let value: f64 = v.parse().unwrap_or(f64::INFINITY);
            let budget = match (k, it.next()) {
                ("memory_bytes", _) => Some(MAX_MEMORY_BYTES),
                (_, Some(b)) => b.parse().ok(),
                _ => None,
            };
            if let Some(b) = budget {
                if value > b {
                    r.reject(format!("{k} is {v}, budget is {b}"));
                }
            }
        } else if line == "DONE" {
            finished = true;
        }
    }
    if record {
        r.metrics.insert("playtests_passed".into(), core_passed.to_string());
    }
    if !finished && r.verdict == Verdict::Accepted {
        r.reject(format!("playtests crashed:\n{}", tail(&format!("{}{}", o.stdout, o.stderr), 30, 4000)));
    }
    if !o.success && r.verdict == Verdict::Accepted {
        r.reject("playtest runner failed without saying why");
    }
    (r.verdict == Verdict::Accepted).then_some(tests)
}

fn valid_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
        && !RESERVED.contains(&n)
}

/// Test folders start with the goal they belong to: c12_, t3_, r1_, p4_. Keep in sync with game/build.rs.
fn valid_test_name(n: &str) -> bool {
    let b = n.as_bytes();
    let digits = b.iter().skip(1).take_while(|c| c.is_ascii_digit()).count();
    valid_name(n) && b"ctrp".contains(&b[0]) && (1..=3).contains(&digits) && b.get(1 + digits) == Some(&b'_')
}

/// `<root>/<name>/[<dir>/...]<file>.rs`, all parts valid names, at most MAX_DEPTH deep.
fn valid_source_path(p: &str, root: &str, top: fn(&str) -> bool) -> bool {
    let parts: Vec<&str> = p.split('/').collect();
    if parts.len() < 3 || parts.len() > MAX_DEPTH + 1 || parts[0] != root || !top(parts[1]) {
        return false;
    }
    let (file, dirs) = parts[2..].split_last().unwrap();
    dirs.iter().all(|d| valid_name(d)) && file.strip_suffix(".rs").is_some_and(|stem| valid_name(stem) || stem == "mod")
}

fn valid_module_path(p: &str) -> bool {
    valid_source_path(p, "modules", valid_name)
}

fn valid_test_path(p: &str) -> bool {
    valid_source_path(p, "tests", valid_test_name)
}

/// Returns whether the pull request adds tests, and which research entries it touches.
fn check_changed(r: &mut Report, list: &str) -> Option<(bool, Vec<String>)> {
    let lines: Vec<&str> = list.lines().map(str::trim).filter(|l| !l.is_empty()).collect();
    if lines.len() > MAX_CHANGED {
        r.reject(format!("{} files changed, at most {MAX_CHANGED} per pull request", lines.len()));
        return None;
    }
    let (mut modules, mut tests) = (false, false);
    let mut bench: Vec<String> = Vec::new();
    for line in lines {
        let (status, p) = line.split_once('\t').unwrap_or(("A", line));
        if valid_module_path(p) {
            modules = true;
        } else if valid_bench_path(p) {
            if status == "D" {
                r.reject(format!("{p}: research entries can be improved, not removed"));
            }
            let entry = p.split('/').nth(1).unwrap_or("").to_string();
            if !bench.contains(&entry) {
                bench.push(entry);
            }
        } else if valid_test_path(p) {
            tests = true;
            if status != "A" {
                r.reject(format!("{p}: existing tests cannot be changed or removed, only added"));
            }
        } else {
            r.reject(format!("touches {p:?}: pull requests may only change modules/<name>/**.rs, add tests/<goal>_<name>/**.rs or change bench/<goal>_<name>/**.rs"));
        }
    }
    if modules && tests {
        r.reject("a pull request either changes modules or adds tests, not both: tests come first, code second");
    }
    if !bench.is_empty() && (modules || tests) {
        r.reject("a research entry in bench/ goes in a pull request of its own, without modules or tests");
    }
    Some((tests, bench))
}

/// Research entries belong to a goal with a benchmark harness: r1_<name>. Keep in sync with game/build.rs.
fn valid_bench_name(n: &str) -> bool {
    valid_test_name(n) && BENCH_GOALS.iter().any(|g| n.strip_prefix(g).is_some_and(|rest| rest.starts_with('_')))
}

fn valid_bench_path(p: &str) -> bool {
    valid_source_path(p, "bench", valid_bench_name)
}

/// Runs the research benchmarks in a separate bench.wasm. `only` is the entries a pull request
/// touches (problems reject it); `None` means every entry, on main, where problems are only noted.
fn benchmarks(r: &mut Report, target: &Path, only: Option<&[String]>) {
    if only.is_some_and(|o| o.is_empty()) {
        return;
    }
    let strict = only.is_some();
    let problem = |r: &mut Report, key: String, why: String| {
        if strict { r.reject(why) } else { r.metrics.insert(key, why); }
    };
    r.stage = "bench";
    let dir = target.join("bench");
    let mut build = Command::new("cargo");
    build.env("CARGO_TARGET_DIR", &dir).env("CARGO_TERM_COLOR", "never");
    build.args(["build", "--release", "--locked", "--offline", "--target", "wasm32-unknown-unknown", "-p", "engine", "--features", "bench"]);
    match run_with_timeout(&mut build, BUILD_TIMEOUT) {
        Ok(o) if o.success && !o.timed_out => {}
        Ok(o) => return problem(r, "why bench".into(), format!("bench build failed:\n{}", tail(&o.stderr, 40, 4000))),
        Err(e) => return r.fault(format!("cannot run cargo: {e}")),
    }
    let wasm = dir.join("wasm32-unknown-unknown/release/engine.wasm");
    let Ok(bytes) = fs::read(&wasm) else { return r.fault("bench build produced no engine.wasm") };
    match wasm_interface(&bytes) {
        Err(e) => return r.fault(format!("bench.wasm does not parse: {e}")),
        Ok((imports, exports, max_pages)) => {
            if !imports.is_empty() || max_pages.is_none_or(|p| p > MAX_MEMORY_PAGES) {
                return r.reject(format!("bench.wasm must import nothing and cap its memory (imports {imports:?}, pages {max_pages:?})"));
            }
            if let Some(e) = exports.iter().find(|e| !ENGINE_EXPORTS.contains(&e.as_str()) && !BENCH_EXPORTS.contains(&e.as_str())) {
                return r.reject(format!("bench.wasm exports unexpected symbol {e:?}"));
            }
        }
    }
    let mut node = Command::new("node");
    node.arg("checker/bench.mjs").arg(&wasm).arg(only.map_or("*".to_string(), |o| o.join(","))).arg(MAX_BENCH_MS.to_string());
    let o = match run_with_timeout(&mut node, BENCH_TIMEOUT) {
        Err(e) => return r.fault(format!("cannot run node: {e}")),
        Ok(o) => o,
    };
    if o.timed_out {
        return problem(r, "why bench".into(), format!("benchmarks took longer than {BENCH_TIMEOUT} s (an endless loop?)"));
    }
    let mut finished = false;
    for line in o.stdout.lines() {
        if let Some(rest) = line.strip_prefix("BENCH ") {
            if let Some((name, score)) = rest.split_once(' ') {
                r.metrics.insert(format!("bench_{name}"), score.to_string());
            }
        } else if let Some(rest) = line.strip_prefix("FAIL ") {
            let name = rest.split(':').next().unwrap_or("").to_string();
            problem(r, format!("why {name}"), format!("benchmark {rest}"));
        } else if let Some(rest) = line.strip_prefix("METRIC bench_ms ") {
            r.metrics.insert("bench_ms".into(), rest.to_string());
        } else if line == "DONE" {
            finished = true;
        }
    }
    if !finished {
        problem(r, "why bench".into(), format!("benchmarks crashed:\n{}", tail(&format!("{}{}", o.stdout, o.stderr), 30, 4000)));
    }
}

fn scan_tree(r: &mut Report, root: &Path, label: &str) {
    let mut files = 0;
    let mut total = 0;
    let entries = match fs::read_dir(root) {
        Ok(e) => e,
        Err(_) if label == "tests" || label == "bench" => return, // none yet
        Err(e) => return r.fault(format!("cannot read {label}/: {e}")),
    };
    let top: fn(&str) -> bool = match label {
        "tests" => valid_test_name,
        "bench" => valid_bench_name,
        _ => valid_name,
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().into_owned();
        let Ok(meta) = fs::symlink_metadata(&path) else { continue };
        if meta.is_file() && name == "AGENTS.md" {
            continue; // core team docs
        }
        if !meta.is_dir() {
            r.reject(format!("{label}/{name} must be a folder"));
            continue;
        }
        if !top(&name) {
            r.reject(format!("{label}/{name}: not a valid folder name (see AGENTS.md)"));
            continue;
        }
        if !path.join("mod.rs").is_file() {
            r.reject(format!("{label}/{name}/mod.rs is missing"));
        }
        walk(r, &path, &format!("{label}/{name}"), 1, &mut files, &mut total);
    }
    if files > MAX_FILES {
        r.reject(format!("{files} module files, budget is {MAX_FILES}"));
    }
    if total > MAX_MODULE_BYTES {
        r.reject(format!("module sources are {total} bytes, budget is {MAX_MODULE_BYTES}"));
    }
    r.metrics.insert(format!("{label}_files"), files.to_string());
    r.metrics.insert(format!("{label}_bytes"), total.to_string());
}

fn walk(r: &mut Report, dir: &Path, shown: &str, depth: usize, files: &mut usize, total: &mut u64) {
    let Ok(entries) = fs::read_dir(dir) else { return r.fault(format!("cannot read {shown}")) };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let name = entry.file_name().to_string_lossy().into_owned();
        let shown = format!("{shown}/{name}");
        let Ok(meta) = fs::symlink_metadata(entry.path()) else { continue };
        if meta.file_type().is_symlink() {
            r.reject(format!("{shown} is a symlink"));
        } else if meta.is_dir() {
            if depth >= MAX_DEPTH - 1 || !valid_name(&name) {
                r.reject(format!("{shown}: folders are [a-z][a-z0-9_]* and at most {MAX_DEPTH} deep"));
            } else {
                walk(r, &entry.path(), &shown, depth + 1, files, total);
            }
        } else if !meta.is_file() || !(valid_module_path(&shown) || valid_test_path(&shown) || valid_bench_path(&shown)) {
            r.reject(format!("{shown}: only .rs files named [a-z][a-z0-9_]*.rs belong here"));
        } else {
            *files += 1;
            *total += meta.len();
            if meta.len() > MAX_FILE_BYTES {
                r.reject(format!("{shown} is {} bytes, budget is {MAX_FILE_BYTES}", meta.len()));
                continue;
            }
            match fs::read(entry.path()) {
                Ok(src) => scan_source(r, &shown, &src),
                Err(e) => r.fault(format!("cannot read {shown}: {e}")),
            }
        }
    }
}

/// The source rules. Deliberately blunt: it reads raw text, so a banned word
/// in a comment is rejected too. False alarms are cheap, misses are not.
fn scan_source(r: &mut Report, shown: &str, src: &[u8]) {
    let line_of = |at: usize| src[..at].iter().filter(|&&b| b == b'\n').count() + 1;
    if let Some(at) = src.iter().position(|&b| !(b == b'\n' || b == b'\t' || b == b'\r' || (0x20..0x7f).contains(&b))) {
        return r.reject(format!("{shown}:{}: only plain ASCII is allowed", line_of(at)));
    }
    if let Some(at) = src.iter().position(|&b| b == b'"') {
        return r.reject(format!("{shown}:{}: no string literals or double quotes in modules", line_of(at)));
    }
    let mut i = 0;
    while i < src.len() {
        let b = src[i];
        if b.is_ascii_alphanumeric() || b == b'_' {
            let start = i;
            while i < src.len() && (src[i].is_ascii_alphanumeric() || src[i] == b'_') {
                i += 1;
            }
            if b.is_ascii_digit() {
                continue; // number literal like 1e5f32
            }
            let word = std::str::from_utf8(&src[start..i]).unwrap();
            if let Some((_, why)) = BANNED.iter().find(|(w, _)| *w == word) {
                r.reject(format!("{shown}:{}: `{word}` is not allowed ({why})", line_of(start)));
            }
            let before = src[..start].iter().rev().find(|b| !b.is_ascii_whitespace());
            if (word == "const" || word == "mut") && before == Some(&b'*') {
                r.reject(format!("{shown}:{}: raw pointer types are not allowed", line_of(start)));
            }
            // `static` items are state outside the World: hidden from tests, replays and
            // determinism checks. The `'static` lifetime is fine.
            if word == "static" && (start == 0 || src[start - 1] != b'\'') {
                r.reject(format!("{shown}:{}: `static` is not allowed (keep all state in the World)", line_of(start)));
            }
        } else {
            i += 1;
        }
    }
}

// ---------------------------------------------------------------- wasm

fn leb(b: &[u8], at: &mut usize) -> Result<u32, String> {
    let mut v: u32 = 0;
    for shift in (0..35).step_by(7) {
        let byte = *b.get(*at).ok_or("truncated")?;
        *at += 1;
        v |= ((byte & 0x7f) as u32) << shift;
        if byte & 0x80 == 0 {
            return Ok(v);
        }
    }
    Err("bad LEB128".into())
}

fn name(b: &[u8], at: &mut usize) -> Result<String, String> {
    let len = leb(b, at)? as usize;
    let s = b.get(*at..*at + len).ok_or("truncated name")?;
    *at += len;
    Ok(String::from_utf8_lossy(s).into_owned())
}

/// (import names, export names, maximum memory pages) of a wasm binary.
fn wasm_interface(b: &[u8]) -> Result<(Vec<String>, Vec<String>, Option<u32>), String> {
    if b.get(0..8) != Some(b"\0asm\x01\0\0\0") {
        return Err("not a wasm 1.0 binary".into());
    }
    let (mut imports, mut exports, mut max_pages) = (Vec::new(), Vec::new(), None);
    let mut at = 8;
    while at < b.len() {
        let id = b[at];
        at += 1;
        let size = leb(b, &mut at)? as usize;
        let end = at.checked_add(size).filter(|&e| e <= b.len()).ok_or("section past end")?;
        let mut p = at;
        match id {
            2 => {
                // Any import at all is a rejection, so names are enough.
                let n = leb(b, &mut p)?;
                for _ in 0..n.min(1) {
                    imports.push(format!("{}.{}", name(b, &mut p)?, name(b, &mut p)?));
                }
                if n > 1 {
                    imports.push(format!("and {} more", n - 1));
                }
            }
            5 => {
                // One memory: flags (bit 0 = has maximum), minimum, maximum.
                if leb(b, &mut p)? != 1 {
                    return Err("expected exactly one memory".into());
                }
                let flags = *b.get(p).ok_or("truncated")?;
                p += 1;
                leb(b, &mut p)?;
                if flags & 1 == 1 && flags < 2 {
                    max_pages = Some(leb(b, &mut p)?);
                }
            }
            7 => {
                let n = leb(b, &mut p)?;
                for _ in 0..n {
                    exports.push(name(b, &mut p)?);
                    p += 1; // kind
                    leb(b, &mut p)?; // index
                }
            }
            _ => {}
        }
        at = end;
    }
    Ok((imports, exports, max_pages))
}

// ---------------------------------------------------------------- processes

struct Output {
    success: bool,
    timed_out: bool,
    stdout: String,
    stderr: String,
}

fn run_with_timeout(cmd: &mut Command, secs: u64) -> std::io::Result<Output> {
    let mut child = cmd.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped()).spawn()?;
    let (mut out, mut err) = (child.stdout.take().unwrap(), child.stderr.take().unwrap());
    // Read both pipes on threads so a chatty child can't block on a full pipe.
    let t_out = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = out.read_to_end(&mut s);
        s
    });
    let t_err = std::thread::spawn(move || {
        let mut s = Vec::new();
        let _ = err.read_to_end(&mut s);
        s
    });
    let deadline = Instant::now() + Duration::from_secs(secs);
    let (status, timed_out) = loop {
        if let Some(s) = child.try_wait()? {
            break (Some(s), false);
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            break (None, true);
        }
        std::thread::sleep(Duration::from_millis(50));
    };
    Ok(Output {
        success: status.is_some_and(|s| s.success()),
        timed_out,
        stdout: String::from_utf8_lossy(&t_out.join().unwrap_or_default()).into_owned(),
        stderr: String::from_utf8_lossy(&t_err.join().unwrap_or_default()).into_owned(),
    })
}

/// Last `lines` lines, at most `bytes` long, so a verdict never floods a comment.
fn tail(s: &str, lines: usize, bytes: usize) -> String {
    let all: Vec<&str> = s.lines().collect();
    let mut t = all[all.len().saturating_sub(lines)..].join("\n");
    if t.len() > bytes {
        let cut = t.len() - bytes;
        let cut = (cut..t.len()).find(|&i| t.is_char_boundary(i)).unwrap_or(t.len());
        t = format!("...{}", &t[cut..]);
    }
    t
}

fn json_str(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for c in s.chars() {
        match c {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            '\n' => o.push_str("\\n"),
            '\r' => o.push_str("\\r"),
            '\t' => o.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(o, "\\u{:04x}", c as u32);
            }
            c => o.push(c),
        }
    }
    o.push('"');
    o
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn module_paths() {
        assert!(valid_module_path("modules/car/mod.rs"));
        assert!(valid_module_path("modules/car/physics/wheel.rs"));
        assert!(!valid_module_path("modules/car/Mod.rs"));
        assert!(!valid_module_path("modules/car/notes.md"));
        assert!(!valid_module_path("modules/AGENTS.md"));
        assert!(!valid_module_path("api/src/lib.rs"));
        assert!(!valid_module_path("modules/../api/src/lib.rs"));
        assert!(!valid_module_path("modules/std/mod.rs"));
        assert!(!valid_module_path(".github/workflows/check.yml"));
        assert!(!valid_module_path("modules/a/b/c/d/e.rs"));
        assert!(!valid_module_path("modules/tests/mod.rs"));
        assert!(valid_test_path("tests/c9_camera/mod.rs"));
        assert!(valid_test_path("tests/r12_gi_score/scene.rs"));
        assert!(!valid_test_path("tests/camera/mod.rs"));
        assert!(!valid_test_path("tests/x9_camera/mod.rs"));
        assert!(!valid_test_path("tests/c1234_camera/mod.rs"));
        assert!(!valid_test_path("tests/c_camera/mod.rs"));
    }

    #[test]
    fn changed_rules() {
        let new = || Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        let mut r = new();
        assert_eq!(check_changed(&mut r, "A\ttests/c9_camera/mod.rs\n"), Some((true, vec![])));
        assert_eq!(r.verdict, Verdict::Accepted);
        let mut r = new();
        assert_eq!(check_changed(&mut r, "M\tmodules/car/mod.rs\nD\tmodules/old/mod.rs\n"), Some((false, vec![])));
        assert_eq!(r.verdict, Verdict::Accepted);
        let mut r = new();
        let entries = check_changed(&mut r, "A\tbench/r1_probes/mod.rs\nM\tbench/r1_probes/grid.rs\nM\tbench/r1_rays/mod.rs\n");
        assert_eq!(entries, Some((false, vec!["r1_probes".to_string(), "r1_rays".to_string()])));
        assert_eq!(r.verdict, Verdict::Accepted, "adding and improving research entries");
        let mut r = new();
        check_changed(&mut r, "D\tbench/r1_rays/mod.rs\n");
        assert_eq!(r.verdict, Verdict::Rejected, "removing a research entry");
        let mut r = new();
        check_changed(&mut r, "A\tbench/r1_rays/mod.rs\nM\tmodules/car/mod.rs\n");
        assert_eq!(r.verdict, Verdict::Rejected, "mixing a research entry and code");
        let mut r = new();
        check_changed(&mut r, "A\tbench/r2_crowd/mod.rs\n");
        assert_eq!(r.verdict, Verdict::Rejected, "a research goal without a harness");
        let mut r = new();
        check_changed(&mut r, "M\ttests/c9_camera/mod.rs\n");
        assert_eq!(r.verdict, Verdict::Rejected, "editing a test");
        let mut r = new();
        check_changed(&mut r, "A\ttests/c9_camera/mod.rs\nM\tmodules/car/mod.rs\n");
        assert_eq!(r.verdict, Verdict::Rejected, "mixing tests and code");
    }

    #[test]
    fn scanner_catches_banned_words() {
        let mut r = Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        scan_source(&mut r, "x.rs", b"let a = 1e5f32; let b = r#include_str;");
        assert_eq!(r.verdict, Verdict::Rejected);
        let mut r = Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        scan_source(&mut r, "x.rs", b"fn standard_env() {} // stdx envy\nlet a = &mut *b; let c = x * mut_y;");
        assert_eq!(r.verdict, Verdict::Accepted, "{:?}", r.reasons);
        let mut r = Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        scan_source(&mut r, "x.rs", b"let p = &x as * const u8;");
        assert_eq!(r.verdict, Verdict::Rejected);
        let mut r = Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        scan_source(&mut r, "x.rs", b"static N: u32 = 0;");
        assert_eq!(r.verdict, Verdict::Rejected, "static item");
        let mut r = Report { verdict: Verdict::Accepted, stage: "t", reasons: vec![], metrics: BTreeMap::new() };
        scan_source(&mut r, "x.rs", b"fn f() -> &'static [u8] { &[] }");
        assert_eq!(r.verdict, Verdict::Accepted, "static lifetime");
    }

    #[test]
    fn wasm_imports_are_seen() {
        // magic, version, import section: 1 import env.f (func, type 0)
        let w = b"\0asm\x01\0\0\0\x02\x09\x01\x03env\x01f\x00\x00";
        let (imports, exports, max) = wasm_interface(w).unwrap();
        assert_eq!(imports, vec!["env.f".to_string()]);
        assert!(exports.is_empty() && max.is_none());
        // memory section: one memory, has max, min 17, max 4096 (LEB 0x80 0x20)
        let m = b"\0asm\x01\0\0\0\x05\x05\x01\x01\x11\x80\x20";
        assert_eq!(wasm_interface(m).unwrap().2, Some(4096));
        assert!(wasm_interface(b"\0asm\x01\0\0\0\x02\x40").is_err());
    }

    #[test]
    fn json_escapes() {
        assert_eq!(json_str("a\"b\\c\nd\u{1}"), "\"a\\\"b\\\\c\\nd\\u0001\"");
    }
}
