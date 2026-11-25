// Local desk for trying the site: real worker code, SQLite in memory, the web/
// folder as static assets, sample data. GitHub calls are not made.
//   node desk/dev.mjs   then open http://localhost:8788
import { createServer } from 'node:http';
import { readFileSync, existsSync } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';
import { extname, join, normalize } from 'node:path';
import desk, { DAY, tick } from './src/worker.js';
import { seedGoals } from './seed-goals.mjs';

const root = new URL('..', import.meta.url).pathname;
const db = new DatabaseSync(':memory:');
db.exec(readFileSync(join(root, 'desk/schema.sql'), 'utf8'));
const stmt = (sql, a) => db.prepare(sql);
const DB = { prepare: (sql) => ({ bind: (...a) => ({
  first: async () => stmt(sql).get(...a) ?? null,
  all: async () => ({ results: stmt(sql).all(...a) }),
  run: async () => { const r = stmt(sql).run(...a); return { meta: { changes: Number(r.changes), last_row_id: Number(r.lastInsertRowid) } }; },
}) }) };

const TYPES = { '.html': 'text/html; charset=utf-8', '.js': 'text/javascript', '.css': 'text/css', '.wasm': 'application/wasm', '.txt': 'text/plain; charset=utf-8', '.json': 'application/json' };
const ASSETS = { fetch: async (req) => {
  let p = normalize(decodeURIComponent(new URL(req.url).pathname)).replace(/^(\.\.[/\\])+/, '');
  if (p === '/') p = '/index.html';
  if (!extname(p)) p += '.html';
  const file = join(root, 'web', p);
  if (!file.startsWith(join(root, 'web')) || !existsSync(file)) return new Response('not found', { status: 404 });
  return new Response(readFileSync(file), { headers: { 'content-type': TYPES[extname(file)] ?? 'application/octet-stream' } });
} };
const env = { DB, ASSETS, REPO: 'lastgameevermade/lastgame', SITE: 'http://localhost:8788', ADMIN_KEY: 'dev' };
globalThis.fetch = async () => new Response('{}', { status: 503 }); // no GitHub here

// Sample data: the seed backlog, a few owners with votes, some history.
const now = Math.floor(Date.now() / 1000);
await desk.fetch(new Request('http://x/admin/goals', { method: 'POST', headers: { authorization: 'Bearer dev' }, body: JSON.stringify({ goals: seedGoals(readFileSync(join(root, 'GOALS.md'), 'utf8')) }) }), env);
for (const [i, who] of ['ada', 'grace', 'linus', 'margaret'].entries()) {
  db.prepare('INSERT INTO accounts (login, github_id, created_gh, joined, bonus, accepted, caught) VALUES (?, ?, 0, ?, 3, ?, ?)').run(who, i + 1, now - i * DAY, 5 - i, i);
}
const day = Math.floor(now / DAY);
for (const [who, goal, n, why] of [['ada', 'c11', 3, 'traffic makes the city feel alive'], ['grace', 'c11', 2, 'needed before missions'],
  ['linus', 't1', 3, 'unblocks bigger cities'], ['margaret', 'r1', 2, 'the research nobody else will do'], ['ada', 'p1', 2, 'half the visitors are on phones'],
  ['grace', 'c9', 2, 'camera clipping is the first thing people notice'], ['linus', 'c2', 1, 'cheap and visible']]) {
  db.prepare('INSERT INTO votes (owner, goal, day, n, reason) VALUES (?, ?, ?, ?, ?)').run(who, goal, day, n, why);
}
db.prepare("INSERT INTO prs (number, author, head, state, title, why, updated) VALUES (1, 'ada', 'a', 'merged', 'c0: street lamps along the main road', NULL, ?), (2, 'linus', 'b', 'rejected', 'c2: parked cars', '- cars overlap the pavement\n- draws one mesh per car instead of one for all', ?)").run(now - 4000, now - 2000);
await tick(env);
db.prepare("UPDATE goals SET status = 'active', activated = ? WHERE id IN ('c9', 'c11')").run(now);

createServer(async (req, res) => {
  const chunks = [];
  for await (const c of req) chunks.push(c);
  const r = await desk.fetch(new Request(`http://localhost:8788${req.url}`, { method: req.method, headers: req.headers, body: ['GET', 'HEAD'].includes(req.method) ? undefined : Buffer.concat(chunks) }), env);
  res.writeHead(r.status, Object.fromEntries(r.headers));
  res.end(Buffer.from(await r.arrayBuffer()));
}).listen(8788, () => console.log('desk on http://localhost:8788'));
