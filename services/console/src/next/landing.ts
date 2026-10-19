import type { projectAt } from "./home";

/** The reader's main project as the browser remembers it, for a signed in reader; the root page inlines its source, so it reads nothing outside itself. */
export const landing = (
	user: string | null,
	versions: string | null,
	search: string,
	now: number,
	at: typeof projectAt,
): string | undefined => {
	if (search) {
		return undefined;
	}
	try {
		const reader = JSON.parse(user ?? "null");
		if (
			!reader?.token ||
			typeof reader.token !== "string" ||
			!(Date.parse(reader.expiration) > now)
		) {
			return undefined;
		}
		const remembered = Object.entries(JSON.parse(versions ?? "null") ?? {});
		for (const [slug, version] of remembered.reverse()) {
			const to = at(slug, version);
			if (to) {
				return to;
			}
		}
		return undefined;
	} catch {
		return undefined;
	}
};
