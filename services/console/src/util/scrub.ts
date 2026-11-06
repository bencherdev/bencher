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
	const clean = (text: string) =>
		patterns.reduce((done, pattern) => done.replace(pattern, FILTERED), text);
	return <T>(value: T): T => scrub(value, clean, new WeakSet<object>()) as T;
};

/** Every string in `value`, at any depth, put through `clean`. */
export const scrub = (
	value: unknown,
	clean: (text: string) => string,
	seen: WeakSet<object>,
): unknown => {
	if (typeof value === "string") {
		return clean(value);
	}
	if (typeof value !== "object" || value === null || seen.has(value)) {
		return value;
	}
	seen.add(value);
	const fields = value as Record<string, unknown>;
	for (const [name, field] of Object.entries(fields)) {
		const scrubbed = scrub(field, clean, seen);
		if (scrubbed !== field) {
			fields[name] = scrubbed;
		}
	}
	return value;
};

/** Where a page names what its reports must never carry, for the reporter that loads after it. */
const REPORT_SECRETS = "BENCHER_REPORT_SECRETS";

/** Keep each of `secrets`, in any case, out of every report this page sends from now on. */
export const keepOutOfReports = (
	secrets: readonly string[],
	memory: object = window,
) => {
	Reflect.set(memory, REPORT_SECRETS, [...secrets]);
};

/** What this page asked to keep out of its reports. */
export const reportSecrets = (memory: object = globalThis): string[] => {
	const kept: unknown = Reflect.get(memory, REPORT_SECRETS);
	return Array.isArray(kept)
		? kept.filter(
				(secret): secret is string =>
					typeof secret === "string" && secret.length > 0,
			)
		: [];
};
