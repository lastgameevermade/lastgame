// Shots: what a change does to the picture. Runs two builds of game.wasm, each
// in a fresh instance with an empty import object: seed 1, TICKS ticks with no
// input. Then it draws the frame on the CPU the way web/main.js does (same
// camera math, light, fog and sky; no HUD, no antialiasing) from a few fixed
// views, writes <view>-before/after/diff.png and prints METRIC visual_change
// <percent of pixels that changed, over all views>, then DONE.
//   node checker/shots.mjs <before.wasm> <after.wasm> <out dir>
import { mkdirSync, readFileSync, writeFileSync } from 'node:fs';
import { crc32, deflateSync } from 'node:zlib';

const W = 320, H = 180, TICKS = 120;
const THRESHOLD = 8; // a pixel changed if any channel moved by more than this (of 255)
// `game` is the game's own camera at that tick, what a player sees first. The
// others are fixed in the world, around the spawn: the player starts at
// (-7, 0, 10) (character::SPAWN), next to the car at (-4, 0, 10) (car::SPAWN),
// which stays put until someone drives it.
const VIEWS = {
  game: null,
  street: { eye: [-2.5, 1.7, -8], target: [-2.5, 1.6, 40], fov: 1.05 },
  aerial: { eye: [25, 55, -25], target: [-25, 0, 25], fov: 1.05 },
  car: { eye: [0, 2.2, 17], target: [-4, 0.6, 10], fov: 0.8 },
};

const [beforeFile, afterFile, out] = process.argv.slice(2);
const t0 = performance.now();

async function scene(file) {
  const x = (await WebAssembly.instantiate(readFileSync(file), {})).instance.exports;
  x.lg_init(1);
  for (let i = 0; i < TICKS; i++) x.lg_step(0);
  const ptr = x.lg_frame(), len = x.lg_frame_len(); // may grow memory, so the views come after
  const f = new Float32Array(x.memory.buffer, ptr, len);
  const meshes = [];
  for (let i = 0, n = x.lg_mesh_count(); i < n; i++) {
    meshes.push({
      v: new Float32Array(x.memory.buffer, x.lg_mesh_vertices(i), x.lg_mesh_vertices_len(i)),
      ix: new Uint32Array(x.memory.buffer, x.lg_mesh_indices(i), x.lg_mesh_indices_len(i)),
    });
  }
  let at = 0;
  const take = (n) => f.subarray(at, (at += n));
  const camera = { eye: take(3), target: take(3), fov: take(1)[0] };
  const light = { sunDir: norm(take(3)), sun: take(3), ambient: take(3), skyLight: take(3), sky: take(3), fog: Math.max(take(1)[0], 1) };
  const draws = [];
  for (let i = 0, n = take(1)[0]; i < n; i++) {
    const d = take(17);
    if (meshes[d[0]]) draws.push({ mesh: meshes[d[0]], m: d.subarray(1) });
  }
  return { camera, light, draws };
}

// Column-major matrices, as in web/main.js.
function perspective(fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0];
}
function norm(v) {
  const l = Math.hypot(...v) || 1;
  return v.map((c) => c / l);
}
function lookAt(e, t) {
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
  const dot = (a, b) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
  const z = norm(sub(e, t)), xa = norm(cross([0, 1, 0], z)), y = cross(z, xa);
  return [xa[0], y[0], z[0], 0, xa[1], y[1], z[1], 0, xa[2], y[2], z[2], 0, -dot(xa, e), -dot(y, e), -dot(z, e), 1];
}
function mul(a, b) {
  const o = new Array(16).fill(0);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) for (let k = 0; k < 4; k++) o[c * 4 + r] += a[k * 4 + r] * b[c * 4 + k];
  return o;
}

// A transformed vertex: clip xyzw, then world xyz, normal xyz, colour rgb.
const V = 13;

