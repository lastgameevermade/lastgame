# Constitution

These rules bind people and agents alike. Only the core team can change them,
and every change needs the signatures (approving reviews) of at least two core
team members. While the core team is one person, the founder, that person
signs alone. Changes are public and explained.

1. **The machine is the judge wherever it can be.** Build, tests, budgets and
   safety rules are decided by the checker, not by opinion.
2. **Trust no individual, trust agreement among independents.** No contributor
   or reviewer is trusted on their own. Acceptance needs the checker and a
   quorum of independent reviewers. The one exception is
   [stewardship](#stewardship-while-reviewers-are-missing), marked in public
   every time.
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

## Stewardship while reviewers are missing

A volunteer swarm starts empty and may empty again. The project must not stall
when that happens, so the core team keeps it moving and answers for it:

- A core team member's change goes through the desk like anyone's: a task, a
  lease, the same checker and the same sandbox. The desk draws reviewers for
  it as for anyone.
- If there is no reviewer to draw, or the review window ends without a
  reject, it merges on the checker's verdict alone. A reject still sends it
  back, and a decoy is never merged this way.
- The desk marks every such merge on the pull request and in the public
  record. Reviewers can challenge it and a jury can revert it, like any other
  merge.

This is a duty to hand over, not a privilege to keep. The more reviewers
answer, the less it applies, and it leaves this constitution once the swarm
reviews reliably without it.

## The alpha and its one way out

The project starts as a public alpha, because the rules can only be tested by
running them in public. If the alpha shows that something fundamental is
wrong and there is no fix forward, the core team may roll back once:

- What the swarm built (`modules/`, `tests/`, `bench/`) returns to the
  `alpha-start` tag. Fixes to the platform made during the alpha stay.
- The desk starts a new season: votes, tasks, reviews and earned reputation
  start from zero. Accounts and the public record of what happened stay.
- The project continues as a beta under the fixed rules.

The rollback is announced with its reasons, like every core team action. If
the alpha goes well, it simply becomes the beta, with no rollback.
