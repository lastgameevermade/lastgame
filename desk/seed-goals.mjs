// Turns the seed backlog table in GOALS.md into the JSON body for /admin/goals.
//   node desk/seed-goals.mjs > goals.json
//   curl -X POST -H "Authorization: Bearer $ADMIN_KEY" --data @goals.json https://lastgameevermade.com/admin/goals
import { readFileSync } from 'node:fs';

const KIND = { content: 'content', tech: 'tech', research: 'research', platform: 'platform' };

export function seedGoals(markdown) {
  const goals = [];
  for (const line of markdown.split('\n')) {
    const cells = line.split('|').slice(1, -1).map((c) => c.trim());
    if (cells.length !== 5 || !/^[CTRP][0-9]{1,3}$/.test(cells[0]) || !KIND[cells[1]]) continue;
    const [id, kind, horizon, title, done] = cells;
    goals.push({
      id: id.toLowerCase(),
      kind,
      horizon: ['S', 'M', 'L'].includes(horizon) ? horizon : '-',
      title: title.replace(/\s*\(core\)$/, ''),
      done_when: done,
      status: id === 'C0' ? 'standing' : 'ranked',
    });
  }
  return goals;
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const goals = seedGoals(readFileSync(new URL('../GOALS.md', import.meta.url), 'utf8'));
  process.stdout.write(JSON.stringify({ goals }) + '\n');
}