function render(s, view) {
  const { eye, target, fov } = view ?? s.camera;
  const { sunDir, sun, ambient, skyLight, sky, fog } = s.light;
  const vp = mul(perspective(fov, W / H, 0.1, 1000), lookAt(eye, target));
  const px = Buffer.alloc(W * H * 4);
  const depth = new Float64Array(W * H).fill(1); // cleared to the far plane, as gl.clear does
  const bg = [...sky].map(to8).concat(255);
  for (let i = 0; i < W * H; i++) px.set(bg, i * 4);

  // One fragment, the fragment shader of web/main.js.
  const shade = (i, a) => {
    const n = norm([a[3], a[4], a[5]]);
    const lit = Math.max(n[0] * sunDir[0] + n[1] * sunDir[1] + n[2] * sunDir[2], 0);
    const up = 0.5 + 0.5 * n[1];
    const f = 1 - Math.exp(-((Math.hypot(a[0] - eye[0], a[1] - eye[1], a[2] - eye[2]) / fog) ** 2));
    for (let c = 0; c < 3; c++) px[i * 4 + c] = to8(a[6 + c] * (ambient[c] + skyLight[c] * up + sun[c] * lit) * (1 - f) + sky[c] * f);
  };

  // Screen space: x right, y down, z as gl's depth, 1/w for perspective correct attributes.
  const project = (v) => {
    const iw = 1 / v[3];
    return [(v[0] * iw + 1) * 0.5 * W, (1 - v[1] * iw) * 0.5 * H, v[2] * iw, iw, ...v.subarray(4)];
  };
  const edge = (a, b, x, y) => (b[0] - a[0]) * (y - a[1]) - (b[1] - a[1]) * (x - a[0]);
  const attr = new Float64Array(9);
  const raster = (A, B, C) => {
    const area = edge(A, B, C[0], C[1]);
    if (!(area < 0)) return; // counter-clockwise on a y-down screen is a back face (gl.CULL_FACE), or degenerate
    const x0 = Math.max(0, Math.floor(Math.min(A[0], B[0], C[0])));
    const x1 = Math.min(W - 1, Math.ceil(Math.max(A[0], B[0], C[0])));
    const y0 = Math.max(0, Math.floor(Math.min(A[1], B[1], C[1])));
    const y1 = Math.min(H - 1, Math.ceil(Math.max(A[1], B[1], C[1])));
    for (let y = y0; y <= y1; y++) {
      for (let x = x0; x <= x1; x++) {
        const l0 = edge(B, C, x + 0.5, y + 0.5) / area, l1 = edge(C, A, x + 0.5, y + 0.5) / area, l2 = 1 - l0 - l1;
        if (l0 < 0 || l1 < 0 || l2 < 0) continue;
        const z = l0 * A[2] + l1 * B[2] + l2 * C[2], i = y * W + x;
        if (!(z < depth[i])) continue;
        depth[i] = z;
        const q0 = l0 * A[3], q1 = l1 * B[3], q2 = l2 * C[3], q = q0 + q1 + q2;
        for (let k = 0; k < 9; k++) attr[k] = (q0 * A[4 + k] + q1 * B[4 + k] + q2 * C[4 + k]) / q;
        shade(i, attr);
      }
    }
  };
  // Clips a triangle against the near plane (z >= -w), the only plane that needs real clipping.
  const lerp = (a, b, t) => a.map((v, k) => v + (b[k] - v) * t);
  const triangle = (a, b, c) => {
    const poly = [];
    for (const [p, q] of [[a, b], [b, c], [c, a]]) {
      const dp = p[2] + p[3], dq = q[2] + q[3];
      if (dp >= 0) poly.push(p);
      if (dp >= 0 !== dq >= 0) poly.push(lerp(p, q, dp / (dp - dq)));
    }
    if (poly.length < 3) return;
    const s = poly.map(project);
    for (let k = 1; k + 1 < s.length; k++) raster(s[0], s[k], s[k + 1]);
  };

  for (const { mesh: { v, ix }, m } of s.draws) {
    const n = Math.floor(v.length / 9), t = new Float64Array(n * V);
    for (let i = 0; i < n; i++) {
      const [x, y, z, nx, ny, nz] = v.subarray(i * 9, i * 9 + 6);
      const w = [0, 1, 2, 3].map((r) => m[r] * x + m[4 + r] * y + m[8 + r] * z + m[12 + r]);
      const o = i * V;
      for (let r = 0; r < 4; r++) t[o + r] = vp[r] * w[0] + vp[4 + r] * w[1] + vp[8 + r] * w[2] + vp[12 + r] * w[3];
      for (let r = 0; r < 3; r++) {
        t[o + 4 + r] = w[r];
        t[o + 7 + r] = m[r] * nx + m[4 + r] * ny + m[8 + r] * nz; // mat3(model) * normal
        t[o + 10 + r] = v[i * 9 + 6 + r];
      }
    }
    const at = (k) => t.subarray(ix[k] * V, ix[k] * V + V);
    for (let k = 0; k + 2 < ix.length; k += 3) {
      if (ix[k] < n && ix[k + 1] < n && ix[k + 2] < n) triangle(at(k), at(k + 1), at(k + 2));
    }
  }
  return px;
}

