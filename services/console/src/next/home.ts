import { VERSION_KEY, readJson } from "./memory";

// Kept out of `memory.ts`, which the public plot loads too, so only the console pays for it.

/** Where a project the browser remembers at either version opens; `Landing.astro` inlines its source. */
export const projectAt = (
	slug: string,
	version: unknown,
): string | undefined =>
	version === 0 || version === 1 ? `/console/projects/${slug}/` : undefined;

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
