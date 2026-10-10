import type {
	JsonProjectKey,
	JsonProjectKeyCreated,
} from "../../types/bencher";

const DAY = 24 * 60 * 60;

export interface Expiry {
	id: string;
	label: string;
	/** Seconds; the API takes none as never. */
	ttl: number | undefined;
}

export const EXPIRIES: readonly Expiry[] = [
	{ id: "30d", label: "30 days", ttl: 30 * DAY },
	{ id: "90d", label: "90 days", ttl: 90 * DAY },
	{ id: "1y", label: "1 year", ttl: 365 * DAY },
	{ id: "never", label: "Never", ttl: undefined },
];

export const DEFAULT_EXPIRY = "90d";

/** When a key made at `now` with `expiry` stops working. */
export const expiresAt = (now: number, expiry: Expiry) =>
	expiry.ttl === undefined ? undefined : now + expiry.ttl * 1000;

// A key made with no time to live lasts the API's longest, `u32::MAX` seconds.
const NEVER_MS = (2 ** 32 - 1) * 1000;

const dayFormats = new Map<string, Intl.DateTimeFormat>();

/** "Sep 2, 2026", in the reader's time zone unless one is given. */
export const formatDay = (ms: number, timeZone?: string) => {
	const key = timeZone ?? "";
	let format = dayFormats.get(key);
	if (!format) {
		format = new Intl.DateTimeFormat("en-US", {
			month: "short",
			day: "numeric",
			year: "numeric",
			...(timeZone ? { timeZone } : {}),
		});
		dayFormats.set(key, format);
	}
	return format.format(ms);
};

/** How an active key's end reads: a day, Never, or the day it expired. */
export const keyEnd = (
	key: Pick<JsonProjectKey, "creation" | "expiration">,
	now: number,
	timeZone?: string,
) => {
	const expiration = Date.parse(key.expiration);
	if (expiration - Date.parse(key.creation) >= NEVER_MS) {
		return { text: "Never", expired: false };
	}
	const day = formatDay(expiration, timeZone);
	return expiration <= now
		? { text: `Expired ${day}`, expired: true }
		: { text: day, expired: false };
};

/** Keys by when they were made, newest first, so a new key leads. */
export const newestFirst = (keys: readonly JsonProjectKey[]) =>
	[...keys].sort(
		(a, b) =>
			Date.parse(b.creation) - Date.parse(a.creation) ||
			a.name.localeCompare(b.name),
	);

/** A new key as the list holds it: everything but the secret, which is shown once. */
export const listed = ({
	uuid,
	project,
	name,
	creation,
	expiration,
}: JsonProjectKeyCreated): JsonProjectKey => ({
	uuid,
	project,
	name,
	creation,
	expiration,
});

export interface KeyLists {
	active: JsonProjectKey[];
	revoked: JsonProjectKey[];
}

/** The lists once `uuid` is revoked at `at`. */
export const revoke = (
	{ active, revoked }: KeyLists,
	uuid: string,
	at: string,
): KeyLists => {
	const key = active.find((candidate) => candidate.uuid === uuid);
	return {
		active: active.filter((candidate) => candidate.uuid !== uuid),
		revoked: key ? [{ ...key, revoked: at }, ...revoked] : revoked,
	};
};