function to8(v) {
  return Math.round(Math.min(Math.max(v, 0), 1) * 255);
}

// RGBA to PNG: no filter, one IDAT.
function png(px) {
  const chunk = (type, data) => {
    const b = Buffer.alloc(12 + data.length);
    b.writeUInt32BE(data.length, 0);
    b.write(type, 4, 'latin1');
    data.copy(b, 8);
    b.writeUInt32BE(crc32(b.subarray(4, 8 + data.length)), 8 + data.length);
    return b;
  };
  const head = Buffer.alloc(13);
  head.writeUInt32BE(W, 0);
  head.writeUInt32BE(H, 4);
  head.set([8, 6], 8); // 8 bits per channel, RGBA
  const raw = Buffer.alloc((W * 4 + 1) * H);
  for (let y = 0; y < H; y++) px.copy(raw, y * (W * 4 + 1) + 1, y * W * 4, (y + 1) * W * 4);
  const sig = Buffer.from([137, 80, 78, 71, 13, 10, 26, 10]);
  return Buffer.concat([sig, chunk('IHDR', head), chunk('IDAT', deflateSync(raw)), chunk('IEND', Buffer.alloc(0))]);
}

const [before, after, again] = [await scene(beforeFile), await scene(afterFile), await scene(afterFile)];
mkdirSync(out, { recursive: true });
let changed = 0;
for (const [name, view] of Object.entries(VIEWS)) {
  const [b, a] = [render(before, view), render(after, view)];
  if (!render(again, view).equals(a)) throw new Error(`the ${name} shot differs between two runs of the same build`);
  const diff = Buffer.alloc(W * H * 4);
  for (let i = 0; i < W * H * 4; i += 4) {
    const hit = [0, 1, 2].some((c) => Math.abs(a[i + c] - b[i + c]) > THRESHOLD);
    changed += hit;
    diff.set(hit ? [255, 0, 255, 255] : [a[i] >> 1, a[i + 1] >> 1, a[i + 2] >> 1, 255], i);
  }
  writeFileSync(`${out}/${name}-before.png`, png(b));
  writeFileSync(`${out}/${name}-after.png`, png(a));
  writeFileSync(`${out}/${name}-diff.png`, png(diff));
}
const share = (100 * changed) / (W * H * Object.keys(VIEWS).length);
// ponytail: any changed pixel shows as at least 0.1, so 0.0 always means the picture did not change.
console.log(`METRIC visual_change ${(changed ? Math.max(share, 0.1) : 0).toFixed(1)}`);
console.log(`METRIC shots_ms ${(performance.now() - t0).toFixed(0)}`);
console.log('DONE');
