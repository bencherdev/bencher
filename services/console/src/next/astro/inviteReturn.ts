/**
 * Where the door's first script keeps the secret of an invitation its link
 * carried in the fragment: the page's memory, never its address.
 */
export const INVITE_RETURN_KEY = "BENCHER_INVITE_RETURN";

/** The invitation's secret the door's first script took from its address, once: a second take finds none. */
export const takeInvite = (memory: object = window): string | undefined => {
	const kept: unknown = Reflect.get(memory, INVITE_RETURN_KEY);
	Reflect.deleteProperty(memory, INVITE_RETURN_KEY);
	return typeof kept === "string" && kept ? kept : undefined;
};
