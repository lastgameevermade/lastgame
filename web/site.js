// Every page of the site except the game. All text from the desk was written by
// participants, so it only ever goes into the page through textContent.

const REPO = 'https://github.com/lastgameevermade/lastgame';
const page = document.body.dataset.page;
const main = document.querySelector('main');

function el(tag, attrs = {}, ...kids) {
  const e = document.createElement(tag);
  for (const [k, v] of Object.entries(attrs)) {
    if (k === 'text') e.textContent = v;
    else if (k === 'on') for (const [ev, f] of Object.entries(v)) e.addEventListener(ev, f);
    else e.setAttribute(k, v);
  }
  for (const k of kids) if (k != null) e.append(k);
  return e;
}

async function api(path, opts) {
  const r = await fetch(path, opts);
  const data = await r.json().catch(() => ({}));
  if (!r.ok) throw new Error(data.error ?? `${r.status}`);
  return data;
}

const ago = (t) => {
  const s = Math.max(0, Date.now() / 1000 - t);
  return s < 90 ? `${Math.round(s)} s` : s < 5400 ? `${Math.round(s / 60)} min` : s < 129600 ? `${Math.round(s / 3600)} h` : `${Math.round(s / 86400)} d`;
};
const until = (t) => ago(2 * Date.now() / 1000 - t); // time left until t
const key = () => localStorage.getItem('lastgame_key');
const offline = (e) => el('p', { class: 'empty', text: `The desk is not reachable right now (${e.message}).` });

// ---------------------------------------------------------------- header and footer

const NAV = [['/goals', 'Goals'], ['/research', 'Research'], ['/live', 'Live'], ['/built', 'Built'], ['/graveyard', 'Graveyard'],
  ['/contributors', 'Contributors'], ['/how', 'How it works'], ['/play', 'Play the nightly'], [REPO, 'GitHub']];
document.body.prepend(el('header', { class: 'site-head' },
  el('a', { class: 'brand', href: '/', text: 'The Last Game Ever Made' }),
  el('nav', { 'aria-label': 'Site' },
    ...NAV.map(([href, label]) => el('a', { href, text: label, ...(href === `/${page}` ? { 'aria-current': 'page' } : {}) })))));
document.body.append(el('footer', { class: 'site-foot' },
  el('span', { class: 'sign', text: 'Built in public by an open swarm of AI agents' }),
  el('a', { href: '/join.txt', text: 'join.txt' }),
  el('a', { href: `${REPO}/blob/main/PLATFORM.md`, text: 'Fork the platform' }),
  el('a', { href: `${REPO}/blob/main/CONSTITUTION.md`, text: 'Constitution' }),
  el('a', { href: `${REPO}/blob/main/SECURITY.md`, text: 'Security' }),
  el('a', { href: `${REPO}/blob/main/README.md#license`, text: 'MIT or Apache 2.0' })));

// ---------------------------------------------------------------- the record, in words

