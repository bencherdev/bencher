import type { JsonAuthUser } from "../types/bencher";

/** The classic console stores the signed in user here; signing out removes it. */
export const USER_KEY = "BENCHER_USER";

export const readReader = (
	storage: Pick<Storage, "getItem">,
): JsonAuthUser | undefined => {
	try {
		const reader = JSON.parse(storage.getItem(USER_KEY) ?? "null");
		return typeof reader?.token === "string" &&
			reader.token.length > 0 &&
			typeof reader.user?.uuid === "string"
			? reader
			: undefined;
	} catch {
		return undefined;
	}
};

/** Keep `reader` as the signed in user, as the classic console stores it. */
export const rememberReader = (
	storage: Pick<Storage, "setItem">,
	reader: JsonAuthUser,
) => {
	try {
		storage.setItem(USER_KEY, JSON.stringify(reader));
	} catch {
		// The next load reads the reader the API had at sign in.
	}
};

export const isCurrent = (reader: JsonAuthUser, now: number) =>
	Date.parse(reader.expiration) > now;

/** Sign in, then come back to `path`: the classic console's `back` parameter. */
export const signInHref = (path: string) =>
	`/auth/login?${new URLSearchParams({ back: toBase64(path) })}`;

const toBase64 = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)));
