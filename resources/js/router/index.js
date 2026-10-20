// The router: before a page shows, its loader fetches the props the server
// used to render it with (`meta.load(to)` → `meta.props`), after the
// session (`/me/bootstrap`) of the visit. Access rules follow the Laravel
// middleware: `auth` → login, `verified` → email notice, `guest` → home.

import { createRouter, createWebHistory } from 'vue-router';
import routes from './routes';
import progress from './progress';
import { refreshSession, session } from '@/lib/session';
import { pageErrors } from '@/lib/form';
import { ApiError } from '@/api/runtime';

const router = createRouter({
  history: createWebHistory(),
  routes,
  scrollBehavior(to, from, saved) {
    if (saved) return saved;
    if (to.hash) return { el: to.hash };
    if (to.meta.preserveScroll || from.meta.preserveScroll) return false;
    return { top: 0 };
  },
});

const INTENDED = 'url.intended';

/** Remembers where the visitor wanted to go (`redirect()->intended()`). */
export function setIntended(path) {
  try {
    sessionStorage.setItem(INTENDED, path);
  } catch {
    // storage unavailable: the visitor lands on the default page
  }
}

/** `redirect()->intended($default)`: the remembered page, else `fallback`. */
export function intended(fallback = '/vaults') {
  let path = null;
  try {
    path = sessionStorage.getItem(INTENDED);
    sessionStorage.removeItem(INTENDED);
  } catch {
    path = null;
  }
  return path ?? fallback;
}

router.beforeResolve(async (to) => {
  const access = to.meta.access ?? 'verified';
  progress.start();
  try {
    await refreshSession();
    if (access === 'guest' && session.user) {
      return { name: 'home' };
    }
    if ((access === 'user' || access === 'verified') && !session.user) {
      setIntended(to.fullPath);
      return { name: 'login' };
    }
    if (access === 'verified' && !session.user.email_verified_at) {
      return { name: 'verification.notice' };
    }
    pageErrors.errors = {};
    to.meta.error = null;
    to.meta.props = to.meta.load ? await to.meta.load(to) : {};
    if (to.meta.props?.redirect) return to.meta.props.redirect;
  } catch (e) {
    if (!(e instanceof ApiError)) throw e;
    if (e.status === 401) {
      setIntended(to.fullPath);
      return { name: 'login' };
    }
    if (e.code === 'email_not_verified') return { name: 'verification.notice' };
    if (e.code === 'password_confirmation_required') {
      setIntended(to.fullPath);
      return { name: 'password.confirm' };
    }
    to.meta.props = {};
    to.meta.error = { status: e.status, message: e.message };
  } finally {
    progress.done();
  }
  return true;
});

router.onError(() => progress.done());

/** `route(name, params)` of Ziggy, for the SPA's own pages. */
export function route(name, params = {}) {
  const record = router.getRoutes().find((r) => r.name === name);
  if (!record) throw new Error(`route "${name}" is not a page of the SPA`);
  const keys = [...record.path.matchAll(/:(\w+)/g)].map((m) => m[1]);
  if (typeof params !== 'object' || params === null) params = keys.length ? { [keys[0]]: params } : {};
  const path = {};
  const query = {};
  for (const [k, v] of Object.entries(params)) {
    (keys.includes(k) ? path : query)[k] = String(v);
  }
  return router.resolve({ name, params: path, query }).href;
}

/** `router.visit(url)` of Inertia. */
export const visit = (url, { replace = false } = {}) => (replace ? router.replace(url) : router.push(url));

export default router;