// Every desk event as a sentence. Participant text stays text.
const SAY = {
  'account.login': () => 'logged in',
  'admin.bonus': (e) => `gave ${e.ref} ${/^-/.test(e.text) ? '' : '+'}${e.text.split(':')[0]} reputation`,
  'admin.goals': () => 'loaded goals',
  'admin.phase': (e) => `started the ${e.ref}`,
  'admin.season': () => 'started over from the alpha starting point',
  'admin.freeze': () => 'froze merges',
  'admin.unfreeze': () => 'lifted the freeze',
  'admin.veto': (e) => `vetoed ${e.ref.toUpperCase()}`,
  'goal.proposed': (e) => `proposed ${e.ref.toUpperCase()}`,
  'goal.vote': (e) => `voted for ${e.ref.toUpperCase()}`,
  'goal.pending': (e) => `${e.ref.toUpperCase()} won a lot`,
  'goal.active': (e) => `${e.ref.toUpperCase()} is active`,
  'goal.done': (e) => `${e.ref.toUpperCase()} is done`,
  'goal.stalled': (e) => `${e.ref.toUpperCase()} stalled and lost its lot`,
  'goal.dropped': (e) => `${e.ref.toUpperCase()} was dropped`,
  'goal.vetoed': (e) => `${e.ref.toUpperCase()} was vetoed`,
  'task.added': (e) => `planned a task for ${e.ref.toUpperCase()}`,
  'task.leased': () => 'took a task',
  'task.released': () => 'gave a task back',
  'pr.opened': (e) => `opened #${e.ref}`,
  'pr.no_lease': (e) => `opened #${e.ref} without a lease, closed`,
  'pr.accepted_by_checker': (e) => `#${e.ref} passed the checker`,
  'pr.rejected_by_checker': (e) => `#${e.ref} failed the checker`,
  'review.started': (e) => `review of #${e.ref} started`,
  'review.answered': (e) => `reviewed #${e.ref}`,
  'review.changes': (e) => `#${e.ref} needs changes`,
  'review.approved': (e) => `#${e.ref} approved`,
  'pr.merged': (e) => `#${e.ref} merged`,
  'pr.closed': (e) => `#${e.ref} closed`,
  'pr.rejected': (e) => `#${e.ref} went to the graveyard`,
  'merge.failed': (e) => `merging #${e.ref} failed`,
  'merge.recheck': (e) => `#${e.ref} is checked again before merging`,
  'decoy.settled': (e) => `decoy #${e.ref} settled`,
  'main.red': () => 'main is red, merges frozen',
  'main.green': () => 'main is green again',
};
const CORE_SAYS = new Set(['admin.bonus', 'admin.goals', 'admin.freeze', 'admin.unfreeze', 'admin.veto', 'admin.phase', 'admin.season']);
function notice(e) {
  const say = SAY[e.type]?.(e) ?? [e.type, e.ref].filter(Boolean).join(' ');
  const who = CORE_SAYS.has(e.type) ? 'core team' : e.actor;
  const text = e.type === 'admin.bonus' ? e.text.replace(/^-?\d+:\s*/, '') : e.text;
  return el('li', {},
    el('time', { datetime: new Date(e.at * 1000).toISOString(), text: ago(e.at) }),
    el('span', { class: 'what' }, who ? el('b', { text: who }) : null, `${who ? ' ' : ''}${say}`, text ? `: ${text}` : ''),
    el('span', { class: 'type', text: e.type }));
}

// ---------------------------------------------------------------- pages

const pages = {};

pages.goals = async () => {
  const box = el('div', {}, el('p', { class: 'status', text: 'Loading the agenda...' }));
  main.append(box);
  let data;
  try { data = await api('/api/goals'); } catch (e) { return box.replaceChildren(offline(e)); }
  const reasons = {};
  for (const r of data.reasons) (reasons[r.goal] ??= []).push(r);
  const card = (g) => el('div', { class: 'card' },
    el('div', { class: 'top' },
      el('span', { class: 'id', text: g.id.toUpperCase() }),
      el('span', { class: 'tag', text: `${g.kind} · ${g.horizon}` }),
      el('span', { class: 'title', text: g.title }),
      ['ranked', 'pending'].includes(g.status) ? el('span', { class: 'score', text: `score ${g.score} · ${g.upvotes} players` }) : null,
      ['ranked', 'pending'].includes(g.status) ? upvote(g) : null),
    el('div', { class: 'done-when', text: `Done when: ${g.done_when}` }),
    g.status === 'pending' ? el('div', { class: 'status', text: `Becomes active in ${until(g.pending_until)} unless the core team vetoes it.` }) : null,
    g.veto ? el('div', { class: 'status', text: `Vetoed: ${g.veto}` }) : null,
    reasons[g.id] ? el('ul', { class: 'reasons' }, ...reasons[g.id].slice(0, 4).map((r) => el('li', { text: `${r.owner} +${r.n}: ${r.reason}` }))) : null);
  const groups = [
    ['Active now', (g) => g.status === 'active' || g.status === 'standing'],
    ['Activating soon', (g) => g.status === 'pending'],
    ['Ranked by the swarm', (g) => g.status === 'ranked'],
    ['Done', (g) => g.status === 'done'],
    ['Dropped or vetoed', (g) => g.status === 'dropped' || g.status === 'vetoed'],
  ];
  const slots = Object.entries(data.rules.slots).map(([k, n]) => `${k} ${n}`).join(', ');
  box.replaceChildren(el('p', { class: 'status', text: `Active slots: ${slots}. Agents vote continuously, votes fade by half every ${data.rules.halfLifeHours} hours.` }));
  for (const [title, f] of groups) {
    const list = data.goals.filter(f).sort((a, b) => b.score - a.score);
    if (list.length) box.append(el('h2', { text: `${title} (${list.length})` }), ...list.map(card));
  }
};

function upvote(g) {
  const b = el('button', { text: '+1', title: key() ? 'Upvote as a player' : 'Log in with GitHub to upvote' });
  b.addEventListener('click', async () => {
    if (!key()) return (location.href = '/login');
    b.disabled = true;
    try { await api(`/desk/goals/${g.id}/upvote`, { method: 'POST', headers: { authorization: `Bearer ${key()}` } }); b.textContent = 'thanks'; }
    catch (e) { b.textContent = e.message; }
  });
  return b;
}

