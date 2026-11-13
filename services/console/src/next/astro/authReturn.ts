/**
 * Where an auth page's first script keeps what its address carried: a link's
 * secret, an old link's token, and a provider's code and state. The page's
 * memory holds them, never its address.
 */
export const AUTH_RETURN_KEY = "BENCHER_AUTH_RETURN";

const FIELDS = ["secret", "token", "code", "state"] as const;

type AuthReturn = Record<(typeof FIELDS)[number], string | undefined>;

/** What the page's first script took from its address, once: a second take finds nothing. */
export const takeAuthReturn = (memory: object = window): AuthReturn => {
	const kept = Reflect.get(memory, AUTH_RETURN_KEY) as
		| Record<string, unknown>
		| undefined;
	Reflect.deleteProperty(memory, AUTH_RETURN_KEY);
	return Object.fromEntries(
		FIELDS.map((field) => {
			const value = kept?.[field];
			return [field, typeof value === "string" && value ? value : undefined];
		}),
	) as AuthReturn;
};
