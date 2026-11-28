// End-to-end tests for the desk against a real SQLite database and a fake
// GitHub. Run: node desk/test.mjs   (Node 22.5+, uses node:sqlite)
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import desk, { RULES, agenda, cost, credits, decay, doneGoals, pickReviewers, rep, scheduledTick, score, tick, upholdNeed, DAY, HOUR } from './src/worker.js';
import { hmac, pkcs8, sha256 } from './src/github.js';
import { generateKeyPairSync } from 'node:crypto';

// ---------------------------------------------------------------- pure rules

const acct = (o = {}) => ({ login: 'x', accepted: 0, caught: 0, missed: 0, overturned: 0, bonus: 0, banned: 0, created_gh: 0, last_seen: 0, ...o });
assert.equal(rep(acct({ accepted: 4, caught: 2, missed: 1, overturned: 1 })), 4 + 2 - 3 - 2);
assert.equal(credits(acct()), 20);
assert.equal(credits(acct({ bonus: 100 })), 30, 'bonus capped');
assert.equal(cost(0, 3), 9);
assert.equal(cost(3, 4), 7, 'quadratic is cumulative within a day');
assert.ok(Math.abs(decay(72 * HOUR) - 0.5) < 1e-9);
assert.ok(score([{ n: 2, day: 0 }], 0, 0, 0) === 2);
assert.ok(score([{ n: 2, day: 0 }], 1000, 0, 0) < 2 * 1.3 + 1e-9, 'players add at most 30 %');
{
  const g = (id, kind, s, status = 'ranked', depends = '') => ({ id, kind, status, score: s, depends, created: 0, stalls: 0 });
  const plan = agenda([g('c1', 'content', 5), g('c2', 'content', 9), g('c3', 'content', 1, 'ranked', 'c9'), g('c9', 'content', 0),
    g('p1', 'platform', 3), g('p2', 'platform', 4)], 0);
  assert.deepEqual(plan.pend.sort(), ['c1', 'c2', 'p2'], 'top goals per kind, dependencies respected, zero score never');
  const stalled = agenda([{ ...g('t1', 'tech', 1, 'active'), activated: 0, last_merge: 0 }], 15 * DAY);
  assert.deepEqual(stalled.stall, ['t1']);
}
assert.deepEqual(doneGoals([{ id: 'c1', status: 'active' }, { id: 'c2', status: 'active' }, { id: 'c3', status: 'active' }],
  { green: ['c1_a', 'c2_a', 'c12_x'], red: ['c2_b'] }), ['c1'], 'c12_ is not c1_, a red test blocks');
assert.equal(pickReviewers([acct({ login: 'a', bonus: 9 })], 'z', 3, 200 * DAY), null, 'not enough reviewers');
assert.deepEqual([3, 4, 5].map(upholdNeed), [3, 4, 4], '4 of 5, the same share rounded up for a smaller jury');

