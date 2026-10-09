import type { GroupRow } from "../report/lines";
import type { Slot } from "../report/slots";
import type { AlertLine, AlertReport } from "./lines";

/** Each alert once, where it first came: a status change can move one into the next batch. */
export const distinctLines = (
	batches: readonly (readonly AlertLine[])[],
): AlertLine[] => {
	const seen = new Set<string>();
	return batches.flat().filter(({ key }) => {
		if (seen.has(key)) {
			return false;
		}
		seen.add(key);
		return true;
	});
};

/** A row the page changed, and the row it followed on screen when it changed. */
export interface Kept {
	line: AlertLine;
	after: string | undefined;
}

/**
 * The list's rows, each kept row as the object drawn when it changed, and each
 * kept row the list no longer holds back after the row it followed.
 */
export const withKept = (
	listedRows: readonly AlertLine[],
	kept: readonly Kept[],
): AlertLine[] => {
	const drawn = new Map(kept.map(({ line }) => [line.key, line]));
	const rows = listedRows.map((row) => drawn.get(row.key) ?? row);
	const listed = new Set(rows.map(({ key }) => key));
	const following = new Map<string | undefined, AlertLine[]>();
	for (const { line, after } of kept) {
		if (!listed.has(line.key)) {
			following.set(after, [...(following.get(after) ?? []), line]);
		}
	}
	if (following.size === 0) {
		return [...rows];
	}
	const out: AlertLine[] = [];
	const placed = new Set<string>();
	const place = (line: AlertLine) => {
		if (placed.has(line.key)) {
			return;
		}
		placed.add(line.key);
		out.push(line);
		for (const next of following.get(line.key) ?? []) {
			place(next);
		}
	};
	for (const line of following.get(undefined) ?? []) {
		place(line);
	}
	for (const line of rows) {
		place(line);
	}
	for (const lines of following.values()) {
		for (const line of lines) {
			place(line);
		}
	}
	return out;
};

/**
 * The rows a status view shows. Active keeps an alert the page dismissed, grey
 * where it was, until the reader leaves; Dismissed lets go of one the page
 * reactivated.
 */
export const visibleLines = (
	lines: readonly AlertLine[],
	view: "active" | "dismissed" | "all",
	statusOf: (line: AlertLine) => AlertLine["status"],
	changed: (line: AlertLine) => boolean,
) => {
	switch (view) {
		case "all":
			return [...lines];
		case "active":
			return lines.filter(
				(line) => statusOf(line) === "active" || changed(line),
			);
		case "dismissed":
			return lines.filter((line) => statusOf(line) !== "active");
	}
};

/** One report's run of rows: its header in the list, the report, and its rows' alerts. */
export interface AlertGroup {
	row: GroupRow;
	report: AlertReport;
	branch: AlertLine["branch"];
	testbed: AlertLine["testbed"];
	keys: string[];
}

/**
 * The rows with a header before each run of one report's rows. A report keeps
 * its header row from the `previous` layout, so the table redraws none.
 */
export const alertSlots = (
	lines: readonly AlertLine[],
	previous: readonly AlertGroup[] = [],
) => {
	const headers = new Map(
		previous.map(({ report, row }) => [report.uuid, row]),
	);
	const slots: Slot[] = [];
	const groups: AlertGroup[] = [];
	let current: AlertGroup | undefined;
	lines.forEach((line, position) => {
		if (current?.report.uuid !== line.report.uuid) {
			const kept = headers.get(line.report.uuid);
			headers.delete(line.report.uuid);
			const row: GroupRow = Object.assign(
				kept ?? { name: line.report.uuid, variants: 0 },
				{ lines: 0, alerts: line.report.total, start: position },
			);
			current = {
				row,
				report: line.report,
				branch: line.branch,
				testbed: line.testbed,
				keys: [],
			};
			groups.push(current);
			slots.push(row);
		}
		current.keys.push(line.key);
		current.row.lines += 1;
		slots.push(line);
	});
	return { slots, groups };
};

/** How much of a group is selected. */
export const groupState = (
	keys: readonly string[],
	selected: ReadonlySet<string>,
): "none" | "some" | "all" => {
	const count = keys.filter((key) => selected.has(key)).length;
	return count === 0 ? "none" : count === keys.length ? "all" : "some";
};

/** The selection after a group's checkbox: a full group empties, any other fills. */
export const toggleGroup = (
	keys: readonly string[],
	selected: ReadonlySet<string>,
) => {
	const next = new Set(selected);
	if (groupState(keys, selected) === "all") {
		for (const key of keys) {
			next.delete(key);
		}
	} else {
		for (const key of keys) {
			next.add(key);
		}
	}
	return next;
};
