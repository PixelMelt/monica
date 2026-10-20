// Page loaders: auth. Each returns the props of its page (see router/index.js).
import { session } from '@/lib/session';
import { route, intended } from '@/router';
import * as dto from '@/adapters/auth';

/** The `/` closure of routes/web.php: the only vault if there is one, else the vault list. */
export function home() {
  if (session.vaults.length === 1) {
    return { redirect: intended(route('vault.show', { vault: session.vaults[0].id })) };
  }
  return { redirect: intended(route('vault.index')) };
}

/** `LoginController::__invoke`: username and password sign-in. */
export function login() {
  return { wallpaperUrl: dto.wallpaper() };
}

/** First-run setup: only while no user exists, else the login page. */
export function register() {
  if (!session.instance.setup_required) return { redirect: route('login') };
  return {};
}

/** `TwoFactorChallengeView`: TOTP or a recovery code; without a pending login, back to the login page. */
export function twoFactorChallenge() {
  if (!dto.hasPendingLogin()) return { redirect: route('login') };
  return { twoFactor: true };
}

/** Fortify's `ConfirmablePasswordController::show`. */
export const confirmPassword = () => ({});
