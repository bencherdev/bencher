import type { Query, QueryClient } from "@tanstack/solid-query";
import type {
	AlertStatus,
	JsonAlert,
	JsonConsoleAlertDetail,
	JsonConsoleAlerts,
	JsonConsolePerf,
	JsonConsolePlots,
	JsonConsoleProject,
	JsonConsoleReport,
	JsonConsoleSeries,
	JsonUpdateAlerts,
} from "../../types/bencher";
import type { Api } from "../api";
import { consoleProjectQuery } from "../queries";
import type { ReportBatch } from "../report/query";
import type { AlertsBatch } from "./query";
import { type Settable, updateAlerts, withStatuses } from "./status";

/**
 * Write the statuses the API took into every cached answer of the project's
 * that holds one of them, but those of the kind `skip` names, which the page
 * writes and marks itself. An answer a page draws waits until no page does,
 * since its rows and plots would redraw under the reader, and is left as it
 * is if it reads again meanwhile. Every view of alerts is then stale, so each
 * reads again on its next visit.
 */
export const writeStatuses = (
	client: QueryClient,
	slug: string,
	statuses: Statuses,
	modified: number,
	skip?: string,
) => {
	const cache = client.getQueryCache();
	const stale = (query: Query) =>
		query.queryKey[0] === "console" &&
		query.queryKey[2] === slug &&
		query.queryKey[1] !== skip &&
		STALE.has(query.queryKey[1] as string);
	/** Whether more of the answer waits for its page to go. */
	const write = (query: Query, drawn: boolean) => {
		const data = query.state.data;
		const written = rewrite(query.queryKey, data, statuses, modified, drawn);
		if (written !== data) {
			client.setQueryData(query.queryKey, written);
			ownWrites.set(query, query.state.dataUpdateCount);
		}
		return (
			drawn &&
			rewrite(query.queryKey, data, statuses, modified, false) !== written
		);
	};
	// Each drawn answer still to write, and how many times its data had changed when it was left waiting.
	const waiting = new Map<Query, number>();
	for (const query of cache.findAll({ predicate: stale })) {
		if (query.state.data === undefined) {
			continue;
		}
		if (write(query, query.getObserversCount() > 0)) {
			waiting.set(query, query.state.dataUpdateCount);
		}
	}
	client.invalidateQueries({ predicate: stale, refetchType: "none" });
	if (waiting.size === 0) {
		return;
	}
	const unsubscribe = cache.subscribe(({ type, query }) => {
		if (
			!waiting.has(query) ||
			(type !== "observerRemoved" && type !== "removed")
		) {
			return;
		}
		// A page leaving and the next one arriving can share the same tick.
		queueMicrotask(() => {
			const count = waiting.get(query);
			if (count === undefined || query.getObserversCount() > 0) {
				return;
			}
			waiting.delete(query);
			const { dataUpdateCount } = query.state;
			if (
				cache.get(query.queryHash) === query &&
				(dataUpdateCount === count || dataUpdateCount === ownWrites.get(query))
			) {
				write(query, false);
				client.invalidateQueries({
					queryKey: query.queryKey,
					exact: true,
					refetchType: "none",
				});
			}
			if (waiting.size === 0) {
				unsubscribe();
			}
		});
	});
};

/**
 * How many times each answer's data had changed when this writer last wrote
 * it, so a change waiting on the same answer does not take an earlier change's
 * write for a read.
 */
const ownWrites = new WeakMap<Query, number>();

/**
 * Change one alert's status, and once the API takes it write it into every
 * cached answer that holds it. Resolves to how many alerts changed.
 */
export const sendChange = async (
	api: Api,
	client: QueryClient,
	slug: string,
	uuid: string,
	status: Settable,
) => {
	const changed = await updateAlerts(api, slug, {
		status: status as JsonUpdateAlerts["status"],
		alerts: [uuid],
	});
	if (changed > 0) {
		writeStatuses(client, slug, new Map([[uuid, status]]), Date.now());
	}
	return changed;
};

/**
 * Move the Alerts count by the change at once, make it, and if the API refuses
 * it or changes nothing, `undo` it and move the count back unless a newer change
 * stands; a change of nothing reads the count again, since the API's may differ
 * from the one shown.
 */
