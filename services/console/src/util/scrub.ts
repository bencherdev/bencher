const API_KEY = /bencher_(?:user|run)_[A-Za-z0-9]+/g;
export const FILTERED = "[Filtered]";

/** A filter of every Bencher API key, and of each of `secrets` in any case, out of every string in a value, at any depth. */
export const scrubber = (secrets: readonly string[]) => {
	const patterns = [
		API_KEY,
		...secrets.map(
			(secret) =>
				new RegExp(secret.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"), "gi"),
		),
	];
	return <T>(value: T): T => scrub(value, patterns, new WeakSet<object>()) as T;
};

const scrub = (
	value: unknown,
	patterns: readonly RegExp[],
	seen: WeakSet<object>,
): unknown => {
	if (typeof value === "string") {
		return patterns.reduce(
			(text, pattern) => text.replace(pattern, FILTERED),
			value,
		);
	}
	if (typeof value !== "object" || value === null || seen.has(value)) {
		return value;
	}
	seen.add(value);
	const fields = value as Record<string, unknown>;
	for (const [name, field] of Object.entries(fields)) {
		const scrubbed = scrub(field, patterns, seen);
		if (scrubbed !== field) {
			fields[name] = scrubbed;
		}
	}
	return value;
};
