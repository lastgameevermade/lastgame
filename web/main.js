// Loads game.wasm with an EMPTY import object, so the game code cannot call
// anything in the browser, then draws what it describes: the scene with
// WebGL2 (instanced, one draw call per mesh), the HUD on a 2D canvas, and
// sound through an AudioWorklet. Protocol: see engine/src/lib.rs.

const canvas = document.getElementById('c');
const hudCanvas = document.getElementById('hud');
const fatal = document.getElementById('fatal');
const tickEl = document.getElementById('tick');
const die = (msg) => { fatal.textContent = msg; fatal.classList.add('on'); };

const gl = canvas.getContext('webgl2', { antialias: true });
if (!gl) throw die('This browser has no WebGL2, so the game cannot run here.');
const hud = hudCanvas.getContext('2d');

const bytes = await (await fetch('game.wasm')).arrayBuffer();
const { instance } = await WebAssembly.instantiate(bytes, {});
const x = instance.exports;
x.lg_init(1);

// ---------------------------------------------------------------- input

const KEYS = {
  KeyW: 1, ArrowUp: 1, KeyS: 2, ArrowDown: 2, KeyA: 4, ArrowLeft: 4, KeyD: 8, ArrowRight: 8,
  KeyE: 16, Enter: 16, ShiftLeft: 32, ShiftRight: 32, Space: 64,
};
const held = new Set();
let latch = 0; // a tap shorter than one tick still counts
addEventListener('keydown', (e) => {
  const bit = KEYS[e.code];
  if (!bit || e.metaKey || e.ctrlKey || e.altKey) return;
  e.preventDefault();
  held.add(e.code);
  latch |= bit;
  startAudio();
});
addEventListener('keyup', (e) => held.delete(e.code));
addEventListener('blur', () => held.clear());
const keys = () => {
  let b = latch;
  for (const k of held) b |= KEYS[k];
  latch = 0;
  return b;
};

// ---------------------------------------------------------------- sound

let audio = null;
async function startAudio() {
  if (audio) return;
  audio = 'starting'; // browsers only allow sound after a key press
  try {
    const ctx = new AudioContext({ sampleRate: 48000 });
    await ctx.audioWorklet.addModule('audio.js');
    const node = new AudioWorkletNode(ctx, 'lastgame-audio');
    node.connect(ctx.destination);
    audio = node;
  } catch {
    audio = 'off';
  }
}
function pumpAudio() {
  const n = x.lg_audio_len();
  if (n && audio instanceof AudioWorkletNode) {
    const chunk = new Float32Array(x.memory.buffer, x.lg_audio(), n).slice();
    if (chunk.some((v) => v !== 0)) audio.port.postMessage(chunk, [chunk.buffer]);
  }
  x.lg_audio_clear();
}

// ---------------------------------------------------------------- gl

const compile = (type, src) => {
  const s = gl.createShader(type);
  gl.shaderSource(s, src);
  gl.compileShader(s);
  if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw die(gl.getShaderInfoLog(s));
  return s;
};
const prog = gl.createProgram();
gl.attachShader(prog, compile(gl.VERTEX_SHADER, `#version 300 es
layout(location=0) in vec3 pos;
layout(location=1) in vec3 normal;
layout(location=2) in vec3 color;
layout(location=3) in mat4 model;
uniform mat4 vp;
out vec3 vN, vC, vW;
void main() {
  vec4 w = model * vec4(pos, 1.0);
  vW = w.xyz; vN = mat3(model) * normal; vC = color;
  gl_Position = vp * w;
}`));
gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, `#version 300 es
precision highp float;
in vec3 vN, vC, vW;
uniform vec3 eye, sunDir, sun, ambient, skyLight, sky;
uniform float fog;
out vec4 o;
void main() {
  vec3 n = normalize(vN);
  float lit = max(dot(n, normalize(sunDir)), 0.0);
  float up = 0.5 + 0.5 * n.y;
  vec3 c = vC * (ambient + skyLight * up + sun * lit);
  float f = 1.0 - exp(-pow(length(vW - eye) / max(fog, 1.0), 2.0));
  o = vec4(mix(c, sky, f), 1.0);
}`));
gl.linkProgram(prog);
if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) throw die(gl.getProgramInfoLog(prog));
gl.useProgram(prog);
const U = Object.fromEntries(['vp', 'eye', 'sunDir', 'sun', 'ambient', 'skyLight', 'sky', 'fog'].map((n) => [n, gl.getUniformLocation(prog, n)]));
gl.enable(gl.DEPTH_TEST);
gl.enable(gl.CULL_FACE);
const instances = gl.createBuffer();

const meshes = [];
function syncMeshes() {
  // Meshes are append-only. Views are made fresh because memory growth detaches old ones.
  for (let i = meshes.length, n = x.lg_mesh_count(); i < n; i++) {
    const vao = gl.createVertexArray();
    gl.bindVertexArray(vao);
    gl.bindBuffer(gl.ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array(x.memory.buffer, x.lg_mesh_vertices(i), x.lg_mesh_vertices_len(i)), gl.STATIC_DRAW);
    for (let a = 0; a < 3; a++) {
      gl.enableVertexAttribArray(a);
      gl.vertexAttribPointer(a, 3, gl.FLOAT, false, 36, a * 12);
    }
    const count = x.lg_mesh_indices_len(i);
    gl.bindBuffer(gl.ELEMENT_ARRAY_BUFFER, gl.createBuffer());
    gl.bufferData(gl.ELEMENT_ARRAY_BUFFER, new Uint32Array(x.memory.buffer, x.lg_mesh_indices(i), count), gl.STATIC_DRAW);
    gl.bindBuffer(gl.ARRAY_BUFFER, instances);
    for (let c = 0; c < 4; c++) {
      gl.enableVertexAttribArray(3 + c);
      gl.vertexAttribDivisor(3 + c, 1);
    }
    meshes.push({ vao, count });
  }
  gl.bindVertexArray(null);
}

