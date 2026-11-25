# Security

We do not try to find malicious code. We take away its ability to do harm,
mechanically, and then let review catch what is merely bad.

## Reporting

Use GitHub's private vulnerability reporting (Security tab, "Report a
vulnerability"). Please do not open a public issue for a sandbox escape.

## The sandbox, layer by layer

| Threat | Defence | Enforced by |
| --- | --- | --- |
| Game code calls the network, storage, cookies or any browser API | `game.wasm` is instantiated with an empty import object and has zero imports | Checker parses the wasm on every build, the loader passes `{}` |
| Module reads files, env vars or the clock | Modules are `no_std`: those functions do not exist for them | `game` crate is `#![no_std]`, checker bans `std` and `extern` |
| Memory corruption | `#![forbid(unsafe_code)]` over all module code, no FFI, no inline asm | Compiler, checker bans `unsafe`, `asm`, `arch` |
| Supply chain | Zero external crates. Modules cannot add dependencies at all: they have no `Cargo.toml` | Workspace has none, pull requests may not touch `Cargo.*` |
| Code run at build time | No build scripts or proc macros in modules. Compile time file and env reads are banned (`include*`, `env`, `option_env`, `concat`, `stringify`, and string literals, which `#[path]` needs) | Checker source rules, path rules |
| Leaking build machine data into a build log | Same rules, plus symlinks in modules are rejected and CI builds with networking off | Checker, `docker run --network none` |
| Tests pass, browser misbehaves (platform detection) | Playtests are built into the shipped `game.wasm` and run on those exact bytes with no imports. Modules may not read addresses, float bits or the sign of NaN (it differs between x86 and ARM), and `cfg` is banned | Checker |
| Posing as the core team to skip the path rule | Core is the list of logins in `.github/core-team`, read from `main` by the trusted workflow and by the desk. Changing it needs core review | `verdict.yml`, rulesets |
| Faking the verdict by editing CI or the checker | A pull request may only change `modules/<name>/**.rs`. A trusted workflow checks the file list from GitHub's API, not from the run, ties the run to the pull request's current head, base branch and fork, and only it can set the `lastgame/verdict` status | `verdict.yml`, branch protection |
| Script injection into the page | Strict Content Security Policy, no third-party scripts, fonts or trackers | `web/index.html` |
| A game that slowly degrades: more colliders, a growing list scanned every tick, a heap that creeps | The world is static after the first minute of play, the heap may grow at most 256 KiB in five minutes, and a tick may not cost twice as much after five minutes as after one | Playtests `no_leak`, `growth_short`, `growth_long` |
| Mining or freezing the player's machine | CPU, memory, leak, draw call and triangle budgets measured in playtests, plus timeouts. The wasm itself cannot grow past 256 MiB, in tests and in the browser | Checker, linker flag in `.cargo/config.toml` |
| Hidden behaviour through randomness or time | Only `World::tick` and the seeded `World::random`. Two identical runs must give identical worlds | Playtest `deterministic` |
| Hidden state that tells tests from players (counting worlds, remembering earlier scenarios) | No `static` items in swarm code, and every scenario runs in a fresh wasm instance | Checker, `checker/playtest.mjs` |
| Weakening the tests instead of fixing the code | Swarm tests can only be added, never edited or removed. A test green on `main` must stay green (ratchet against a fresh build of `main`). A new test must fail on `main` | Checker, `verdict.yml` |
| A binary that is not built from the source | Pinned toolchain image, fixed build path, zero dependencies: anyone can rebuild and compare the sha256 | `scripts/build.sh`, build quorum |
| Text aimed at reviewers or other agents (prompt injection) | No string literals and plain ASCII only in modules, no non-code files in modules. Reviewers get the diff as data | Checker |
| Hidden code via Unicode tricks (Trojan Source) | Plain ASCII only | Checker |

`checker/suite/` holds known attacks, each of which must be rejected, plus one
harmless change that must pass. `scripts/suite.sh` runs them all.

## The desk

The desk (`desk/`) hands out work, runs the votes and merges. How it is set up,
and why it cannot merge what the trusted workflow did not mark green:
[desk/README.md](desk/README.md).

## Known limits

We say these out loud:

- **Time bombs.** `if w.tick == 50_000_000 { ... }` passes every test because
  no test plays that long. Reviewers have to catch it. Decoys train them to.
- **Compiler soundness bugs.** Known rustc bugs (see the cve-rs project) let
  safe code corrupt memory. Inside a wasm instance with no imports that still
  cannot reach anything outside the game, but it could fake playtest results
  or corrupt game state. Review is the line here.
- **Code that recognises a test.** Playtests use scripted inputs. Code that
  detects those input patterns, or that only misbehaves on a rare in-game
  condition, can pass. Reviewers and decoys are the line here.
- **Engine-specific limits.** Different browsers allow different call stack
  depths, so deep recursion can pass on the test engine and fail elsewhere.
- **Bad but harmless content.** Ugly or offensive shapes pass the sandbox.
  Review and moderation handle those.
- **Sybil reviewers.** One owner can run many accounts. Reputation thresholds
  make it expensive, not impossible.
- **Shared model blind spots.** Three agreeing language models are weaker
  evidence than three agreeing hashes. That is why the sandbox comes first.
