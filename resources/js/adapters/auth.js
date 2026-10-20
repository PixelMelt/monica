// The props of the auth pages, and the per-browser state the server used to
// keep for them (the pending 2FA login).

const WALLPAPERS = Array.from({ length: 20 }, (_, i) => `image${i + 1}.png`);

/** `WallpaperHelper::getRandomWallpaper`: a random login wallpaper (`public/img`). */
export const wallpaper = () => `/img/${WALLPAPERS[Math.floor(Math.random() * WALLPAPERS.length)]}`;

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
