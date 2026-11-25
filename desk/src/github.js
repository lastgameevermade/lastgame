// GitHub plumbing for the desk: webhook signatures, the App's installation
// token, and a small API helper. The App needs: pull requests (write),
// contents (write, for merging), issues (write, for comments), metadata (read).
// It must NOT get commit statuses: only the trusted verdict workflow sets those.

const enc = new TextEncoder();

const hex = (buf) => [...new Uint8Array(buf)].map((b) => b.toString(16).padStart(2, '0')).join('');

export async function sha256(text) {
  return hex(await crypto.subtle.digest('SHA-256', enc.encode(text)));
}

export async function hmac(secret, text) {
  const key = await crypto.subtle.importKey('raw', enc.encode(secret), { name: 'HMAC', hash: 'SHA-256' }, false, ['sign']);
  return hex(await crypto.subtle.sign('HMAC', key, enc.encode(text)));
}

/** Constant-time comparison of two hex strings. */
export function same(a, b) {
  if (typeof a !== 'string' || typeof b !== 'string' || a.length !== b.length) return false;
  let d = 0;
  for (let i = 0; i < a.length; i++) d |= a.charCodeAt(i) ^ b.charCodeAt(i);
  return d === 0;
}

/** X-Hub-Signature-256 check for webhook deliveries. */
export async function verifyWebhook(secret, body, header) {
  if (!secret || !header?.startsWith('sha256=')) return false;
  return same(header.slice(7), await hmac(secret, body));
}

const b64url = (bytes) => btoa(String.fromCharCode(...new Uint8Array(bytes))).replace(/\+/g, '-').replace(/\//g, '_').replace(/=+$/, '');

/** A JWT for the GitHub App. GITHUB_APP_KEY is the PKCS#8 PEM (see desk/README.md). */
async function appJwt(env, now) {
  const der = Uint8Array.from(atob(env.GITHUB_APP_KEY.replace(/-----[^-]+-----|\s/g, '')), (c) => c.charCodeAt(0));
  const key = await crypto.subtle.importKey('pkcs8', der, { name: 'RSASSA-PKCS1-v1_5', hash: 'SHA-256' }, false, ['sign']);
  const part = (o) => b64url(enc.encode(JSON.stringify(o)));
  const body = `${part({ alg: 'RS256', typ: 'JWT' })}.${part({ iat: now - 60, exp: now + 540, iss: env.GITHUB_APP_ID })}`;
  return `${body}.${b64url(await crypto.subtle.sign('RSASSA-PKCS1-v1_5', key, enc.encode(body)))}`;
}

let cached = { token: '', until: 0 };

async function installationToken(env, now) {
  if (cached.until > now + 60) return cached.token;
  const r = await fetch(`https://api.github.com/app/installations/${env.GITHUB_INSTALLATION_ID}/access_tokens`, {
    method: 'POST',
    headers: { authorization: `Bearer ${await appJwt(env, now)}`, accept: 'application/vnd.github+json', 'user-agent': 'lastgame-desk' },
  });
  if (!r.ok) throw new Error(`installation token: ${r.status}`);
  const j = await r.json();
  cached = { token: j.token, until: Math.floor(Date.parse(j.expires_at) / 1000) };
  return cached.token;
}

/** Calls the GitHub REST API as the App. Returns { ok, status, data }. */
export async function gh(env, method, path, body) {
  const now = Math.floor(Date.now() / 1000);
  const r = await fetch(`https://api.github.com${path}`, {
    method,
    headers: {
      authorization: `Bearer ${await installationToken(env, now)}`,
      accept: 'application/vnd.github+json',
      'user-agent': 'lastgame-desk',
      ...(body ? { 'content-type': 'application/json' } : {}),
    },
    body: body ? JSON.stringify(body) : undefined,
  });
  const text = await r.text();
  let data = null;
  try { data = JSON.parse(text); } catch { data = text; }
  return { ok: r.ok, status: r.status, data };
}
