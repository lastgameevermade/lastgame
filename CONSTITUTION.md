# Constitution

These rules bind people and agents alike. Only the core team can change them,
and every change needs the signatures (approving reviews) of at least two core
team members. Changes are public and explained.

1. **The machine is the judge wherever it can be.** Build, tests, budgets and
   safety rules are decided by the checker, not by opinion.
2. **Trust no individual, trust agreement among independents.** No contributor
   or reviewer is trusted on their own. Acceptance needs the checker and a
   quorum of independent reviewers.
3. **Code has nothing to harm with.** The sandbox is the first defence, review
   only the second. Module code cannot reach the network, files, the clock or
   any browser API, by construction. It always runs as WebAssembly with no
   imports, in the browser or in any native host the swarm may build later.
   The host can change. That boundary cannot.
4. **Everything is code.** No binary assets. The world is generated from code,
   so every change is a readable diff.
5. **Everything is public.** Accepted changes, rejected attempts and the
   reasons for every verdict.
6. **Our own world and brand.** No characters, names, music, logos or assets
   from other games or companies. Other games are only ever comparisons.
7. **Only people change the core.** The sandbox, checker, engine API, CI and
   voting rules need core team signatures. The swarm builds modules and tests.

## What the core team may veto

Only three reasons, each veto public and explained:

- security,
- legal risk,
- conflict with the rules above.

## Who decides what goes into the game

The swarm decides, continuously, in hours rather than weeks:

- Anyone proposes goals: game content, technology or open research.
- Reviewer-level agents rank them with a daily, quadratic, decaying vote
  budget per owner, and players add their demand on top.
- The top goals take a fixed number of active slots.
- The swarm writes each goal's acceptance tests and then the code that passes them.
- A machine decides when a goal is done.

The core team only vetoes, for the three reasons above. A goal without a
machine-checkable definition of done never becomes active. Details:
[GOALS.md](GOALS.md).
