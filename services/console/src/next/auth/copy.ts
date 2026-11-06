import type { Refused } from "./request";

/** What a refusal tells the reader, and whether trying again could help. */
export interface Failure {
	text: string;
	retry: boolean;
}

export const VERIFICATION_FAILED: Failure = {
	text: "Verification failed",
	retry: true,
};
export const SIGN_IN_FAILED: Failure = { text: "Sign-in failed", retry: true };

/** The door's words for a refusal to start a sign-in; any code it does not know is the generic failure. */
export const doorFailure = ({ code, retryAfter }: Refused): Failure => {
	switch (code) {
		case "turnstile_failed":
			return VERIFICATION_FAILED;
		case "email_failed":
			return { text: "The email did not send", retry: true };
		case "too_many_attempts":
			return { text: tooManyAttempts(retryAfter), retry: false };
		case "invalid_invite":
			return { text: "This invite is not valid", retry: false };
		case "invite_for_another_email":
			return { text: "This invite is for another email", retry: false };
		case "unknown_organization":
			return { text: "This organization no longer exists", retry: false };
		default:
			return SIGN_IN_FAILED;
	}
};

/** Check your inbox's words for a refusal that is no state of its own. */
export const inboxFailure = ({ code, retryAfter }: Refused): Failure => {
	switch (code) {
		case "turnstile_failed":
			return VERIFICATION_FAILED;
		case "email_failed":
			return { text: "The email did not send", retry: true };
		case "too_many_attempts":
			return { text: tooManyAttempts(retryAfter), retry: false };
		case "locked":
			return { text: LOCKED, retry: false };
		default:
			return SIGN_IN_FAILED;
	}
};

/**
 * A provider callback's words for a refusal by the API, which is never the
 * provider's own; any code it does not know is the generic failure.
 */
export const providerFailure = ({ code, retryAfter }: Refused): string => {
	switch (code) {
		case "locked":
			return LOCKED;
		case "too_many_attempts":
			return tooManyAttempts(retryAfter);
		default:
			return SIGN_IN_FAILED.text;
	}
};

export const refusedBy = (provider: string) => `Refused by ${provider}`;

export const WRONG_CODE = "Wrong code";
export const NO_TRIES_LEFT = "No tries left";
export const EXPIRED = "Expired";
export const CODES_PAUSED = "Too many wrong codes. Use the link in your email.";
const LOCKED = "This account is locked";

/** An address at its cap, with the real wait. */
export const capped = (seconds: number) =>
	`Too many emails to this address. Try again in ${duration(seconds)}.`;

const tooManyAttempts = (seconds: number | undefined) =>
	seconds === undefined
		? "Too many attempts. Try again later."
		: `Too many attempts. Try again in ${duration(seconds)}.`;

/** A wait in whole minutes, or whole hours past the first, always rounded up. */
export const duration = (seconds: number) => {
	const minutes = Math.max(1, Math.ceil(seconds / 60));
	if (minutes < 60) {
		return minutes === 1 ? "1 minute" : `${minutes} minutes`;
	}
	const hours = Math.ceil(minutes / 60);
	return hours === 1 ? "1 hour" : `${hours} hours`;
};

/** Resend's countdown, as the board writes it. */
export const resendIn = (seconds: number) => `Resend in ${seconds}s`;