// GitHub's PKCS#1 key works as is.
{
  const { privateKey: pk1 } = generateKeyPairSync('rsa', { modulusLength: 2048, privateKeyEncoding: { type: 'pkcs1', format: 'pem' }, publicKeyEncoding: { type: 'spki', format: 'pem' } });
  assert.ok(pk1.includes('BEGIN RSA PRIVATE KEY'));
  const key = await crypto.subtle.importKey('pkcs8', pkcs8(pk1), { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, false, ['sign']);
  assert.ok((await crypto.subtle.sign('RSASSA-PKCS1-v1_5', key, new Uint8Array([1, 2, 3]))).byteLength === 256);
}

// ---------------------------------------------------------------- a desk with a fake GitHub

const db = new DatabaseSync(':memory:');
db.exec(readFileSync(new URL('./schema.sql', import.meta.url), 'utf8'));
const d1 = {
  prepare(sql) {
    const st = db.prepare(sql);
    return {
      bind: (...a) => ({
        first: async () => st.get(...a) ?? null,
        all: async () => ({ results: st.all(...a) }),
        run: async () => {
          const r = st.run(...a);
          return { meta: { changes: Number(r.changes), last_row_id: Number(r.lastInsertRowid) } };
        },
      }),
    };
  },
};

let clock = 400 * DAY;
let mainStatus = null;
let seedGoalsFile = { goals: [{ id: 'c77', kind: 'content', horizon: 'S', title: 'Seeded goal', done_when: 'it was seeded from goals.json' }] };
const calls = [];
let mainSha = 'a'.repeat(40);
let graphqlReply = { data: { revertPullRequest: { revertPullRequest: { number: 99, headRefOid: 'rv1' } } } };
const ok = () => `ACCEPTED at stage ratchet on main ${mainSha.slice(0, 12)} swarm`;
const { privateKey } = await crypto.subtle.generateKey(
  { name: 'RSASSA-PKCS1-v1_5', modulusLength: 2048, publicExponent: new Uint8Array([1, 0, 1]), hash: 'SHA-256' }, true, ['sign', 'verify']);
const pem = `-----BEGIN PRIVATE KEY-----\n${Buffer.from(await crypto.subtle.exportKey('pkcs8', privateKey)).toString('base64')}\n-----END PRIVATE KEY-----`;
const env = {
  DB: d1, NOW: () => clock, REPO: 'o/lastgame', SITE: 'https://site', WEBHOOK_SECRET: 'whs', STATE_SECRET: 'sts', ADMIN_KEY: 'adm',
  GITHUB_APP_ID: '1', GITHUB_APP_KEY: pem, GITHUB_INSTALLATION_ID: '2', GITHUB_CLIENT_ID: 'cid', GITHUB_CLIENT_SECRET: 'cs',
  ASSETS: { fetch: async (r) => {
    const p = new URL(r.url).pathname;
    if (p === '/status.json' && mainStatus) return Response.json(mainStatus);
    if (p === '/goals.json' && seedGoalsFile) return Response.json(seedGoalsFile);
    return new Response('nope', { status: 404 });
  } },
};
globalThis.fetch = async (url, opts = {}) => {
  calls.push({ url: String(url), method: opts.method ?? 'GET', body: opts.body ? JSON.parse(opts.body) : null });
  if (String(url).endsWith('/access_tokens')) return Response.json({ token: 't', expires_at: new Date(Date.now() + 3600e3).toISOString() });
  if (String(url).endsWith('/contents/.github/core-team')) return Response.json({ content: btoa('# core\nBoss\n') });
  if (String(url).endsWith('/git/ref/heads/main')) return Response.json({ object: { sha: mainSha } });
  if (String(url).includes('/actions/runs?')) return Response.json({ workflow_runs: [{ id: 77, path: '.github/workflows/check.yml', status: 'completed' }] });
  if (String(url).endsWith('/graphql')) return Response.json(graphqlReply);
  const pull = /\/pulls\/(\d+)$/.exec(String(url));
  if (pull && (opts.method ?? 'GET') === 'GET') return Response.json({ node_id: `PR_${pull[1]}` });
  return Response.json({ ok: true });
};

const keys = {};
async function person(login, o = {}) {
  keys[login] = (await sha256(login)).slice(0, 64);
  db.prepare('INSERT INTO accounts (login, github_id, created_gh, key_hash, joined, bonus, core) VALUES (?, ?, ?, ?, ?, ?, ?)')
    .run(login, Math.floor(Math.random() * 1e9), o.young ? clock - DAY : 0, await sha256(keys[login]), clock, o.bonus ?? 0, o.core ?? 0);
}
const call = async (method, path, who, payload, headers = {}) => {
  const h = { ...headers };
  if (who === 'core') h.authorization = 'Bearer adm';
  else if (who) h.authorization = `Bearer ${keys[who]}`;
  const r = await desk.fetch(new Request(`https://site${path}`, { method, headers: h, body: payload ? JSON.stringify(payload) : undefined }), env);
  return { status: r.status, data: await r.json() };
};
const hook = async (kind, payload, secret = env.WEBHOOK_SECRET) => {
  const raw = JSON.stringify({ repository: { full_name: env.REPO }, ...payload });
  const r = await desk.fetch(new Request('https://site/hooks/github', {
    method: 'POST', body: raw, headers: { 'x-github-event': kind, 'x-hub-signature-256': `sha256=${await hmac(secret, raw)}` },
  }), env);
  return r.status;
};
const pr = (number, login, head, body = '', extra = {}) => ({
  number, title: `pr ${number}`, body, head: { sha: head }, user: { login }, author_association: 'NONE', merged: false, ...extra,
});
const row = (sql, ...a) => db.prepare(sql).get(...a);
const ghCalls = (pattern) => calls.filter((c) => c.url.includes(pattern));

await person('alice');
await person('young', { young: true, bonus: 10 });
for (const r of ['r1', 'r2', 'r3', 'r4', 'r5', 'r6', 'r7']) await person(r, { bonus: 3 });
await person('decoybot');
await person('planner', { bonus: 10 }); // stays eligible whatever the random decoy draw does
await person('boss', { core: 1 });

// An empty database seeds itself from goals.json on the first tick, and the tick reports its health.
await scheduledTick(env);
assert.equal((await call('GET', '/api/summary')).data.last_tick, clock);
assert.equal((await call('GET', '/api/summary')).data.tick_error, null);
assert.equal((await call('GET', '/api/summary')).data.github_error, null, 'the App reaches GitHub');
assert.equal(row("SELECT title FROM goals WHERE id = 'c77'").title, 'Seeded goal');
db.prepare("DELETE FROM goals").run();
db.prepare("DELETE FROM events").run();
seedGoalsFile = null;

// Seed goals, vote, activate.
assert.equal((await call('POST', '/admin/goals', 'core', { goals: [
  { id: 'c1', kind: 'content', horizon: 'S', title: 'Traffic lights', done_when: 'every crossing has a light' },
  { id: 'c2', kind: 'content', horizon: 'S', title: 'Parked cars', done_when: 'parked cars on every street' },
] })).status, 200);
assert.equal((await call('POST', '/admin/goals', 'alice', { goals: [] })).status, 401, 'admin needs the core key');
assert.equal((await call('POST', '/desk/goals/c1/vote', 'alice', { votes: 1, reason: 'want it' })).status, 403, 'no reputation, no vote');
assert.equal((await call('POST', '/desk/goals/c1/vote', 'young', { votes: 1, reason: 'want it' })).status, 403, 'young GitHub account, no vote');
assert.equal((await call('POST', '/desk/goals/c1/vote', 'r1', { votes: 3, reason: 'core city feature' })).status, 200);
const tooMuch = await call('POST', '/desk/goals/c1/vote', 'r1', { votes: 5, reason: 'more' });
assert.equal(tooMuch.status, 400, 'quadratic cost: 8 votes cost 64 > 21 credits');
assert.equal((await call('POST', '/desk/goals', 'alice', { kind: 'content', horizon: 'S', title: 'Benches', done_when: 'x' })).status, 400, 'done_when required');
const proposed = await call('POST', '/desk/goals', 'alice', { kind: 'content', horizon: 'S', title: 'Benches', done_when: 'every pavement has benches' });
assert.equal(proposed.data.id, 'c3');

db.prepare("INSERT INTO goals (id, kind, horizon, title, done_when, status, proposer, created) VALUES ('r7', 'research', 'L', 'No harness yet', 'a benchmark', 'ranked', 'core', ?)").run(clock);
assert.equal((await call('POST', '/desk/goals/r7/vote', 'r1', { votes: 2, reason: 'interesting' })).status, 200);
await tick(env);
assert.equal(row("SELECT status FROM goals WHERE id = 'r7'").status, 'ranked', 'a research goal without a harness never takes a lot');
assert.equal(row("SELECT status FROM goals WHERE id = 'c1'").status, 'pending', 'top goal enters the objection window');
assert.equal(row("SELECT status FROM goals WHERE id = 'c2'").status, 'ranked', 'no votes, no activation');
clock += 13 * HOUR;
await tick(env);
assert.equal(row("SELECT status FROM goals WHERE id = 'c1'").status, 'active');

// Tests first: a code task cannot be leased before a test is merged.
assert.equal((await call('POST', '/desk/tasks', 'r1', { goal: 'c1', kind: 'code', title: 'Lights module', detail: 'cycle every 20 s' })).status, 201);
assert.equal((await call('GET', '/desk/next?kind=code', 'alice')).data.lease, null, 'code waits for its tests');
assert.equal((await call('POST', '/desk/tasks', 'r1', { goal: 'c1', kind: 'test', title: 'Test the lights', detail: 'every crossing cycles' })).status, 201);
const leased = await call('GET', '/desk/next', 'alice', null, { 'x-agent': 'alice-bot' });
assert.equal(leased.data.task.kind, 'test');
const leaseId = leased.data.lease;
assert.equal((await call('POST', `/desk/beat?lease=${leaseId}`, 'alice')).status, 200);
assert.equal((await call('POST', `/desk/beat?lease=${leaseId}`, 'r1')).status, 410, 'someone else cannot touch the lease');

// The pull request, the checker, the quorum, the merge.
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(10, 'Alice', 'h1', `Adds the test.\n\nLease: ${leaseId}`) }, 'wrong'), 401, 'forged webhook');
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(10, 'Alice', 'h1', `Adds the test.\n\nLease: ${leaseId}`) }), 200);
assert.equal(await hook('status', { context: 'lastgame/verdict', sha: 'h1', state: 'success', description: ok() }), 200);
const reviewers = db.prepare('SELECT reviewer FROM reviews WHERE pr = 10').all().map((r) => r.reviewer);
assert.equal(reviewers.length, 3);
assert.ok(!reviewers.includes('alice') && !reviewers.includes('young'), 'never the author, never ineligible');
for (const r of reviewers) {
  const job = await call('GET', '/desk/review/next', r);
  assert.equal(job.data.pull_request, 10);
  assert.equal((await call('POST', '/desk/review', r, { review: job.data.review, verdict: 'accept', reason: 'matches the goal' })).status, 200);
}
const mergeCall = ghCalls('/pulls/10/merge')[0];
assert.ok(mergeCall && mergeCall.method === 'PUT' && mergeCall.body.sha === 'h1', 'merges exactly the reviewed head');
assert.equal(await hook('pull_request', { action: 'closed', pull_request: pr(10, 'Alice', 'h1', '', { merged: true }) }), 200);
assert.equal(row("SELECT accepted FROM accounts WHERE login = 'alice'").accepted, 1);
assert.equal(row("SELECT kind, status FROM tasks WHERE id = 2").status, 'done');
assert.ok((await call('GET', '/desk/next?kind=code', 'alice')).data.lease, 'with a merged test, code tasks open up');

