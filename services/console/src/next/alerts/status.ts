import type { JsonConsoleAlertsCounts } from "../../types/bencher";
import type { AlertLine, Kept } from "./rows";

/** A status the page can set: the API never silences or unsilences. */
export type Settable = "active" | "dismissed";

/** A status the page set on an alert, shown before the API answers and kept while the reader stays. */
interface Change extends Kept {
	status: Settable;
	/** The request still out for it; none once the API took it. */
	request: number | undefined;
	/** The change it was made over, which a refusal falls back to. */
	previous: Change | undefined;
	/** The filters it was made under, where the row stays while the reader does. */
	view: string;
}

export type Changes = ReadonlyMap<string, Change>;

/** The changes with `rows` set to `status` by `request`, at once. */
export const begin = (
	changes: Changes,
	request: number,
	status: Settable,
	rows: readonly Kept[],
	view = "",
): Changes => {
	const next = new Map(changes);
	for (const { line, after } of rows) {
		next.set(line.key, {
			line,
			after,
			status,
			request,
			previous: changes.get(line.key),
			view,
		});
	}
	return next;
};

/** Each change of a row, newest first, with `edit` applied: undefined drops a change. */
const rewrite = (
	change: Change | undefined,
	edit: (change: Change) => Change | undefined,
): Change | undefined => {
	if (!change) {
		return undefined;
	}
	const previous = rewrite(change.previous, edit);
	const edited = edit(change);
	return edited ? { ...edited, previous } : previous;
};

const each = (
	changes: Changes,
	request: number,
	edit: (change: Change) => Change | undefined,
): Changes => {
	const next = new Map(changes);
	for (const [key, change] of changes) {
		const rewritten = rewrite(change, (each) =>
			each.request === request ? edit(each) : each,
		);
		if (rewritten) {
			next.set(key, rewritten);
		} else {
			next.delete(key);
		}
	}
	return next;
};

/** The API took `request`: its rows keep their status, no longer pending. */
export const settle = (changes: Changes, request: number): Changes =>
	each(changes, request, (change) => ({ ...change, request: undefined }));

/** The API refused `request`: its changes are gone, wherever they sit under later ones. */
export const rollback = (changes: Changes, request: number): Changes =>
	each(changes, request, () => undefined);

/** The status the row shows: the page's newest change, else the API's. */
export const statusOf = (line: AlertLine, changes: Changes) =>
	changes.get(line.key)?.status ?? line.status;

/** The status the API last took for the row. */
export const confirmedOf = (
	line: AlertLine,
	changes: Changes,
): AlertLine["status"] => {
	for (let change = changes.get(line.key); change; change = change.previous) {
		if (change.request === undefined) {
			return change.status;
		}
	}
	return line.status;
};

/** How far the rows on screen move the active count past what the API took. */
export const shownDelta = (changes: Changes) => {
	let delta = 0;
	for (const { line } of changes.values()) {
		const shown = statusOf(line, changes) === "active";
		const confirmed = confirmedOf(line, changes) === "active";
		delta += Number(shown) - Number(confirmed);
	}
	return delta;
};

const IN_VIEW = {
	active: (status: AlertLine["status"]) => status === "active",
	dismissed: (status: AlertLine["status"]) => status !== "active",
	all: () => true,
} as const;

/** The loaded rows still in a view on the server: where its next batch starts. */
export const inView = (
	lines: readonly AlertLine[],
	view: "active" | "dismissed" | "all",
	changes: Changes,
) => lines.filter((line) => IN_VIEW[view](confirmedOf(line, changes))).length;

/** The lines a change to `status` would move: active ones to dismiss, dismissed ones to reactivate. */
export const changeable = (
	lines: readonly AlertLine[],
	status: Settable,
	changes: Changes,
) =>
	lines.filter(
		(line) =>
			statusOf(line, changes) ===
			(status === "dismissed" ? "active" : "dismissed"),
	);

/** The API changes at most this many listed alerts at once. */
const MAX_LISTED = 255;

/** The items in runs the API takes in one request each. */
export const inChunks = <T>(items: readonly T[]): T[][] => {
	const chunks: T[][] = [];
	for (let start = 0; start < items.length; start += MAX_LISTED) {
		chunks.push(items.slice(start, start + MAX_LISTED));
	}
	return chunks;
};

/** The counts as the page shows them: each delta moves alerts from dismissed to active, a negative one back. */
export const withPending = (
	counts: JsonConsoleAlertsCounts,
	deltas: Iterable<number>,
): JsonConsoleAlertsCounts => {
	let moved = 0;
	for (const delta of deltas) {
		moved += delta;
	}
	return {
		...counts,
		active: Math.max(0, counts.active + moved),
		dismissed: Math.max(0, counts.dismissed - moved),
	};
};
