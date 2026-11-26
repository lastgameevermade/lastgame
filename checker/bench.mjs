// Runs the research benchmarks inside bench.wasm with an empty import object.
// For each street scene: the path-traced reference, the core baseline (direct
// light, the unit of cost), then each entry: its error against the reference
// and its cost in baselines. Entries see the scene and an image, nothing else.
// Prints BENCH <entry> <error> <cost> or FAIL bench/<entry>: <why>, then DONE.
//   node checker/bench.mjs bench.wasm <entries, comma separated, or *> <max ms per render>
import { readFileSync } from 'node:fs';

const [file, only, max] = process.argv.slice(2);
const maxMs = Number(max);
const SCENES = [1, 2, 3];
const module = await WebAssembly.compile(readFileSync(file));
const fresh = async () => (await WebAssembly.instantiate(module, {})).exports;
let x = await fresh();
const count = x.bn_count();
const name = (i) => Buffer.from(x.memory.buffer, x.bn_name(i), x.bn_name_len(i)).toString('utf8');
const entries = Array.from({ length: count }, (_, i) => ({ i, name: name(i), errors: [], costs: [], failed: '' }));
const picked = only === '*' ? entries : entries.filter((e) => only.split(',').includes(e.name));

// Mean time of one render. Cheap ones repeat until 50 ms have passed, so timer noise drops out.
const time = (f) => {
  const t0 = performance.now();
  let n = 0, t = 0;
  do { f(); n++; t = performance.now() - t0; } while (t < 50 && n < 200);
  return t / n;
};

const t0 = performance.now();
const base = [];
for (const seed of SCENES) {
  x.bn_scene(seed);
  x.bn_reference();
  const unit = time(() => x.bn_render(count));
  base.push(x.bn_error());
  for (const e of picked) {
    if (e.failed) continue;
    let ms, first, last, err;
    try {
      x.bn_render(e.i);
      first = x.bn_hash();
      ms = time(() => x.bn_render(e.i));
      last = x.bn_hash();
      err = x.bn_error();
    } catch (ex) {
      // A trap (panic, overflow, out of memory) ends this instance.
      e.failed = `crashed (${ex.message})`;
      x = await fresh();
      x.bn_scene(seed);
      x.bn_reference();
      continue;
    }
    if (ms > maxMs) e.failed = `takes ${ms.toFixed(0)} ms per render, at most ${maxMs} ms`;
    else if (first !== last) e.failed = 'renders a different image each time: entries must be deterministic';
    else if (!Number.isFinite(err)) e.failed = 'the image has the wrong size, or values that are not finite and non-negative';
    else { e.errors.push(err); e.costs.push(ms / unit); }
  }
}

const mean = (a) => a.reduce((s, v) => s + v, 0) / a.length;
console.log(`BENCH core_direct ${mean(base).toFixed(4)} 1.00`);
for (const e of picked) {
  if (e.failed) console.log(`FAIL bench/${e.name}: ${e.failed}`);
  else console.log(`BENCH ${e.name} ${mean(e.errors).toFixed(4)} ${mean(e.costs).toFixed(2)}`);
}
console.log(`METRIC bench_ms ${(performance.now() - t0).toFixed(0)}`);
console.log('DONE');
