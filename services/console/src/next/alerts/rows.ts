import type {
	AlertStatus,
	JsonConsoleAlertLine,
	JsonConsoleAlerts,
} from "../../types/bencher";
import { lineKey } from "../query/line";
import { guardOf } from "../report/delta";
import { type GroupRow, type ReportLine, alignSeries } from "../report/lines";
import type { Slot } from "../report/slots";
import { shortHash } from "../reports/format";

/** The report that raised an alert, as its group's header names it. */
interface AlertReport {
	uuid: string;
	start: number;
	created: number;
	hash: string | undefined;
	adapter: string;
	/** Its alerts that match the list's filters and status, over every page. */
	total: number;
}

/** An alert as its row draws it: its line, keyed by the alert, with the report that raised it. */
export interface AlertLine extends ReportLine {
	/** The line's own key, as Explore names it; `key` is the alert's. */
	line: string;
	report: AlertReport;
	status: "active" | "dismissed" | "silenced";
	/** When the alert last changed status, or was raised. */
	modified: number;
}

/** Each alert of a batch, in its group's order, resolved against the batch's tables. */
export const alertLinesOf = (batch: JsonConsoleAlerts): AlertLine[] =>
	batch.groups.flatMap((group) => {
		const branch = batch.branches[group.branch];
		const testbed = batch.testbeds[group.testbed];
		if (!(branch && testbed)) {
			return [];
		}
		const points = {
			x: group.points.x,
			report: group.points.report,
			reports: batch.reports,
		};
		const report: AlertReport = {
			uuid: group.uuid,
			start: group.start_time,
			created: group.created,
			hash: shortHash(group.version.hash),
			adapter: group.adapter,
			total: group.total,
		};
		return group.alerts.flatMap(({ line, modified }) => {
			const benchmark = batch.benchmarks[line.benchmark];
			const variant = batch.variants[line.variant];
			const measure = batch.measures[line.measure];
			if (!(benchmark && variant && measure && line.alert)) {
				return [];
			}
			const model =
				line.model === undefined ? undefined : batch.models[line.model];
			return [
				{
					key: line.alert.uuid,
					line: lineKey({
						branch: branch.uuid,
						testbed: testbed.uuid,
						benchmark: benchmark.uuid,
						parameters: variant.parameters,
						measure: measure.uuid,
						metric: line.metric,
					}),
					branch,
					testbed,
					benchmark,
					variant: variant.uuid,
					parameters: variant.parameters,
					measure,
					metric: line.metric,
					value: line.value,
					baseline: line.baseline,
					lower_limit: line.lower_limit,
					upper_limit: line.upper_limit,
					// A row alerts while its alert is active; the rest say their status.
					alert: line.alert.status === "active" ? line.alert : undefined,
					guard: guardOf(model),
					history: alignSeries(line.history, points.x.length),
					points,
					report,
					status: line.alert.status,
					modified,
				},
			];
		});
	});

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

/** The list's rows, with each kept row the list no longer holds back after the row it followed. */
export const withKept = (
	rows: readonly AlertLine[],
	kept: readonly Kept[],
): AlertLine[] => {
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

/** A batch with each alert in `statuses` changed to its status at `modified`; the rest keep their objects. */
export const withStatuses = (
	alerts: JsonConsoleAlerts,
	statuses: ReadonlyMap<string, "active" | "dismissed">,
	modified: number,
): JsonConsoleAlerts => {
	// An alert also marks its point in the history of later rows on its line.
	const touched = ({ line }: JsonConsoleAlertLine) =>
		(line.alert !== undefined && statuses.has(line.alert.uuid)) ||
		line.history.alerts.some(({ uuid }) => statuses.has(uuid));
	if (!alerts.groups.some((group) => group.alerts.some(touched))) {
		return alerts;
	}
	const changed = <T extends { uuid: string; status: AlertStatus }>(
		alert: T,
	): T => {
		const status = statuses.get(alert.uuid);
		return status === undefined
			? alert
			: { ...alert, status: status as AlertStatus };
	};
	const rewrite = (entry: JsonConsoleAlertLine): JsonConsoleAlertLine => {
		const { alert, history } = entry.line;
		return {
			modified:
				alert !== undefined && statuses.has(alert.uuid)
					? modified
					: entry.modified,
			line: {
				...entry.line,
				...(alert && { alert: changed(alert) }),
				history: { ...history, alerts: history.alerts.map(changed) },
			},
		};
	};
	return {
		...alerts,
		groups: alerts.groups.map((group) =>
			group.alerts.some(touched)
				? {
						...group,
						alerts: group.alerts.map((entry) =>
							touched(entry) ? rewrite(entry) : entry,
						),
					}
				: group,
		),
	};
};