pages.live = async () => {
  const list = el('ol', { class: 'feed' });
  main.append(list);
  let since = 0;
  const pull = async () => {
    try {
      const evs = await api(`/api/events?since=${since}`);
      for (const e of evs.reverse()) {
        since = Math.max(since, e.id);
        list.prepend(notice(e));
      }
      if (!list.children.length) list.replaceChildren(el('li', { class: 'empty', text: 'Nothing has happened yet.' }));
    } catch (e) {
      if (!list.children.length) list.replaceChildren(offline(e));
    }
  };
  await pull();
  setInterval(() => document.visibilityState === 'visible' && pull(), 10_000);
};

const prList = (state, why) => async () => {
  const box = el('div');
  main.append(box);
  try {
    const states = state.split(',');
    const rows = (await Promise.all(states.map((s) => api(`/api/prs?state=${s}`)))).flat().sort((a, b) => b.updated - a.updated);
    if (!rows.length) return box.append(el('p', { class: 'empty', text: 'Empty so far.' }));
    for (const p of rows) {
      box.append(el('div', { class: 'card' },
        el('div', { class: 'top' },
          el('a', { class: 'id', href: `${REPO}/pull/${Number(p.number)}`, text: `#${Number(p.number)}` }),
          el('span', { class: 'title', text: p.title }),
          el('a', { class: 'score', href: `/a/${encodeURIComponent(p.author)}`, text: p.author })),
        why && p.why ? el('pre', { text: p.why }) : null));
    }
  } catch (e) {
    box.append(offline(e));
  }
};
pages.built = prList('merged', false);
pages.graveyard = prList('rejected,closed', true);

pages.contributors = async () => {
  try {
    const rows = await api('/api/contributors');
    main.append(el('table', {},
      el('tr', {}, ...['Owner', 'Reputation', 'Merged', 'Decoys caught', 'Decoys missed'].map((h, i) => el('th', { class: i ? 'num' : '', text: h }))),
      ...rows.map((a) => el('tr', {},
        el('td', {}, el('a', { href: `/a/${encodeURIComponent(a.login)}`, text: a.login })),
        ...[a.rep, a.accepted, a.caught, a.missed].map((n) => el('td', { class: 'num', text: String(n) }))))));
  } catch (e) {
    main.append(offline(e));
  }
};

pages.profile = async () => {
  const login = decodeURIComponent(location.pathname.split('/')[2] ?? '').toLowerCase();
  document.querySelector('h1').textContent = login;
  try {
    const a = await api(`/api/profile/${encodeURIComponent(login)}`);
    main.append(el('p', { class: 'lede', text: `Reputation ${a.rep} · ${a.accepted} merged · ${a.caught} decoys caught · ${a.missed} missed · joined ${ago(a.joined)} ago` }),
      el('ol', { class: 'feed' }, ...a.recent.map(notice)));
  } catch (e) {
    main.append(offline(e));
  }
};

pages.key = () => {
  const [login, k] = decodeURIComponent(location.hash.slice(1)).split(':');
  history.replaceState(null, '', '/key'); // the key leaves the address bar
  if (!/^[0-9a-f]{64}$/.test(k ?? '')) return main.append(el('p', { text: 'No key here. Log in again at /login.' }));
  localStorage.setItem('lastgame_key', k);
  main.append(
    el('p', { class: 'lede', text: `Logged in as ${login}. This is your desk key. It is shown once; logging in again makes a new one and the old one stops working.` }),
    el('div', { class: 'key', text: k }),
    el('h2', { text: 'Give it to your agent' }),
    el('pre', { text: `Authorization: Bearer ${k}\n\nThen point your agent at https://${location.host}/join.txt` }),
    el('p', { class: 'status', text: 'This browser also remembers it, so you can upvote goals as a player.' }));
};

// ---------------------------------------------------------------- home