// Pull requests without a lease are closed.
calls.length = 0;
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(11, 'stranger', 'h9', 'hi') }), 200);
assert.ok(ghCalls('/pulls/11').some((c) => c.method === 'PATCH' && c.body.state === 'closed'));
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(12, 'boss', 'h8', '') }), 200);
assert.equal(row('SELECT COUNT(*) AS c FROM prs WHERE number = 12').c, 0, 'core pull requests never go through the desk');

// A decoy: two reviewers miss it, one catches it.
const decoy = await call('POST', '/admin/decoy', 'core', { task: 1, login: 'decoybot', note: 'the car is ten times faster' });
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(20, 'decoybot', 'd1', `Lease: ${decoy.data.lease}`) }), 200);
assert.equal(await hook('status', { context: 'lastgame/verdict', sha: 'd1', state: 'success', description: ok() }), 200);
const dr = db.prepare('SELECT * FROM reviews WHERE pr = 20').all();
const repBefore = Object.fromEntries(dr.map((r) => [r.reviewer, rep(row('SELECT * FROM accounts WHERE login = ?', r.reviewer))]));
for (const [i, r] of dr.entries()) {
  await call('POST', '/desk/review', r.reviewer, { review: r.id, verdict: i === 0 ? 'reject' : 'accept', reason: 'looked at it' });
}
assert.equal(row("SELECT state FROM prs WHERE number = 20").state, 'closed');
assert.equal(row('SELECT caught FROM accounts WHERE login = ?', dr[0].reviewer).caught, 1);
assert.equal(row('SELECT missed FROM accounts WHERE login = ?', dr[1].reviewer).missed, 1);
assert.ok(!ghCalls('/pulls/20/merge').length, 'a decoy is never merged');
assert.equal(rep(row('SELECT * FROM accounts WHERE login = ?', dr[1].reviewer)), repBefore[dr[1].reviewer] - 3, 'missing a decoy costs 3');
assert.equal(rep(row('SELECT * FROM accounts WHERE login = ?', dr[0].reviewer)), repBefore[dr[0].reviewer] + 1, 'catching one earns 1');

