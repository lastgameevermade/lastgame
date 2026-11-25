// End-to-end tests for the desk against a real SQLite database and a fake
// GitHub. Run: node desk/test.mjs   (Node 22.5+, uses node:sqlite)
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import desk, { RULES, agenda, cost, credits, decay, doneGoals, pickReviewers, rep, score, tick, DAY, HOUR } from './src/worker.js';
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

// An empty database seeds itself from goals.json on the first tick.
await tick(env);
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

await tick(env);
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
assert.equal(await hook('status', { context: 'lastgame/verdict', sha: 'h1', state: 'success' }), 200);
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
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(12, 'boss', 'h8', '', { author_association: 'MEMBER' }) }), 200);
assert.equal(row('SELECT COUNT(*) AS c FROM prs WHERE number = 12').c, 0, 'core pull requests never go through the desk');

// A decoy: two reviewers miss it, one catches it.
const decoy = await call('POST', '/admin/decoy', 'core', { task: 1, login: 'decoybot', note: 'the car is ten times faster' });
assert.equal(await hook('pull_request', { action: 'opened', pull_request: pr(20, 'decoybot', 'd1', `Lease: ${decoy.data.lease}`) }), 200);
assert.equal(await hook('status', { context: 'lastgame/verdict', sha: 'd1', state: 'success' }), 200);
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
  await hook('status', { context: 'lastgame/verdict', sha: head, state: 'success' });
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
await hook('status', { context: 'lastgame/verdict', sha: 'y1', state: 'success' });
const stale = db.prepare('SELECT * FROM reviews WHERE pr = 40').get();
await hook('pull_request', { action: 'synchronize', pull_request: pr(40, 'alice', 'y2') });
assert.equal((await call('POST', '/desk/review', stale.reviewer, { review: stale.id, verdict: 'accept', reason: 'ok' })).status, 410);

// Kill switch: a red main or a manual freeze holds approved merges.
mainStatus = { verdict: 'REJECTED', metrics: {} };
await tick(env);
await hook('status', { context: 'lastgame/verdict', sha: 'y2', state: 'success' });
calls.length = 0;
for (const r of db.prepare("SELECT * FROM reviews WHERE pr = 40 AND verdict IS NULL").all()) {
  await call('POST', '/desk/review', r.reviewer, { review: r.id, verdict: 'accept', reason: 'fine' });
}
assert.equal(row('SELECT state FROM prs WHERE number = 40').state, 'approved');
assert.equal(ghCalls('/pulls/40/merge').length, 0, 'main is red: no merge');
mainStatus = { verdict: 'ACCEPTED', metrics: { tests_green: 'c0_car_turns c1_lights_cycle', tests_red: '' } };
await call('POST', '/admin/freeze', 'core', { on: true, why: 'testing' });
await tick(env);
assert.equal(ghCalls('/pulls/40/merge').length, 0, 'frozen: no merge');
await call('POST', '/admin/freeze', 'core', { on: false, why: 'done testing' });
await tick(env);
assert.equal(ghCalls('/pulls/40/merge').length, 1, 'unfrozen: the approved merge goes through');
assert.equal(row("SELECT status FROM goals WHERE id = 'c1'").status, 'done', 'all c1 tests green on main');

// Public reads never leak keys.
const contributors = await call('GET', '/api/contributors');
assert.ok(contributors.data.every((c) => !('key_hash' in c) && !('github_id' in c)));
const goals = await call('GET', '/api/goals');
assert.ok(goals.data.goals.find((g) => g.id === 'c1').status === 'done');
assert.equal((await call('GET', '/desk/me')).status, 401);

console.log('desk: all tests passed');
