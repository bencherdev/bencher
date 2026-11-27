import type { projectAt } from "./home";

/** The reader's main project as the browser remembers it, for a signed in reader; the root page inlines its source, so it reads nothing outside itself. */
export const landing = (
	reader: string | null,
	versions: string | null,
	search: string,
	at: typeof projectAt,
): string | undefined => {
	if (search) {
		return undefined;
	}
	try {
		if (!JSON.parse(reader ?? "null")?.uuid) {
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