export const changeAlert = async (
	{ api, slug }: { api: Api; slug: () => string },
	client: QueryClient,
	uuid: string,
	status: Settable,
	undo: () => boolean,
) => {
	const key = consoleProjectQuery(api, slug()).queryKey;
	const moveBadge = (delta: number) =>
		client.setQueryData<JsonConsoleProject>(
			key,
			(project) =>
				project && {
					...project,
					active_alerts: Math.max(0, project.active_alerts + delta),
				},
		);
	const delta = status === "dismissed" ? -1 : 1;
	moveBadge(delta);
	const changed = await sendChange(api, client, slug(), uuid, status).catch(
		() => -1,
	);
	if (changed < 1 && undo()) {
		moveBadge(-delta);
	}
	if (changed === 0) {
		client.invalidateQueries({ queryKey: key });
	}
};

/**
 * An answer of a kind that holds alerts, with each alert in `statuses` at its
 * status; any other answer keeps its object. A `drawn` one changes only an
 * alert's own facts, since its plots would redraw under the reader.
 */
const rewrite = (
	key: readonly unknown[],
	data: unknown,
	statuses: Statuses,
	modified: number,
	drawn: boolean,
): unknown => {
	switch (key[1]) {
		case "alert": {
			const detail = data as JsonConsoleAlertDetail;
			const status = statuses.get(detail.uuid);
			const facts = status
				? { ...detail, status: status as AlertStatus, modified }
				: detail;
			const perf = drawn ? detail.perf : perfWith(detail.perf, statuses);
			return perf === detail.perf ? facts : { ...facts, perf };
		}
		case "alerts": {
			const batch = data as AlertsBatch;
			const alerts = drawn
				? batch.alerts
				: withStatuses(batch.alerts, statuses, modified);
			return alerts === batch.alerts ? batch : { ...batch, alerts };
		}
		// A threshold's own answer has no groups; its batches of alerts do.
		case "threshold":
			return drawn || !(data as JsonConsoleAlerts).groups
				? data
				: withStatuses(data as JsonConsoleAlerts, statuses, modified);
		case "perf":
			return drawn ? data : perfWith(data as JsonConsolePerf, statuses);
		case "plots": {
			const plots = data as JsonConsolePlots;
			const next = drawn
				? plots.plots
				: mapped(plots.plots, (plot) => {
						const perf = perfWith(plot.perf, statuses);
						return perf === plot.perf ? plot : { ...plot, perf };
					});
			return next === plots.plots ? plots : { ...plots, plots: next };
		}
		// A report's batches; its counts are numbers.
		case "report": {
			const batch = data as ReportBatch;
			if (drawn || !batch.report) {
				return data;
			}
			const report = reportWith(batch.report, statuses);
			return report === batch.report ? batch : { ...batch, report };
		}
		// Blank Explore's newest active alerts lose a dismissed one; one made active waits for the next read.
		case "start": {
			if (drawn || key[3] !== "alerts") {
				return data;
			}
			const alerts = data as JsonAlert[];
			const active = alerts.filter(
				({ uuid }) => statuses.get(uuid) !== "dismissed",
			);
			return active.length === alerts.length ? alerts : active;
		}
		default:
			return data;
	}
};

/** The views a change of an alert's status makes stale. */
const STALE = new Set([
	"alert",
	"alerts",
	"perf",
	"plots",
	"report",
	"reports",
	"start",
	"threshold",
	"thresholds",
]);

/** A line's points with each alert in `statuses` at its status; one that holds none keeps its object. */
const seriesWith = (
	series: JsonConsoleSeries,
	statuses: Statuses,
): JsonConsoleSeries =>
	series.alerts.some(({ uuid }) => statuses.has(uuid))
		? {
				...series,
				alerts: series.alerts.map((each) => {
					const status = statuses.get(each.uuid);
					return status ? { ...each, status: status as AlertStatus } : each;
				}),
			}
		: series;

/** Each item through `edit`, and the same array when every item kept its object. */
const mapped = <T>(items: T[], edit: (item: T) => T) => {
	const next = items.map(edit);
	return next.some((item, index) => item !== items[index]) ? next : items;
};

const perfWith = (perf: JsonConsolePerf, statuses: Statuses) => {
	const lines = mapped(perf.lines, (line) => {
		const series = seriesWith(line.series, statuses);
		return series === line.series ? line : { ...line, series };
	});
	return lines === perf.lines ? perf : { ...perf, lines };
};

const reportWith = (report: JsonConsoleReport, statuses: Statuses) => {
	const lines = mapped(report.lines, (line) => {
		const history = seriesWith(line.history, statuses);
		const status = line.alert && statuses.get(line.alert.uuid);
		return history === line.history && !status
			? line
			: {
					...line,
					history,
					...(line.alert &&
						status && {
							alert: { ...line.alert, status: status as AlertStatus },
						}),
				};
	});
	return lines === report.lines ? report : { ...report, lines };
};

type Statuses = ReadonlyMap<string, Settable>;
