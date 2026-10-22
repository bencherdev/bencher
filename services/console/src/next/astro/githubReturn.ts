/** Where the GitHub return page's first script keeps GitHub's code and state: the page's memory, never its URL. */
export const RETURN_KEY = "BENCHER_GITHUB_RETURN";

/** The code and the state the return page took from its URL, once: a second take finds neither. */
export const takeGitHubReturn = (memory: object = window) => {
	const kept = Reflect.get(memory, RETURN_KEY) as
		| { code?: unknown; state?: unknown }
		| undefined;
	Reflect.deleteProperty(memory, RETURN_KEY);
	return {
		code: typeof kept?.code === "string" ? kept.code : undefined,
		state: typeof kept?.state === "string" ? kept.state : undefined,
	};
};
