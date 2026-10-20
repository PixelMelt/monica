// Page loaders: auth. Each returns the props of its page (see router/index.js).
import { notPorted } from '@/loaders/common';

export const home = notPorted('auth.home');

export const login = notPorted('auth.login');

export const register = notPorted('auth.register');

export const forgotPassword = notPorted('auth.forgotPassword');

export const resetPassword = notPorted('auth.resetPassword');

export const twoFactorChallenge = notPorted('auth.twoFactorChallenge');

export const verifyEmail = notPorted('auth.verifyEmail');

export const verifyEmailLink = notPorted('auth.verifyEmailLink');

export const confirmPassword = notPorted('auth.confirmPassword');

export const acceptInvitation = notPorted('auth.acceptInvitation');

export const terms = notPorted('auth.terms');

export const policy = notPorted('auth.policy');
