/**
 * Where the email change page's first script keeps the secret its link
 * carried in the fragment: the page's memory, never its address.
 */
export const EMAIL_RETURN_KEY = "BENCHER_EMAIL_RETURN";

/** The secret the page's first script took from its address, once: a second take finds none. */
export const takeEmailChange = (
	memory: object = window,
): string | undefined => {
	const kept: unknown = Reflect.get(memory, EMAIL_RETURN_KEY);
	Reflect.deleteProperty(memory, EMAIL_RETURN_KEY);
	return typeof kept === "string" && kept ? kept : undefined;
};
