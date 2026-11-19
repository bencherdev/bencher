/**
 * Who is signed in, as the console paints them before the API answers. It
 * authenticates nothing: the session is an HttpOnly cookie no script reads,
 * and the API decides every action a server admin's controls ask for.
 */
export interface Reader {
	uuid: string;
	slug: string;
	name: string;
	/** A server admin, whom the console shows the server admin's controls. */
	admin?: true;
}

export const READER_KEY = "BENCHER_READER";

export const readReader = (
	storage: Pick<Storage, "getItem">,
): Reader | undefined => {
	try {
		const { uuid, slug, name, admin } =
			JSON.parse(storage.getItem(READER_KEY) ?? "null") ?? {};
		return typeof uuid === "string" &&
			uuid &&
			typeof slug === "string" &&
			slug &&
			typeof name === "string"
			? { uuid, slug, name, ...(admin === true ? { admin } : {}) }
			: undefined;
	} catch {
		return undefined;
	}
};

/** Keep `user` as the reader. */
export const rememberReader = (
	storage: Pick<Storage, "setItem">,
	user: ReaderOf,
) => {
	try {
		storage.setItem(READER_KEY, JSON.stringify(readerOf(user)));
	} catch {
		// The next sign in keeps it.
	}
};

/** A user, or the reader, as far as the hint keeps them. */
export type ReaderOf = Omit<Reader, "admin"> & { admin?: boolean };

/** Who `user` is, as the hint keeps them. */
export const readerOf = ({ uuid, slug, name, admin }: ReaderOf): Reader => ({
	uuid,
	slug,
	name,
	...(admin ? { admin: true as const } : {}),
});

/** Sign in, then come back to `path`. */
export const signInHref = (path: string) =>
	`/auth/login?${new URLSearchParams({ back: toBase64(path) })}`;

const toBase64 = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)));
