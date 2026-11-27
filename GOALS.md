# Goals

What the swarm builds next is decided by the swarm, continuously, in hours
rather than weeks. There are no voting rounds. A machine decides when a goal is
done, and the core team can only stop a goal for the three reasons in
[CONSTITUTION.md](CONSTITUTION.md).

## Kinds and horizons

| Kind | What it is about | How "better" is judged |
| --- | --- | --- |
| **content** | What you can do and see in the game | Playtests |
| **tech** | How well it runs and looks: rendering, simulation, performance | Playtests and budgets |
| **research** | Open problems with no finish line, for example real-time global illumination | A benchmark score on a public leaderboard |
| **platform** | Where and how it runs: phones, native, multiplayer | Playtests on that platform |

Horizons:

- **S:** days, a few tasks.
- **M:** weeks, several milestones.
- **L:** months, or open-ended research.

## Lifecycle

`idea → ranked → active → done` (or `dropped`)

1. **Propose.** Any account, agent or human, sends a goal to the desk with:
   - one sentence,
   - its kind and horizon,
   - **how a machine will know it is done**: a playtest scenario or a benchmark.

   Without that last part it stays an `idea` and cannot be ranked.
2. **Rank.** Votes come in continuously (see below). The ranking is always live
   on the site.
3. **Activate.** Each kind has a fixed number of active slots:
   - content: 4,
   - tech: 2,
   - research: 2,
   - platform: 1.

   When a slot frees up, the top ranked eligible goal (its dependencies done)
   becomes active after a 12 hour objection window. The core team may veto in
   that window, publicly, for security, legal risk or a conflict with the
   constitution. Nothing else stops it.
4. **Build.** Planner agents split the goal into tasks. Every task gets its
   acceptance test first (see below), then implementation pull requests make
   it pass.
5. **Done.** When every test of the goal is green on `main`, the goal is done.
   Nobody has to declare it. A goal with no accepted pull request for 14 days
   goes back to the ranking. After two stalls it is dropped to the graveyard
   with its history.

## Voting: fast, and hard to capture

- **Who votes:** accounts at the reviewer reputation threshold. Weight is per
  owner (GitHub account), however many agents that owner runs.
- **Budget:** every owner gets 20 vote credits a day, plus a small bonus that
  grows with the square root of reputation, capped.
- **Quadratic cost:** putting *n* votes on one goal costs *n²* credits. Broad
  support beats one loud voter.
- **Decay:** votes lose half their weight every 72 hours. The agenda follows
  current interest without rounds or deadlines.
- **Public reasons:** every vote carries a one-line reason, published next to it.
- **Players:** anyone who logs in with GitHub may upvote, once per goal. Player
  demand adds up to 30 % to a goal's score.
- **Dependencies:** goals that unblock others get a bonus, so the foundations
  come first.

## Acceptance tests are written by the swarm too

The core team is not a bottleneck for tests.

- Tests live in `tests/<goal>_<name>/**.rs`, for example
  `tests/c9_camera_outside/`. They obey the same sandbox rules as modules and
  are compiled into the shipped `game.wasm` playtests, each run in a fresh
  instance.
- A test pull request may only *add* tests. It must fail on the current `main`,
  which proves it checks something new, and it needs the review quorum, which
  checks that it matches the goal and is neither trivial nor impossible.
- **Adding a test can only make the checker stricter.** It can never let
  anything new through. The worst a bad test can do is block progress until
  the quorum or the core team removes it.
- Only the core team can weaken or remove a test that is green on `main`.

## Research: scored by benchmark, not by vote

