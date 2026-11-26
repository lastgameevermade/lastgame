// Generates the module and test lists from the folders in modules/ and tests/.
// Trusted code: swarm code never gets a build script of its own.
use std::{env, fs, path::Path};

const RESERVED: &[&str] = &[
    "alloc", "api", "bench", "benches", "core", "crate", "game", "harness", "self", "std", "super", "test", "tests",
    // Rust keywords that are valid identifiers otherwise
    "as", "async", "await", "break", "const", "continue", "dyn", "else", "enum", "extern", "false", "fn", "for",
    "gen", "if", "impl", "in", "let", "loop", "match", "mod", "move", "mut", "pub", "ref", "return", "static",
    "struct", "trait", "true", "type", "unsafe", "use", "where", "while", "abstract", "become", "box", "do",
    "final", "macro", "override", "priv", "try", "typeof", "unsized", "virtual", "yield",
];

fn valid_name(n: &str) -> bool {
    let b = n.as_bytes();
    !b.is_empty()
        && b.len() <= 32
        && b[0].is_ascii_lowercase()
        && b.iter().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == b'_')
        && !RESERVED.contains(&n)
}

/// Test folders start with the goal they belong to: c12_, t3_, r1_, p4_.
fn valid_test_name(n: &str) -> bool {
    let b = n.as_bytes();
    let digits = b.iter().skip(1).take_while(|c| c.is_ascii_digit()).count();
    valid_name(n) && b"ctrp".contains(&b[0]) && (1..=3).contains(&digits) && b.get(1 + digits) == Some(&b'_')
}

/// Research entries belong to a goal with a benchmark harness: r1_<name>. Keep in sync with the checker.
fn valid_bench_name(n: &str) -> bool {
    valid_test_name(n) && n.starts_with("r1_")
}

/// Sorted folder names in `dir`, each checked and required to have a mod.rs.
fn folders(dir: &Path, what: &str, ok: fn(&str) -> bool) -> Vec<String> {
    println!("cargo:rerun-if-changed={}", dir.display());
    let mut names = Vec::new();
    let Ok(entries) = fs::read_dir(dir) else { return names };
    for entry in entries {
        let entry = entry.unwrap();
        let ty = entry.file_type().unwrap();
        let name = entry.file_name().into_string().expect("non UTF-8 name");
        if ty.is_file() {
            continue; // docs like AGENTS.md
        }
        assert!(ty.is_dir(), "{what}/{name} is not a plain folder");
        assert!(ok(&name), "{what}/{name}: not a valid folder name");
        assert!(entry.path().join("mod.rs").is_file(), "{what}/{name}/mod.rs missing");
        names.push(name);
    }
    names.sort();
    names
}

fn main() {
    let root = Path::new(&env::var("CARGO_MANIFEST_DIR").unwrap()).join("..").canonicalize().unwrap();
    let modules = folders(&root.join("modules"), "modules", valid_name);
    let tests = folders(&root.join("tests"), "tests", valid_test_name);
    let benches = folders(&root.join("bench"), "bench", valid_bench_name);
    let file = |dir: &str, n: &str| format!("{:?}", root.join(dir).join(n).join("mod.rs").display().to_string());

    let mut out = String::new();
    for n in &modules {
        out += &format!("#[path = {}]\npub mod {n};\n", file("modules", n));
    }
    out += "\n/// Every module as (ORDER, name, init, update), sorted by ORDER then name.\n";
    out += "pub fn modules() -> alloc::vec::Vec<(i32, &'static str, fn(&mut api::World), fn(&mut api::World))> {\n";
    out += "    let mut v = alloc::vec::Vec::new();\n";
    for n in &modules {
        out += &format!("    v.push(({n}::ORDER, {n:?}, {n}::init as fn(&mut api::World), {n}::update as fn(&mut api::World)));\n");
    }
    out += "    v.sort_by(|a, b| (a.0, a.1).cmp(&(b.0, b.1)));\n    v\n}\n";

    out += "\n/// Acceptance tests written by the swarm, one per folder in tests/.\npub mod tests {\n";
    for n in &tests {
        out += &format!("    #[path = {}]\n    pub mod {n};\n", file("tests", n));
    }
    out += "}\n\n/// Every swarm test as (name, run), sorted by name.\n";
    out += "pub fn tests() -> &'static [(&'static str, fn() -> harness::Outcome)] {\n    &[\n";
    for n in &tests {
        out += &format!("        ({n:?}, tests::{n}::run),\n");
    }
    out += "    ]\n}\n";

    out += "\n/// Research benchmark entries, one per folder in bench/.\npub mod bench {\n";
    for n in &benches {
        out += &format!("    #[path = {}]\n    pub mod {n};\n", file("bench", n));
    }
    out += "}\n\n/// Every research entry as (name, render), sorted by name.\n";
    out += "pub fn benches() -> &'static [(&'static str, fn(&api::bench::Scene, &mut api::bench::Image))] {\n    &[\n";
    for n in &benches {
        out += &format!("        ({n:?}, bench::{n}::render),\n");
    }
    out += "    ]\n}\n";
    fs::write(Path::new(&env::var("OUT_DIR").unwrap()).join("modules.rs"), out).unwrap();
}