// Column-major matrices, same as the engine.
function perspective(fovy, aspect, near, far) {
  const f = 1 / Math.tan(fovy / 2), nf = 1 / (near - far);
  return [f / aspect, 0, 0, 0, 0, f, 0, 0, 0, 0, (far + near) * nf, -1, 0, 0, 2 * far * near * nf, 0];
}
function lookAt(e, t) {
  const sub = (a, b) => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
  const norm = (v) => { const l = Math.hypot(...v) || 1; return v.map((c) => c / l); };
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

const rgba = (r, g, b, a) => `rgba(${[r, g, b].map((v) => Math.round(Math.min(Math.max(v, 0), 1) * 255)).join(',')},${Math.min(Math.max(a, 0), 1)})`;

function render() {
  const dpr = Math.min(devicePixelRatio || 1, 2);
  const w = Math.round(canvas.clientWidth * dpr), h = Math.round(canvas.clientHeight * dpr);
  for (const c of [canvas, hudCanvas]) if (c.width !== w || c.height !== h) { c.width = w; c.height = h; }

  const f = new Float32Array(x.memory.buffer, x.lg_frame(), x.lg_frame_len());
  const eye = [f[0], f[1], f[2]];
  let at = 7;
  const take = (n) => { const v = f.subarray(at, at + n); at += n; return v; };
  gl.uniform3fv(U.sunDir, take(3));
  gl.uniform3fv(U.sun, take(3));
  gl.uniform3fv(U.ambient, take(3));
  gl.uniform3fv(U.skyLight, take(3));
  const sky = take(3);
  gl.uniform3fv(U.sky, sky);
  gl.uniform1f(U.fog, take(1)[0]);

  gl.viewport(0, 0, w, h);
  gl.clearColor(sky[0], sky[1], sky[2], 1);
  gl.clear(gl.COLOR_BUFFER_BIT | gl.DEPTH_BUFFER_BIT);
  gl.uniformMatrix4fv(U.vp, false, mul(perspective(f[6], w / h, 0.1, 1000), lookAt(eye, [f[3], f[4], f[5]])));
  gl.uniform3fv(U.eye, eye);

  // Group draws by mesh: one instanced draw call per mesh.
  const draws = take(1)[0];
  const groups = new Map();
  for (let i = 0; i < draws; i++) {
    const d = take(17);
    (groups.get(d[0]) ?? groups.set(d[0], []).get(d[0])).push(d.subarray(1));
  }
  const data = new Float32Array(draws * 16);
  let n = 0;
  const spans = [];
  for (const [mesh, list] of groups) {
    spans.push([mesh, n, list.length]);
    for (const m of list) data.set(m, (n++) * 16);
  }
  gl.bindBuffer(gl.ARRAY_BUFFER, instances);
  gl.bufferData(gl.ARRAY_BUFFER, data, gl.STREAM_DRAW);
  for (const [mesh, first, count] of spans) {
    const m = meshes[mesh];
    gl.bindVertexArray(m.vao);
    for (let c = 0; c < 4; c++) gl.vertexAttribPointer(3 + c, 4, gl.FLOAT, false, 64, first * 64 + c * 16);
    gl.drawElementsInstanced(gl.TRIANGLES, m.count, gl.UNSIGNED_INT, 0, count);
  }
  gl.bindVertexArray(null);

  // HUD: rectangles and plain ASCII text, positions as fractions of the screen.
  hud.clearRect(0, 0, w, h);
  const items = take(1)[0];
  for (let i = 0; i < items; i++) {
    const [kind, hx, hy, a, b, r, g, bl, al] = take(9);
    hud.fillStyle = rgba(r, g, bl, al);
    if (kind === 0) {
      hud.fillRect(hx * w, hy * h, a * w, b * h);
    } else {
      const text = String.fromCharCode(...take(b).filter((c) => c >= 32 && c < 127));
      hud.font = `${Math.max(1, Math.round(a * h))}px ui-monospace, monospace`;
      hud.textBaseline = 'top';
      hud.fillText(text, hx * w, hy * h);
    }
  }
}

// ---------------------------------------------------------------- loop

const STEP = 1000 / 60;
let acc = 0, last = performance.now();
function frame(now) {
  acc += Math.min(now - last, 250);
  last = now;
  try {
    for (let n = 0; acc >= STEP; n++) {
      if (n === 8) { acc = 0; break; } // too slow to catch up: drop time, not frames
      x.lg_step(keys());
      acc -= STEP;
    }
    pumpAudio();
    syncMeshes();
    render();
  } catch (e) {
    return die(`The game crashed at tick ${x.lg_tick()}. This build is broken; the next accepted change can fix it. (${e.message})`);
  }
  tickEl.textContent = `tick ${x.lg_tick()}`;
  requestAnimationFrame(frame);
}
canvas.focus();
requestAnimationFrame(frame);

// Live counters from the desk. The game works without them.
fetch('/api/summary').then((r) => (r.ok ? r.json() : null)).then((s) => {
  if (s) document.getElementById('counts').textContent =
    `${s.merged} ${s.merged === 1 ? 'change' : 'changes'} merged · ${s.owners} owners · ${s.active_goals} goals active · ${s.done_goals} done${s.frozen ? ' · merges frozen' : ''}`;
}).catch(() => {});