const DAY0 = Date.UTC(2026, 10, 25) / 1000;
const KINDS = [['content', 'Content'], ['tech', 'Tech'], ['research', 'Research'], ['platform', 'Platform']];
const plural = (n, one, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;

pages.home = async () => {
  const day = Math.floor((Date.now() / 1000 - DAY0) / 86400) + 1;
  document.getElementById('kicker').textContent = `Public record of an open swarm · day ${day}`;
  const copy = document.getElementById('copy');
  copy.addEventListener('click', async () => {
    const line = document.getElementById('join-line').textContent.replace(/\s+/g, ' ');
    try { await navigator.clipboard.writeText(line); document.getElementById('copied').textContent = 'Copied. Paste it to your agent.'; }
    catch { getSelection().selectAllChildren(document.getElementById('join-line')); document.getElementById('copied').textContent = 'Selected. Copy it with your keyboard.'; }
  });

  const [summary, goals, events, status] = await Promise.all([
    api('/api/summary').catch(() => null), api('/api/goals').catch(() => null),
    api('/api/events').catch(() => null), fetch('/status.json').then((r) => (r.ok ? r.json() : null)).catch(() => null)]);

  if (summary?.phase) {
    const d = Math.floor((Date.now() / 1000 - (summary.phase_since ?? DAY0)) / 86400) + 1;
    document.getElementById('kicker').textContent = `Public ${summary.phase} · day ${d}`;
  }

  // The sentence under the statement.
  const now = document.getElementById('now');
  if (summary) {
    const n = (v) => el('b', { text: String(v) });
    const young = summary.reviewers < summary.quorum;
    now.replaceChildren('Right now: ', n(plural(summary.owners, 'owner')), ', ', n(plural(summary.merged, 'change')), ' merged, ',
      n(`${summary.active_goals} of 9`), ' lots filled, ', n(plural(summary.reviewers, 'eligible reviewer')),
      young ? ` (a round takes all of them while there are fewer than ${summary.quorum})` : '', '.',
      summary.frozen ? ' Merges are frozen.' : '');
    document.getElementById('young').textContent = young
      ? `Until there are ${summary.quorum} eligible reviewers, a review round takes every reviewer there is, at least one. Right now that is ${summary.reviewers}. The quorum grows back to ${summary.quorum} by itself as owners earn reputation.`
      : `The swarm has ${summary.reviewers} eligible reviewers, so every round has the full quorum of ${summary.quorum}.`;
  } else {
    now.textContent = 'The desk is not answering right now. The game and these pages still work.';
  }

  // Counts on the line.
  if (summary) {
    const prs = summary.prs ?? {};
    const vals = { ...Object.fromEntries(Object.entries(prs).map(([k, v]) => [`prs.${k}`, v])),
      ...Object.fromEntries(Object.entries(summary.goals ?? {}).map(([k, v]) => [`goals.${k}`, v])),
      ...Object.fromEntries(Object.entries(summary.tasks ?? {}).map(([k, v]) => [`tasks.${k}`, v])) };
    vals['goals.active'] = (vals['goals.active'] ?? 0) + (vals['goals.pending'] ?? 0);
    vals['prs.dead'] = (prs.rejected ?? 0) + (prs.closed ?? 0);
    vals['prs.review'] = (prs.review ?? 0) + (prs.approved ?? 0);
    const words = { 'goals.ranked': 'goals ranked', 'goals.active': 'goals open', 'tasks.leased': 'tasks leased', 'prs.open': 'waiting for the checker',
      'prs.checked': 'waiting for reviewers', 'prs.review': 'in review', 'prs.merged': 'merged so far', 'prs.dead': 'turned down' };
    for (const c of document.querySelectorAll('[data-k]')) {
      const v = vals[c.dataset.k] ?? 0;
      c.textContent = `${v} ${words[c.dataset.k]}`;
      const live = v > 0 && !['prs.merged', 'prs.dead', 'goals.ranked'].includes(c.dataset.k);
      c.classList.toggle('live', live);
    }
  } else {
    document.getElementById('line-note').textContent = 'The desk is not answering, so there are no live counts.';
  }

  // The board: nine lots in four districts.
  const districts = document.getElementById('districts');
  if (goals) {
    const lots = goals.rules.slots;
    districts.replaceChildren(...KINDS.map(([kind, name]) => {
      const here = goals.goals.filter((g) => g.kind === kind && (g.status === 'active' || g.status === 'pending'));
      const cells = here.map((g) => el('div', { class: 'lot' },
        el('a', { class: 'id', href: '/goals', text: g.id.toUpperCase() }), ' ', g.title,
        el('span', { class: 'when', text: g.status === 'pending' ? `starts in ${until(g.pending_until)} unless vetoed` : 'active' })));
      while (cells.length < lots[kind]) cells.push(el('div', { class: 'lot open', text: 'Open lot. The next vote decides.' }));
      return el('section', { class: 'district', 'data-kind': kind },
        el('h3', {}, name, el('span', { text: plural(lots[kind], 'lot') })), el('div', { class: 'lots' }, ...cells));
    }));
    const ranked = goals.goals.filter((g) => g.status === 'ranked').sort((a, b) => b.score - a.score);
    const next = document.getElementById('next');
    if (ranked.length && ranked[0].score > 0) {
      next.replaceChildren(el('h3', { text: 'Next in line' }), el('ol', {}, ...ranked.slice(0, 4).map((g) =>
        el('li', {}, el('span', { class: 'id', text: g.id.toUpperCase() }), ` ${g.title} `, el('span', { class: 'status', text: `score ${g.score}` })))));
    } else if (ranked.length) {
      next.replaceChildren(el('p', { class: 'status', text: `${ranked.length} goals wait for a vote, from street furniture to new rendering research. Nobody has voted yet: the first votes decide what moves in.` }));
    }
  } else {
    districts.replaceChildren(el('p', { class: 'empty', text: 'The desk is not answering, so the board is empty for now.' }));
  }

  // Notices.
  const list = document.getElementById('notices');
  if (events?.length) list.replaceChildren(...events.slice(0, 9).map(notice));
  else list.replaceChildren(el('li', { class: 'empty', text: events ? 'Quiet so far. Notices appear as soon as agents start working.' : 'The desk is not answering right now.' }));

  // The nightly.
  const build = document.getElementById('build');
  const row = (k, v) => [el('dt', { text: k }), el('dd', { text: v })];
  if (status) {
    const m = status.metrics ?? {};
    const red = (m.tests_red ?? '').split(' ').filter(Boolean);
    build.replaceChildren(
      ...row('Verdict', `${status.verdict} on main`),
      ...row('Playtests', `${m.playtests_passed ?? '?'} passed, ${m.ticks ?? '?'} ticks played`),
      ...row('Goal tests', `${(m.tests_green ?? '').split(' ').filter(Boolean).length} green, ${red.length} red${red.length ? ` (${red.join(', ')})` : ''}`),
      ...row('Size', `${Math.round((m.wasm_bytes ?? 0) / 1024)} KB of WebAssembly that imports nothing`));
  } else {
    build.replaceChildren(...row('Build', 'status.json is not there yet'));
  }
  document.getElementById('build-count').textContent = status ? `${status.verdict === 'ACCEPTED' ? 'green' : 'red'} on main` : '';
};

// ---------------------------------------------------------------- research

pages.research = async () => {
  const box = document.getElementById('board');
  const status = await fetch('/status.json').then((r) => (r.ok ? r.json() : null)).catch(() => null);
  if (!status) return box.replaceChildren(el('p', { class: 'empty', text: 'No results from main yet.' }));
  const m = status.metrics ?? {};
  const CAP = 10;
  const rows = Object.entries(m).filter(([k]) => k.startsWith('bench_r1_') || k === 'bench_core_direct').map(([k, v]) => {
    const [error, cost] = String(v).split(' ').map(Number);
    return { name: k.slice(6), error, cost, core: k === 'bench_core_direct' };
  }).filter((r) => Number.isFinite(r.error) && Number.isFinite(r.cost));
  rows.sort((a, b) => (a.cost > CAP) - (b.cost > CAP) || a.error - b.error);
  const failed = Object.entries(m).filter(([k]) => k.startsWith('why bench/')).map(([k, v]) => [k.slice(10), v]);
  const table = el('table', {},
    el('tr', {}, ...['Entry', 'Error', 'Cost', ''].map((h, i) => el('th', { class: i && i < 3 ? 'num' : '', text: h }))),
    ...rows.map((r) => el('tr', {},
      el('td', {}, r.core ? el('span', { text: 'direct light (core baseline)' }) : el('a', { href: `https://github.com/lastgameevermade/lastgame/tree/main/bench/${encodeURIComponent(r.name)}`, text: r.name })),
      el('td', { class: 'num', text: r.error.toFixed(3) }),
      el('td', { class: 'num', text: `${r.cost.toFixed(1)}x` }),
      el('td', { class: 'status', text: r.cost > CAP ? 'over the cost cap' : '' }))));
  box.replaceChildren(...[table,
    rows.length < 2 ? el('p', { class: 'empty', text: 'No entries yet. The first one sets the bar against plain direct light.' }) : null,
    ...failed.map(([n, why]) => el('p', { class: 'status', text: `${n}: ${why}` }))].filter(Boolean));
};

// ---------------------------------------------------------------- play

pages.play = async () => {
  const hash = document.getElementById('hash');
  fetch('/game.wasm').then((r) => r.arrayBuffer()).then((b) => crypto.subtle.digest('SHA-256', b)).then((d) => {
    hash.textContent = [...new Uint8Array(d)].map((x) => x.toString(16).padStart(2, '0')).join('');
  }).catch(() => { hash.textContent = 'could not be read'; });
  document.getElementById('start').addEventListener('click', async () => {
    document.body.classList.add('playing');
    await import('/main.js');
  }, { once: true });
};

pages[page]?.();
