// The root page inlines this function's source, so it reads nothing outside itself.
export const landing = (
	user: string | null,
	versions: string | null,
	search: string,
	now: number,
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
			if (version === 1) {
				return `/next/console/projects/${slug}/`;
			}
			if (version === 0) {
				return `/console/projects/${slug}/plots`;
			}
		}
		return undefined;
	} catch {
		return undefined;
	}
};
