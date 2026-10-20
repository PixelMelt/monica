// The props of the auth pages, built from API objects as their ViewHelpers
// built them from models, and the per-browser state the server used to keep
// for them (flashed status, beta cookie, pending 2FA login).

/** `AcceptInvitationShowViewHelper::data`. */
export const invitation = (inv) => ({ invitation_code: inv.invitation_code });

const WALLPAPERS = Array.from({ length: 20 }, (_, i) => `image${i + 1}.png`);

/** `WallpaperHelper::getRandomWallpaper`: a random login wallpaper (`public/img`). */
export const wallpaper = () => `/img/${WALLPAPERS[Math.floor(Math.random() * WALLPAPERS.length)]}`;

let status = null;

/** `redirect()->with('status', $message)`: shown once by the next page that reads it. */
export function flashStatus(message) {
  status = message;
}

/** `session('status')`: the flashed status, then forgotten. */
export function takeStatus() {
  const message = status;
  status = null;
  return message;
}

const BETA = 'beta';

/** `$request->cookie('beta') !== 'false'` (R002: now kept in localStorage). */
export function betaShown() {
  try {
    return localStorage.getItem(BETA) !== 'false';
  } catch {
    return true;
  }
}

/** `LoginController::closeBeta`: hides the beta banner in this browser. */
export function hideBeta() {
  try {
    localStorage.setItem(BETA, 'false');
  } catch {
    // storage unavailable: the banner shows again next time
  }
}

const PENDING_LOGIN = 'login.pending';

/** Notes that the password was right and a 2FA code is now expected (`login.id` in the session). */
export function setPendingLogin(pending) {
  try {
    if (pending) sessionStorage.setItem(PENDING_LOGIN, '1');
    else sessionStorage.removeItem(PENDING_LOGIN);
  } catch {
    // storage unavailable
  }
}

/** Whether a login waits for its 2FA code (`$request->hasChallengedUser()`). */
export function hasPendingLogin() {
  try {
    return sessionStorage.getItem(PENDING_LOGIN) === '1';
  } catch {
    return true;
  }
}