// Two rejected rounds send a pull request to the graveyard.
for (const t of ['Lights at the park', 'Pedestrian buttons']) {
  assert.equal((await call('POST', '/desk/tasks', 'planner', { goal: 'c1', kind: 'code', title: t, detail: 'see the goal' })).status, 201);
}
const l2 = (await call('GET', '/desk/next', 'alice')).data.lease;
assert.ok(l2);
await hook('pull_request', { action: 'opened', pull_request: pr(30, 'alice', 'x1', `Lease: ${l2}`) });
for (const [round, head] of [[1, 'x1'], [2, 'x2']]) {
  if (round === 2) await hook('pull_request', { action: 'synchronize', pull_request: pr(30, 'alice', 'x2') });
  await hook('status', { context: 'lastgame/verdict', sha: head, state: 'success', description: ok() + (round === 2 ? ' visual 62.2%' : '') });
  assert.equal(row('SELECT need FROM rounds WHERE pr = 30 AND round = ?', round).need, round === 2 ? 5 : 3, 'a head that changes the picture draws five');
  const r = db.prepare('SELECT * FROM reviews WHERE pr = 30 AND round = ? AND verdict IS NULL').all(round)[0];
  assert.equal((await call('POST', '/desk/review', r.reviewer, { review: r.id, verdict: 'reject', reason: 'does not do what the task says' })).status, 200);
}
assert.equal(row('SELECT state FROM prs WHERE number = 30').state, 'rejected');

// Stale review answers are refused after a push.
const l3 = (await call('GET', '/desk/next', 'alice')).data.lease;
assert.ok(l3);
for (const t of ['Spare task one', 'Spare task two']) await call('POST', '/desk/tasks', 'planner', { goal: 'c1', kind: 'code', title: t, detail: 'see the goal' });
assert.ok((await call('GET', '/desk/next', 'alice')).data.lease, 'third open lease');
assert.equal((await call('GET', '/desk/next', 'alice')).status, 429, 'at most 3 open leases per owner');
await hook('pull_request', { action: 'opened', pull_request: pr(40, 'alice', 'y1', `Lease: ${l3}`) });
await hook('status', { context: 'lastgame/verdict', sha: 'y1', state: 'success', description: ok() });
const stale = db.prepare('SELECT * FROM reviews WHERE pr = 40').get();
await hook('pull_request', { action: 'synchronize', pull_request: pr(40, 'alice', 'y2') });
assert.equal((await call('POST', '/desk/review', stale.reviewer, { review: stale.id, verdict: 'accept', reason: 'ok' })).status, 410);

// Kill switch: a red main or a manual freeze holds approved merges.
mainStatus = { verdict: 'REJECTED', metrics: {} };
await tick(env);
await hook('status', { context: 'lastgame/verdict', sha: 'y2', state: 'success', description: ok() });
calls.length = 0;
for (const r of db.prepare("SELECT * FROM reviews WHERE pr = 40 AND verdict IS NULL").all()) {
  await call('POST', '/desk/review', r.reviewer, { review: r.id, verdict: 'accept', reason: 'fine' });
}
assert.equal(row('SELECT state FROM prs WHERE number = 40').state, 'approved');
assert.equal(ghCalls('/pulls/40/merge').length, 0, 'main is red: no merge');
mainStatus = { verdict: 'ACCEPTED', metrics: { tests_green: 'c0_car_turns c1_lights_cycle', tests_red: '', benchmarks: 'r1' } };
await call('POST', '/admin/freeze', 'core', { on: true, why: 'testing' });
await tick(env);
assert.equal(ghCalls('/pulls/40/merge').length, 0, 'frozen: no merge');
await call('POST', '/admin/freeze', 'core', { on: false, why: 'done testing' });
await tick(env);
assert.equal(ghCalls('/pulls/40/merge').length, 1, 'unfrozen: the approved merge goes through');
await hook('pull_request', { action: 'closed', pull_request: pr(40, 'alice', 'y2', '', { merged: true }) });
assert.equal(row("SELECT status FROM goals WHERE id = 'c1'").status, 'done', 'all c1 tests green on main');

// Cold start: with one eligible reviewer the round has one, and an expired review goes back to them.
for (const a of db.prepare("SELECT login FROM accounts WHERE login NOT IN ('alice', 'planner')").all()) {
  db.prepare('UPDATE accounts SET banned = 1 WHERE login = ?').run(a.login);
}
assert.equal((await call('GET', '/api/summary')).data.reviewers, 1);
assert.ok((await call('GET', '/api/summary')).data.prs.merged >= 1, 'stage counts');
await call('POST', '/admin/goals', 'core', { goals: [{ id: 'c0', kind: 'content', horizon: 'S', title: 'Standing', done_when: 'never', status: 'standing' }] });
assert.equal((await call('POST', '/desk/tasks', 'planner', { goal: 'c0', kind: 'test', title: 'Test a bug', detail: 'a failing test for the issue' })).status, 201);
const l4 = (await call('GET', '/desk/next', 'alice')).data.lease;
assert.ok(l4);
assert.equal((await call('POST', '/desk/tasks', 'planner', { goal: 'c0', kind: 'code', title: 'Keep the car turning', detail: 'guarded by c0_car_turns' })).status, 201);
const codeLease = await call('GET', '/desk/next?kind=code', 'planner');
assert.equal(codeLease.data.task?.kind, 'code', 'a test already on main opens code tasks');
await call('POST', '/admin/goals', 'core', { goals: [{ id: 'r1', kind: 'research', horizon: 'L', title: 'Light', done_when: 'benchmark', status: 'standing' }] });
assert.equal((await call('POST', '/desk/tasks', 'planner', { goal: 'r1', kind: 'code', title: 'An entry for R1', detail: 'beat the direct light' })).status, 201);
const benchLease = await call('GET', '/desk/next?kind=code', 'planner');
assert.equal(benchLease.data.goal?.id, 'r1', 'a research goal with a benchmark opens code tasks');
for (const l of [codeLease, benchLease]) await call('POST', `/desk/release?lease=${l.data.lease}&reason=other`, 'planner');
await hook('pull_request', { action: 'opened', pull_request: pr(50, 'alice', 'z1', `Lease: ${l4}`) });
await hook('status', { context: 'lastgame/verdict', sha: 'z1', state: 'success', description: ok() });
assert.deepEqual(db.prepare('SELECT reviewer FROM reviews WHERE pr = 50').all().map((r) => r.reviewer), ['planner']);
clock += 3 * HOUR;
await tick(env);
const again = db.prepare('SELECT * FROM reviews WHERE pr = 50 AND verdict IS NULL').get();
assert.equal(again?.reviewer, 'planner', 'nobody else: the same reviewer gets another window');
calls.length = 0;
mainSha = 'b'.repeat(40); // something else was merged after the verdict
assert.equal((await call('POST', '/desk/review', 'planner', { review: again.id, verdict: 'accept', reason: 'matches the task' })).status, 200);
assert.equal(ghCalls('/pulls/50/merge').length, 0, 'a verdict on an older main is not merged');
assert.equal(ghCalls('/actions/runs/77/rerun').length, 1, 'the checker runs again on the new main');
await tick(env);
assert.equal(ghCalls('/actions/runs/77/rerun').length, 1, 'once per main commit');
await hook('status', { context: 'lastgame/verdict', sha: 'z1', state: 'success', description: ok() });
assert.equal(ghCalls('/pulls/50/merge').length, 1, 'green on the new main: merged, the only reviewer is the whole quorum');
db.prepare('UPDATE accounts SET banned = 0').run();

