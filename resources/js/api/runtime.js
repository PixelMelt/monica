// The runtime of the generated API client (client.js): every request the
// frontend makes goes through `request` (api-design.md §7.2 for CSRF, §6
// for errors).

export const BASE = '/api/v1';

/**
 * A failed request: the problem details of a non-2xx response. `errors` is
 * `{ field: [message, …] }`, and `response.data` is `{ message, errors }`,
 * the shape the pages read from Laravel's error responses.
 */
export class ApiError extends Error {
  constructor(status, problem) {
    super(problem?.detail ?? problem?.title ?? `HTTP ${status}`);
    this.status = status;
    this.code = problem?.code ?? 'internal_error';
    this.problem = problem ?? {};
    this.errors = {};
    for (const e of problem?.errors ?? []) {
      (this.errors[e.field] ??= []).push(e.message);
    }
    const data = { message: this.message };
    if (status === 422) data.errors = this.errors;
    this.response = { status, data };
  }
}

function cookie(name) {
  const match = document.cookie.split('; ').find((c) => c.startsWith(`${name}=`));
  return match ? decodeURIComponent(match.slice(name.length + 1)) : null;
}

export function buildUrl({ path, pathParams, queryParams }, params = {}) {
  let url = path;
  for (const name of pathParams) {
    if (params[name] === undefined || params[name] === null) throw new Error(`missing path parameter ${name}`);
    url = url.replace(`{${name}}`, encodeURIComponent(params[name]));
  }
  const query = new URLSearchParams();
  for (const name of queryParams ?? []) {
    const value = params[name];
    if (value === undefined || value === null || value === '') continue;
    for (const v of Array.isArray(value) ? value : [value]) query.append(name, v);
  }
  const q = query.toString();
  return `${BASE}${url}${q ? `?${q}` : ''}`;
}

let csrf = null;

async function csrfToken(fresh = false) {
  if (!fresh && cookie('XSRF-TOKEN')) return cookie('XSRF-TOKEN');
  csrf ??= fetch(`${BASE}/auth/csrf`, { credentials: 'same-origin' }).finally(() => (csrf = null));
  await csrf;
  return cookie('XSRF-TOKEN') ?? '';
}

const errorListeners = new Set();

/** Calls `listener(error)` for every failed request; returns the unsubscriber. */
export function onApiError(listener) {
  errorListeners.add(listener);
  return () => errorListeners.delete(listener);
}

async function send(op, params, body, fresh) {
  const headers = { Accept: 'application/json' };
  if (op.method !== 'GET') headers['X-XSRF-TOKEN'] = await csrfToken(fresh);
  let payload;
  if (body instanceof FormData) {
    payload = body;
  } else if (body !== undefined) {
    headers['Content-Type'] = 'application/json';
    payload = JSON.stringify(body);
  }
  return fetch(buildUrl(op, params), { method: op.method, headers, body: payload, credentials: 'same-origin' });
}

/** Sends one operation; resolves to its parsed body (null for 204). */
export async function request(op, params = {}, body = undefined) {
  let res = await send(op, params, body, false);
  if (res.status === 419 || (res.status === 403 && op.method !== 'GET' && (await peekCode(res)) === 'csrf_token_mismatch')) {
    // The session (and its token) expired: get a new token and retry once.
    res = await send(op, params, body, true);
  }
  if (!res.ok) {
    let problem = null;
    try {
      problem = await res.json();
    } catch {
      problem = null;
    }
    const error = new ApiError(res.status, problem);
    for (const listener of errorListeners) listener(error, op);
    throw error;
  }
  if (res.status === 204) return null;
  if (op.binary) return res.blob();
  const type = res.headers.get('content-type') ?? '';
  return type.includes('json') ? res.json() : res.text();
}

async function peekCode(res) {
  try {
    return (await res.clone().json()).code;
  } catch {
    return null;
  }
}
