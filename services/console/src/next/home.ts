import { VERSION_KEY, readJson } from "./memory";

// Kept out of `memory.ts`, which the public plot loads too, so only the console pays for it.

/** Where a project opens at the version the browser last saw it at; `Landing.astro` inlines its source. */
export const projectAt = (
	slug: string,
	version: unknown,
): string | undefined =>
	version === 1
		? `/next/console/projects/${slug}/`
		: version === 0
			? `/console/projects/${slug}/plots`
			: undefined;

/** The most recently loaded project the browser remembers, where it opens. */
export const rememberedHome = (storage: Pick<Storage, "getItem">) => {
	for (const [slug, version] of Object.entries(
		readJson(storage, VERSION_KEY) ?? {},
	).reverse()) {
		const to = projectAt(slug, version);
		if (to) {
			return to;
		}
	}
	return undefined;
};