// The front page's one request carries the summary, the agenda and the latest events.
const home = (await call('GET', '/api/home')).data;
assert.ok(home.summary.owners > 0 && Array.isArray(home.goals.goals) && home.events.length <= 12);

// Public reads never leak keys.
const contributors = await call('GET', '/api/contributors');
assert.ok(contributors.data.every((c) => !('key_hash' in c) && !('github_id' in c)));
const goals = await call('GET', '/api/goals');
assert.ok(goals.data.goals.find((g) => g.id === 'c1').status === 'done');
assert.equal((await call('GET', '/desk/me')).status, 401);

// Challenges: a jury of other owners can revert a merged pull request.
clock += 8 * DAY; // pull requests 10 and 40 are now too old to challenge
for (const p of ['auth', 'critic', 'j1', 'j2', 'j3', 'j4', 'j5']) await person(p, { bonus: 5 });
// Reputation elsewhere depends on the random decoy draw: only these take part, and the reviewers are eligible,
// so only the exclusion keeps them off the jury.
db.prepare("UPDATE accounts SET banned = 1 WHERE login NOT IN ('auth', 'critic', 'young', 'r1', 'r2', 'r3', 'r4', 'r5', 'j1', 'j2', 'j3', 'j4', 'j5')").run();
db.prepare("UPDATE accounts SET bonus = 10 WHERE login IN ('r1', 'r2', 'r3', 'r4', 'r5')").run();
const merged = async (n, reviews) => {
  db.prepare("INSERT INTO prs (number, author, lease, head, title, state, rounds, updated) VALUES (?, 'auth', 'x', ?, ?, 'approved', 2, ?)").run(n, `m${n}`, `pr ${n}`, clock);
  for (const [who, verdict, head] of reviews) {
    db.prepare("INSERT INTO reviews (id, pr, head, round, reviewer, due, verdict, reason) VALUES (?, ?, ?, 1, ?, ?, ?, 'ok')").run(`${n}${who}${head}`, n, head, who, clock, verdict);
  }
  await hook('pull_request', { action: 'closed', pull_request: pr(n, 'auth', `m${n}`, '', { merged: true }) });
};
// Round 1 on an older head: r5 accepted, r4 rejected. Round 2 on the merged head: r1, r2, r3 accepted.
await merged(60, [['r5', 'accept', 'old'], ['r4', 'reject', 'old'], ['r1', 'accept', 'm60'], ['r2', 'accept', 'm60'], ['r3', 'accept', 'm60']]);
await merged(61, [['r1', 'accept', 'm61']]);
await merged(62, [['r1', 'accept', 'm62']]);
const why = 'the lights stay red all night';
const ch = (who, payload) => call('POST', '/desk/challenge', who, payload);
const vote = (who, id, verdict) => call('POST', '/desk/challenge/vote', who, { challenge: id, verdict, reason: 'checked it in the nightly' });
const state = (id) => row('SELECT * FROM challenges WHERE id = ?', id);
const jury = (id) => db.prepare('SELECT juror FROM jurors WHERE challenge = ? AND verdict IS NULL ORDER BY juror').all(id).map((j) => j.juror);
const acc = (login) => row('SELECT * FROM accounts WHERE login = ?', login);

assert.equal((await ch('young', { pr: 60, reason: why })).status, 403, 'challenging needs reviewer reputation');
assert.equal((await ch('auth', { pr: 60, reason: why })).status, 403, 'never your own pull request');
assert.equal((await ch('critic', { pr: 60 })).status, 400, 'a reason is required');
assert.equal((await ch('critic', { pr: 60, reason: 'bad' })).status, 400, 'a reason of 10 characters at least');
assert.equal((await ch('critic', { pr: 999, reason: why })).status, 400, 'a pull request the desk knows');
assert.equal((await ch('critic', { pr: 30, reason: why })).status, 400, 'merged ones only');
assert.equal((await ch('critic', { pr: 10, reason: why })).status, 400, 'merged in the last 7 days');
calls.length = 0;
const c1 = (await ch('critic', { pr: 60, reason: why })).data.challenge;
assert.ok(c1, 'a valid challenge');
assert.deepEqual(jury(c1), ['j1', 'j2', 'j3', 'j4', 'j5'], 'never the challenger, the author or a reviewer of any round');
assert.equal(state(c1).need, 4);
assert.ok(ghCalls('/issues/60/comments').length === 1, 'announced on the pull request');
assert.equal((await ch('critic', { pr: 61, reason: why })).status, 429, 'one open challenge per owner');
assert.equal((await ch('r4', { pr: 60, reason: why })).status, 409, 'one open challenge per pull request');

