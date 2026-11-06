/**
 * Who is signed in, as the console paints them before the API answers. It
 * authenticates nothing: the session is an HttpOnly cookie no script reads.
 */
export interface Reader {
	uuid: string;
	slug: string;
	name: string;
}

export const READER_KEY = "BENCHER_READER";

export const readReader = (
	storage: Pick<Storage, "getItem">,
): Reader | undefined => {
	try {
		const { uuid, slug, name } =
			JSON.parse(storage.getItem(READER_KEY) ?? "null") ?? {};
		return typeof uuid === "string" &&
			uuid &&
			typeof slug === "string" &&
			slug &&
			typeof name === "string"
			? { uuid, slug, name }
			: undefined;
	} catch {
		return undefined;
	}
};

/** Keep `user` as the reader. */
export const rememberReader = (
	storage: Pick<Storage, "setItem">,
	{ uuid, slug, name }: Reader,
) => {
	try {
		storage.setItem(READER_KEY, JSON.stringify({ uuid, slug, name }));
	} catch {
		// The next sign in keeps it.
	}
};

/** Sign in, then come back to `path`. */
export const signInHref = (path: string) =>
	`/auth/login?${new URLSearchParams({ back: toBase64(path) })}`;

const toBase64 = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)));
