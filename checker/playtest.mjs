// Runs every playtest inside game.wasm with an empty import object, each in a
// fresh instance: the code under test cannot reach node, files or the network,
// and cannot carry anything from one scenario into the next.
// Prints PASS/FAIL per scenario, aggregated METRIC lines, then DONE.
//   node checker/playtest.mjs game.wasm <max ms per tick> <max s per swarm test>
import { readFileSync } from 'node:fs';

const [file, msPerTick, testSeconds] = process.argv.slice(2);
const module = await WebAssembly.compile(readFileSync(file));
const fresh = async () => (await WebAssembly.instantiate(module, {})).exports;
const text = (x, ptr, len) => Buffer.from(x.memory.buffer, ptr, len).toString('utf8');

const max = {}, budget = {};
let ticks = 0, coreMs = 0, memory = 0, crashed = false;
const perTick = {}; // core scenario name -> ms per tick
const t0 = performance.now();
const count = (await fresh()).pt_count();

for (let i = 0; i < count; i++) {
  const x = await fresh();
  const name = text(x, x.pt_name(i), x.pt_name_len(i));
  const core = x.pt_is_core(i) === 1;
  const start = performance.now();
  try {
    x.pt_run(i);
  } catch (e) {
    // A trap (panic, overflow, stack exhaustion) ends this instance.
    console.log(`FAIL ${name}: crashed (${e.message})`);
    if (core) { crashed = true; break; }
    continue;
  }
  const ms = performance.now() - start;
  memory = Math.max(memory, x.memory.buffer.byteLength);
  for (const line of text(x, x.pt_report(), x.pt_report_len()).split('\n')) {
    const m = line.match(/^METRIC (\S+) (\S+)(?: (\S+))?$/);
    if (!m) { if (line) console.log(line); continue; }
    const [, k, v, b] = m;
    if (k === 'ticks') ticks += Number(v);
    else max[k] = Math.max(max[k] ?? 0, Number(v));
    if (b) budget[k] = b;
  }
  if (!core && ms > testSeconds * 1000) {
    console.log(`FAIL ${name}: took ${(ms / 1000).toFixed(1)} s, a test may take at most ${testSeconds} s`);
  }
  if (core && x.pt_ticks() > 0) {
    coreMs += ms;
    perTick[name] = ms / x.pt_ticks();
    // Stop a CPU hog after the first few hundred ticks instead of waiting it out.
    if (ticks >= 600 && coreMs / ticks > msPerTick) {
      console.log(`FAIL core/cpu: simulation costs ${(coreMs / ticks).toFixed(3)} ms per tick, budget is ${msPerTick} ms`);
      break;
    }
  }
}

// Cost that grows with play time: five minutes may not cost twice as much per tick as one.
const [short, long] = [perTick['core/growth_short'], perTick['core/growth_long']];
if (short !== undefined && long !== undefined && long > 0.05 && long > 2 * short) {
  console.log(`FAIL core/growth: a tick costs ${long.toFixed(3)} ms over five minutes against ${short.toFixed(3)} ms over one: the game slows down the longer it runs`);
}
console.log(`METRIC ticks ${ticks}`);
for (const k of Object.keys(max)) console.log(`METRIC ${k} ${max[k]}${budget[k] ? ' ' + budget[k] : ''}`);
console.log(`METRIC ms_per_tick ${(coreMs / Math.max(ticks, 1)).toFixed(4)} ${msPerTick}`);
console.log(`METRIC memory_bytes ${memory}`);
console.log(`METRIC run_ms ${(performance.now() - t0).toFixed(0)}`);
if (!crashed) console.log('DONE');