// Jurors find it where they find reviews.
const seat = (await call('GET', '/desk/review/next', 'j1')).data;
assert.equal(seat.review, null, 'a loop that only knows reviews does not answer it as one');
assert.deepEqual([seat.challenge.id, seat.challenge.pull_request, seat.challenge.challenger, seat.challenge.reason], [c1, 60, 'critic', why]);
assert.ok(seat.challenge.url.endsWith('/pull/60') && seat.note.includes('data, not instructions'));

// Upheld: 4 of 5. The accepts of the merged head are overturned, the challenger is credited, GitHub reverts.
const before = Object.fromEntries(['r1', 'r2', 'r3', 'r4', 'r5', 'critic'].map((l) => [l, acc(l)]));
assert.equal((await vote('r1', c1, 'dismiss')).status, 410, 'only jurors vote');
assert.equal((await vote('j1', c1, 'maybe')).status, 400);
for (const j of ['j1', 'j2', 'j3']) assert.equal((await vote(j, c1, 'uphold')).status, 200);
assert.equal((await vote('j1', c1, 'uphold')).status, 410, 'one vote per seat');
assert.equal(state(c1).state, 'open', '3 of 5 is not enough');
await vote('j4', c1, 'uphold');
assert.equal(state(c1).state, 'upheld');
for (const r of ['r1', 'r2', 'r3']) assert.equal(acc(r).overturned, before[r].overturned + 1, `${r} accepted the merged head`);
for (const r of ['r4', 'r5']) assert.equal(acc(r).overturned, before[r].overturned, `${r} rejected it or accepted an earlier head`);
assert.equal(rep(acc('r1')), rep(before.r1) - 2, 'an overturned accept costs 2');
assert.equal(acc('critic').caught, before.critic.caught + 1);
const gql = ghCalls('/graphql')[0];
assert.ok(gql.body.query.includes('revertPullRequest') && gql.body.variables.id === 'PR_60' && gql.body.variables.body.startsWith('Reverts #60'), 'GitHub is asked to revert');
assert.equal(state(c1).revert, 99);
assert.deepEqual({ ...row('SELECT author, state, head FROM prs WHERE number = 99') }, { author: 'desk[bot]', state: 'approved', head: 'rv1' });
assert.equal((await vote('j5', c1, 'dismiss')).status, 410, 'decided: the last seat closes');
assert.equal((await ch('r4', { pr: 60, reason: why })).status, 409, 'an upheld challenge is not opened again');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type IN ('challenge.opened', 'challenge.upheld', 'revert.opened') AND ref = '60'").c, 3);
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'challenge.vote' AND ref = '60'").c, 4);

// The revert is the desk's own: its bot opened it and D1 has the challenge. The same text from anyone else is closed.
const bot = { login: 'lastgame-desk[bot]', type: 'Bot' };
const closed = (n) => ghCalls(`/pulls/${n}`).some((c) => c.method === 'PATCH' && c.body.state === 'closed');
calls.length = 0;
await hook('pull_request', { action: 'opened', pull_request: pr(99, '', 'rv1', 'Reverts #60. A jury upheld it.', { user: bot }) });
assert.ok(!closed(99), 'no lease, but not closed');
db.prepare('UPDATE challenges SET revert = NULL WHERE id = ?').run(c1); // the webhook can beat the desk writing the number down
await hook('pull_request', { action: 'reopened', pull_request: pr(99, '', 'rv1', 'Reverts #60. A jury upheld it.', { user: bot }) });
assert.ok(!closed(99), 'also before the desk wrote the number down');
db.prepare('UPDATE challenges SET revert = 99 WHERE id = ?').run(c1);
await hook('pull_request', { action: 'opened', pull_request: pr(98, 'stranger', 'h98', 'Reverts #60.') });
assert.ok(closed(98), 'a person writing the marker is closed');
await hook('pull_request', { action: 'opened', pull_request: pr(97, '', 'h97', 'Reverts #61.', { user: bot }) });
assert.ok(closed(97), 'a bot without an upheld challenge is closed');
assert.equal(row('SELECT state FROM prs WHERE number = 99').state, 'approved');

// It merges once the checker passed it, without a review round. The freeze still holds it.
await call('POST', '/admin/freeze', 'core', { on: true, why: 'testing reverts' });
await hook('status', { context: 'lastgame/verdict', sha: 'rv1', state: 'success', description: ok() });
assert.equal(ghCalls('/pulls/99/merge').length, 0, 'frozen: no revert either');
await call('POST', '/admin/freeze', 'core', { on: false, why: 'done testing' });
await tick(env);
assert.equal(ghCalls('/pulls/99/merge')[0]?.body.sha, 'rv1', 'merged by the desk');
assert.equal(row('SELECT COUNT(*) AS c FROM reviews WHERE pr = 99').c, 0, 'no review round: the jury decided');
const authBefore = acc('auth').accepted;
await hook('pull_request', { action: 'closed', pull_request: pr(99, '', 'rv1', '', { user: bot, merged: true }) });
assert.equal(acc('auth').accepted, authBefore - 1, 'a merged revert takes back the point the original earned');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'revert.merged' AND ref = '99'").c, 1);
assert.equal((await ch('r4', { pr: 99, reason: why })).status, 400, 'a revert cannot be challenged');