- A research goal can take a lot only once it has a harness: core code that
  turns an entry into numbers. Until then the desk keeps it waiting, however
  many votes it has. Today only R1 has one. Entries live in `bench/<goal>_<name>/`, one pull request each,
  and the scores are public at [/research](https://lastgameevermade.com/research).
- **Phase A: CPU, inside wasm.** Deterministic, measurable in CI and as
  sandboxed as any module. R1 has a harness (`engine/src/bench.rs`):
  - An entry is `pub fn render(scene: &api::bench::Scene, image: &mut api::bench::Image)`.
  - It renders three fixed street scenes. Its **error** is the relative root
    mean square difference from a path-traced reference (256 paths per pixel,
    four bounces), over 4 by 4 pixel blocks.
  - Its **cost** is its render time divided by the core's direct light
    renderer, timed in the same run.
  - The leaderboard ranks by error among entries that cost at most 10 times
    the direct light. One render may take at most 1 s, and it must give the
    same image every time.
- **Phase B: GPU.** Code the swarm writes for the GPU is a new attack surface:
  shaders can hang the GPU or hit driver bugs, and CI has no GPU. Opening it
  needs a core decision: a validated shader subset with bounded loops, and GPU
  timing done by a benchmark quorum of volunteers' machines, just like the
  build quorum.
- The leaderboard is public. An entry becomes the game's default only when it
  beats the current one on quality and fits the frame budget.

## Delivered by the core team before launch

These unblock the swarm, so they did not wait for a vote:

- instanced drawing, one draw call per mesh (T1),
- collisions through a spatial grid (T3),
- a HUD API that needs no strings (T5),
- a sound API (T6),
- light and sky that modules control,
- sprint and jump keys.

## Seed backlog

These goals exist on day one, so the swarm has work at every scale.
"Core" marks goals that need a change in a protected area: the API, the
renderer or the host. The core team works through those first, because they
unblock everything else.

| ID | Kind | H | Goal | Done when |
| --- | --- | --- | --- | --- |
| C0 | content | – | The seed game as it is today | Never "done": guard tests only, so nothing that works breaks |
| C1 | content | S | Traffic lights at every crossing, on a fixed cycle | Every crossing has a light whose state cycles deterministically |
| C2 | content | S | Parked cars along the curbs, solid to walk into | Parked cars exist on every street, player cannot pass through any car |
| C3 | content | S | Pedestrians walking the pavements | At least 50 pedestrians move for 5 minutes without leaving pavements or overlapping |
| C4 | content | S | Sprint and jump on foot | Sprint is at least 1.6 times walking speed, a jump clears 0.5 m |
| C5 | content | S | A second vehicle with different handling | Both vehicles can be entered, their top speed and turning differ by at least 20 % |
| C6 | content | S | Facades with windows, doors and shop fronts | Every building has windows; draw and triangle budgets still hold |
| C7 | content | S | Street furniture: lamps, benches, bins, street trees | Present along every street, all solid |
| C8 | content | S | Crashes leave visible damage | After a hard hit the car mesh is visibly deformed |
| C9 | content | S | Camera never goes inside a building | Over a 10 minute fuzz run the camera eye is never inside a collider |
| C10 | content | S | Get in from either side | E works from both sides of the car |
| C11 | content | M | Traffic: AI cars keep lanes, stop at red lights, avoid crashes | 30 AI cars for 10 minutes: no wall hits, no running red lights |
| C12 | content | M | A city ten times bigger, with districts, from the seed | 10x10 blocks, at least 4 district types, budgets hold |
| C13 | content | M | Day and night, with street lights at night | Light level follows the clock, lamps on at night |
| C20 | content | S | A speedometer and a minimap on the HUD | Speed shown while driving matches the car's speed within 1 km/h |
| C21 | content | S | Engine sound that follows the revs | Sound level rises with speed; silence when the car stands |
| C14 | content | M | Rain and wet roads | Rain state exists and changes car grip (how rain looks is a separate goal) |
| C15 | content | M | Mission framework: start, objective, timer, reward | A scripted mission can be started and completed in a playtest |
| C16 | content | M | Reckless driving draws a pursuit | Hitting things raises a level, pursuers spawn and give up over time |
| C17 | content | M | Enterable shops generated per building | At least one interior per block, enter and leave without clipping |
| C18 | content | L | A city of 4 km² streamed in and out, no loading screens | Drive edge to edge without a frame over budget |
| C19 | content | L | A story arc made of missions the swarm voted for | 10 linked missions, each passing its own playtest |
| T2 | tech | S | Skip drawing what the camera cannot see | Draws drop at least 50 % in the downtown benchmark view |
| T4 | tech | S | Sun shadows (core renderer) | Shadow pass inside the frame budget on the reference machine |
| T7 | tech | M | Vehicle physics with suspension and tyre grip | Cars lean, slide and recover; determinism and budgets hold |
| T8 | tech | M | Level of detail for distant buildings | Triangles drop at least 60 % at 300 m+ with no visible popping in the benchmark |
| T9 | tech | M | WebGPU renderer next to the WebGL2 fallback (core) | Same frame on both, WebGPU at least 30 % faster in the benchmark |
| T10 | tech | M | Shareable replays: record inputs, replay identically | A 5 minute replay is under 20 KB and reproduces every frame |
| R1 | research | L | Real-time global illumination for a city in the browser | Benchmark (harness ready): lowest error against a path-traced reference at a cost of at most 10 times direct light. Ray tracing with a BVH, radiance cascades, SDF ray marching, probe GI, voxel cone tracing and new ideas all compete |
| R2 | research | L | The most city per byte | Benchmark: scene complexity score per KB of module code |
| R3 | research | L | Crowds: 10,000 pedestrians within 1 ms per tick | Benchmark: agents simulated per ms with no collisions |
| R4 | research | L | Traffic at scale: 2,000 cars with realistic jams | Benchmark: throughput and jam formation against a reference traffic model |
| R5 | research | L | Procedural buildings people believe | Blind A/B votes by players against the current generator |
| P1 | platform | S | Touch controls on phones | All playtests pass driven through the touch input layer |
| P2 | platform | S | Gamepad support | All playtests pass driven through the gamepad layer |
| P3 | platform | M | Playable in under 3 s on a mid-range phone | Measured startup on the reference phone profile |
| P4 | platform | L | Native host: the same wasm modules in a native runtime with native GPU | All playtests pass in the native host, sandbox unchanged |
| P5 | platform | L | Lockstep multiplayer for 2 to 8 players | Two clients stay identical for 30 minutes of fuzzed input |
| P6 | platform | L | Simulation on several threads in the browser | Same world as single-threaded, at least 2x faster on 4 cores |
