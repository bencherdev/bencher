import type { JsonOAuthGrant } from "../../../types/bencher";

const USER_KEY_PREFIX = "bencher_user_";

/** Why the connected apps could not be listed. */
export enum AppsRefusal {
	/** The server answers `404` for every user while it does not connect apps. */
	Unavailable = "unavailable",
	/** Connected apps are managed only from a logged in session, never with a key. */
	KeyRefused = "key_refused",
	Forbidden = "forbidden",
	SessionEnded = "session_ended",
	/** Anything else, the network included. */
	Failed = "failed",
}

export const appsRefusal = (
	status: undefined | number,
	token: undefined | string,
): AppsRefusal => {
	switch (status) {
		case 404:
			return AppsRefusal.Unavailable;
		case 403:
			return token?.startsWith(USER_KEY_PREFIX)
				? AppsRefusal.KeyRefused
				: AppsRefusal.Forbidden;
		// A session token the API cannot validate is a `400`.
		case 400:
		case 401:
			return AppsRefusal.SessionEnded;
		default:
			return AppsRefusal.Failed;
	}
};

export const appsRefusalText = (refusal: AppsRefusal): string => {
	switch (refusal) {
		case AppsRefusal.Unavailable:
			return "Apps cannot connect to this Bencher server.";
		case AppsRefusal.KeyRefused:
			return "Connected apps are managed only when you log in, not with an API key.";
		case AppsRefusal.Forbidden:
			return "You do not have access to these connected apps.";
		case AppsRefusal.SessionEnded:
			return "Your session ended. Log in again to manage your connected apps.";
		case AppsRefusal.Failed:
			return "Bencher could not be reached. Try again.";
	}
};

/** When the app last renewed its access, if it ever has. */
export const lastRefreshed = (
	grant: Pick<JsonOAuthGrant, "created" | "modified">,
): undefined | string =>
	Date.parse(grant.modified) > Date.parse(grant.created)
		? grant.modified
		: undefined;