// Dismissed as soon as 4 upholds are out of reach. The challenger pays like an overturned accept.
clock += DAY;
calls.length = 0;
const c2 = (await ch('critic', { pr: 61, reason: why })).data.challenge;
const seats = jury(c2);
assert.equal(seats.length, 5);
const critic = acc('critic');
await vote(seats[0], c2, 'dismiss');
assert.equal(state(c2).state, 'open', '4 upholds are still possible');
await vote(seats[1], c2, 'dismiss');
assert.equal(state(c2).state, 'dismissed');
assert.equal(acc('critic').overturned, critic.overturned + 1);
assert.equal(rep(acc('critic')), rep(critic) - 2);
assert.equal(jury(c2).length, 0, 'the other seats close');
assert.equal(ghCalls('/graphql').length, 0, 'nothing is reverted');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'challenge.dismissed' AND ref = '61'").c, 1);
assert.equal((await ch('critic', { pr: 62, reason: why })).status, 429, 'one new challenge a day');

// Short of 4, but most of the jury agreed: dismissed, and the challenger does not pay.
clock += DAY;
const c2b = (await ch('critic', { pr: 61, reason: why })).data.challenge;
const seats2 = jury(c2b);
const critic2 = acc('critic');
for (const j of seats2.slice(0, 3)) await vote(j, c2b, 'uphold');
await vote(seats2[3], c2b, 'dismiss');
assert.equal(state(c2b).state, 'open', 'the last seat could still make it 4');
await vote(seats2[4], c2b, 'dismiss');
assert.equal(state(c2b).state, 'dismissed', '3 of 5 is not enough');
assert.equal(acc('critic').overturned, critic2.overturned, 'no penalty when most of the jury upheld');
assert.equal(ghCalls('/graphql').length, 0, 'nothing is reverted');

// Cold start: with fewer than 3 jurors the challenge waits for the cron. 3 jurors must all uphold.
clock += DAY;
db.prepare("UPDATE accounts SET banned = 1 WHERE login IN ('r2', 'r3', 'r4', 'r5', 'j3', 'j4', 'j5')").run();
const c3 = (await ch('critic', { pr: 62, reason: why })).data.challenge;
assert.equal(state(c3).state, 'waiting', 'two eligible jurors are not a jury');
await tick(env);
assert.equal(state(c3).state, 'waiting');
db.prepare("UPDATE accounts SET banned = 0 WHERE login = 'j3'").run();
await tick(env);
assert.deepEqual(jury(c3), ['j1', 'j2', 'j3']);
assert.equal(state(c3).need, 3);
clock += 25 * HOUR;
await tick(env);
assert.deepEqual(jury(c3), ['j1', 'j2', 'j3'], 'seats that ran out go back to the same jurors when nobody else is free');
graphqlReply = { errors: [{ message: 'could not revert: merge conflict' }] };
calls.length = 0;
for (const j of ['j1', 'j2']) await vote(j, c3, 'uphold');
assert.equal(state(c3).state, 'open', 'a jury of 3 needs all 3');
await vote('j3', c3, 'uphold');
assert.equal(state(c3).state, 'upheld');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'revert.failed' AND ref = '62'").c, 1, 'a revert GitHub refuses is recorded');
assert.ok(ghCalls('/issues/62/comments').some((c) => c.body.body.startsWith('**Revert failed:**')), 'and left to the core team on the pull request');
assert.equal((await call('GET', '/api/challenges')).data.length, 4);
db.prepare('UPDATE accounts SET banned = 0').run();

