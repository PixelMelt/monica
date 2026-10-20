// Page loaders: auth. Each returns the props of its page (see router/index.js).
import { trans } from 'laravel-vue-i18n';
import { session } from '@/lib/session';
import { markdown } from '@/lib/markdown';
import { route, intended } from '@/router';
import { getInvitation, verifyEmail as sendVerification } from '@/api/client';
import { ApiError } from '@/api/runtime';
import * as dto from '@/adapters/auth';

/** The `/` closure of routes/web.php: the only vault if there is one, else the vault list. */
export function home() {
  if (session.vaults.length === 1) {
    return { redirect: intended(route('vault.show', { vault: session.vaults[0].id })) };
  }
  return { redirect: intended(route('vault.index')) };
}

/** `LoginController::__invoke`: email and password sign-in. */
export function login() {
  return {
    isSignupEnabled: session.instance.signup_enabled,
    canResetPassword: true,
    status: dto.takeStatus(),
    wallpaperUrl: dto.wallpaper(),
    beta: dto.betaShown(),
    showDeletionNotice: session.instance.show_deletion_notice,
  };
}

/** `RegisterController::__invoke`, behind `EnsureSignupIsEnabled`. */
export function register() {
  if (!session.instance.signup_enabled) {
    throw new ApiError(403, { detail: trans('Registration is currently disabled'), code: 'signup_disabled' });
  }
  return {
    beta: dto.betaShown(),
    jetstream: { hasTermsAndPrivacyPolicyFeature: true },
  };
}

/** Fortify's `PasswordResetLinkController::create`. */
export const forgotPassword = () => ({});

/** Fortify's `NewPasswordController::create`. */
export const resetPassword = (to) => ({ token: to.params.token, email: to.query.email ?? null });

/** `TwoFactorChallengeView`: TOTP or a recovery code; without a pending login, back to the login page. */
export function twoFactorChallenge() {
  if (!dto.hasPendingLogin()) return { redirect: route('login') };
  return { twoFactor: true };
}

/** Fortify's `EmailVerificationPromptController`. */
export function verifyEmail() {
  if (session.user.email_verified_at) return { redirect: intended() };
  return {};
}

/** Fortify's `VerifyEmailController`: the link of the verification email. */
export async function verifyEmailLink(to) {
  await sendVerification(
    {},
    {
      id: to.params.id,
      hash: to.params.hash,
      expires: Number(to.query.expires),
      signature: to.query.signature,
    },
  );
  return { redirect: intended(route('vault.index', { verified: 1 })) };
}

/** Fortify's `ConfirmablePasswordController::show`. */
export const confirmPassword = () => ({});

/** `AcceptInvitationController::show`: an unknown or used code goes home. */
export async function acceptInvitation(to) {
  try {
    const inv = await getInvitation({ invitation_code: to.params.code });
    return { data: dto.invitation(inv) };
  } catch (e) {
    if (e instanceof ApiError && [400, 404, 422].includes(e.status)) return { redirect: '/' };
    throw e;
  }
}

const MARKDOWN = import.meta.glob('../../markdown/*.md', { query: '?raw', import: 'default' });

/** `Str::markdown` of `Jetstream::localizedMarkdownPath($name)`: `terms.fr.md` before `terms.md`. */
async function localizedMarkdown(name) {
  const locale = session.user?.preferences.locale ?? session.instance.locale;
  const localName = name.replace(/(\.md)$/i, `.${locale}$1`);
  const load = MARKDOWN[`../../markdown/${localName}`] ?? MARKDOWN[`../../markdown/${name}`];
  return markdown(await load());
}

/** Jetstream's `TermsOfServiceController::show`. */
export const terms = async () => ({ terms: await localizedMarkdown('terms.md') });

/** Jetstream's `PrivacyPolicyController::show`. */
export const policy = async () => ({ policy: await localizedMarkdown('policy.md') });