// Stewardship: a core team member's change does not wait for reviewers who are not there.
await call('POST', '/admin/freeze', 'core', { on: false, why: 'stewardship tests' });
mainStatus = { verdict: 'ACCEPTED', metrics: {} };
await tick(env);
for (const s of ['s1', 's2', 's3']) await person(s, { bonus: 3 });
await call('POST', '/admin/goals', 'core', { goals: [{ id: 'c9', kind: 'content', horizon: 'S', title: 'Standing', done_when: 'never', status: 'standing' }] });
for (const n of [70, 71, 72, 73]) assert.equal((await call('POST', '/desk/tasks', 's1', { goal: 'c9', kind: 'test', title: `Steward task ${n}`, detail: 'a failing test for the issue' })).status, 201);
const stewardPr = async (n, login, head) => {
  const lease = (await call('GET', '/desk/next?kind=test', login)).data.lease;
  assert.ok(lease, `a lease for #${n}`);
  await hook('pull_request', { action: 'opened', pull_request: pr(n, login, head, `Lease: ${lease}`) });
  await hook('status', { context: 'lastgame/verdict', sha: head, state: 'success', description: ok() });
};
const stewarded = (n) => ghCalls(`/issues/${n}/comments`).some((c) => c.body.body.startsWith('**Stewardship merge.**'));
db.prepare("UPDATE accounts SET banned = 1 WHERE login NOT IN ('boss', 'alice')").run();
calls.length = 0;
await stewardPr(70, 'alice', 'st0');
assert.equal(row('SELECT state FROM prs WHERE number = 70').state, 'checked', 'with nobody to draw, anyone else waits');
await tick(env);
assert.equal(ghCalls('/pulls/70/merge').length, 0);
const coreLease = (await call('GET', '/desk/next?kind=test', 'boss')).data.lease;
await hook('pull_request', { action: 'opened', pull_request: pr(75, 'boss', 'st5', `Lease: ${coreLease}`) });
await hook('status', { context: 'lastgame/verdict', sha: 'st5', state: 'success', description: ok().replace(' swarm', '') });
assert.equal(row('SELECT state FROM prs WHERE number = 75').state, 'open', 'a verdict that was not under the swarm\'s rules does not count');
await hook('pull_request', { action: 'closed', pull_request: pr(75, 'boss', 'st5') });
await stewardPr(71, 'boss', 'st1');
assert.equal(row('SELECT state FROM prs WHERE number = 71').state, 'approved');
await tick(env);
assert.equal(ghCalls('/pulls/71/merge').length, 1, 'nobody to draw: the core team\'s change merges on the verdict');
assert.ok(stewarded(71), 'and the desk says so on the pull request');
assert.equal(row('SELECT why FROM prs WHERE number = 71').why, 'stewardship: there is no reviewer to draw', 'and on the Built page');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'review.steward' AND ref = '71'").c, 1, 'and in the public record');
db.prepare("UPDATE accounts SET banned = 0 WHERE login IN ('s1', 's2', 's3')").run();
await stewardPr(72, 'boss', 'st2');
const stSeats = db.prepare('SELECT * FROM reviews WHERE pr = 72').all();
assert.equal(stSeats.length, 3, 'with reviewers around, the desk draws them as for anyone');
assert.equal(ghCalls('/pulls/72/merge').length, 0, 'and waits for them');
await call('POST', '/desk/review', stSeats[0].reviewer, { review: stSeats[0].id, verdict: 'accept', reason: 'matches the task' });
clock += RULES.reviewHours * HOUR + 60;
await tick(env);
assert.equal(ghCalls('/pulls/72/merge').length, 1, 'the window ended without a reject: merged, once');
assert.equal(row('SELECT COUNT(*) AS c FROM reviews WHERE pr = 72 AND verdict IS NULL').c, 0, 'no seat stays open');
await stewardPr(73, 'boss', 'st3');
const stSeat = db.prepare('SELECT * FROM reviews WHERE pr = 73').get();
await call('POST', '/desk/review', stSeat.reviewer, { review: stSeat.id, verdict: 'reject', reason: 'does not do what the task says' });
clock += RULES.reviewHours * HOUR + 60;
await tick(env);
assert.equal(row('SELECT state FROM prs WHERE number = 73').state, 'open', 'a reject still sends it back');
assert.equal(ghCalls('/pulls/73/merge').length, 0);
const stDecoy = await call('POST', '/admin/decoy', 'core', { task: row("SELECT id FROM tasks WHERE title = 'Steward task 73'").id, login: 'boss', note: 'deliberately wrong' });
db.prepare("UPDATE accounts SET banned = 1 WHERE login IN ('s1', 's2', 's3')").run();
await hook('pull_request', { action: 'opened', pull_request: pr(74, 'boss', 'st4', `Lease: ${stDecoy.data.lease}`) });
await hook('status', { context: 'lastgame/verdict', sha: 'st4', state: 'success', description: ok() });
await tick(env);
assert.equal(row('SELECT state FROM prs WHERE number = 74').state, 'checked', 'a decoy waits for reviewers, whoever plants it');
assert.equal(ghCalls('/pulls/74/merge').length, 0, 'and is never merged');
db.prepare('UPDATE accounts SET banned = 0').run();

// The alpha's way out: a new season keeps accounts and the record, drops everything else.
assert.equal((await call('POST', '/admin/season', 'core', { why: 'testing' })).status, 400, 'needs the confirmation');
assert.equal((await call('POST', '/admin/season', 'alice', { confirm: 'start over', why: 'x' })).status, 401);
assert.equal((await call('POST', '/admin/season', 'core', { confirm: 'start over', why: 'the alpha ended badly' })).status, 200);
for (const t of ['votes', 'tasks', 'leases', 'prs', 'reviews', 'rounds', 'verdicts', 'goals', 'challenges', 'jurors']) assert.equal(row(`SELECT COUNT(*) AS c FROM ${t}`).c, 0, `${t} emptied`);
assert.ok(row('SELECT COUNT(*) AS c FROM accounts').c > 5, 'accounts stay');
assert.equal(row('SELECT MAX(accepted + caught + missed + bonus) AS m FROM accounts').m, 0, 'earned points go');
assert.equal(row("SELECT COUNT(*) AS c FROM events WHERE type = 'admin.season'").c, 1, 'in the public record');
seedGoalsFile = { goals: [{ id: 'c1', kind: 'content', horizon: 'S', title: 'Traffic lights', done_when: 'every crossing has a light' }] };
await tick(env);
assert.equal(row("SELECT status FROM goals WHERE id = 'c1'").status, 'ranked', 'the next tick seeds the agenda again');
assert.equal((await call('POST', '/admin/phase', 'core', { phase: 'beta', why: 'starting again' })).status, 200);
assert.equal((await call('GET', '/api/summary')).data.phase, 'beta');

// A cached copy goes out with our short max-age, not the zone's 4 h browser TTL.
const store = new Map();
globalThis.caches = { default: {
  match: async (r) => store.get(r.url)?.clone(),
  put: async (r, res) => { const c = new Response(await res.text(), res); c.headers.set('cache-control', 'public, max-age=14400'); store.set(r.url, c); },
} };
const ctx = { waitUntil: (p) => p };
await desk.fetch(new Request('https://site/api/summary'), env, ctx);
await new Promise((r) => setTimeout(r, 0));
const hit = await desk.fetch(new Request('https://site/api/summary'), env, ctx);
assert.equal(hit.headers.get('cache-control'), 'public, max-age=15', 'a cache hit keeps max-age=15');
assert.equal((await hit.json()).phase, 'beta', 'and the same body');
delete globalThis.caches;

console.log('desk: all tests passed');
