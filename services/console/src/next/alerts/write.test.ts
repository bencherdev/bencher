import { QueryClient, QueryObserver } from "@tanstack/solid-query";
import { afterEach, expect, test } from "vitest";
import type {
	JsonAlert,
	JsonConsoleAlertDetail,
	JsonConsoleAlerts,
	JsonConsolePerf,
} from "../../types/bencher";
import { ALERT, alertDetail, uuid } from "../alert/testing";
import type { Api } from "../api";
import type { ReportBatch } from "../report/query";
import { lineFixture, reportFixture } from "../report/testing";
import { alertsFixture } from "./testing";
import { sendChange, writeStatuses } from "./write";

const unsubscribes: (() => void)[] = [];
afterEach(() => {
	for (const unsubscribe of unsubscribes.splice(0)) {
		unsubscribe();
	}
});

const api = (changed: number): Api => ({
	get: () => new Promise(() => {}),
	send: async <T>() => ({ data: { changed } as T, headers: new Headers() }),
});

/** A page drawing the answer at `key`; the page goes when the result is called. */
const draw = (client: QueryClient, key: readonly unknown[]) => {
	const observer = new QueryObserver(client, {
		queryKey: key,
		queryFn: () => new Promise(() => {}),
		enabled: false,
	});
	const unsubscribe = observer.subscribe(() => {});
	unsubscribes.push(unsubscribe);
	return unsubscribe;
};

/** Long enough for the writer to see a page go. */
const tick = () => new Promise((resolve) => setTimeout(resolve, 0));

const perfStatus = (perf: JsonConsolePerf | undefined) =>
	perf?.lines[0]?.series.alerts[0]?.status;

const alertsWith = (alert: string) => {
	const alerts = alertsFixture([
		{ report: 0, alerts: [{ alert: 1 }, { alert: 2 }] },
	]);
	const group = alerts.groups[0];
	const entry = group?.alerts[0];
	if (!(group && entry?.line.alert)) {
		throw new Error("No alert");
	}
	entry.line.alert = { ...entry.line.alert, uuid: alert };
	return alerts;
};
const alertStatus = (alerts: JsonConsoleAlerts | undefined) =>
	alerts?.groups[0]?.alerts[0]?.line.alert?.status;

// Kills a drawn plot or list rewritten under its reader (which redraws it and
// drops a pinned readout), an answer no page draws left with the old status,
// another project's answers touched, and a drawn alert page whose own facts
// stay behind.
test("writes the status into every answer no page draws, and the alert's own facts", async () => {
	const client = new QueryClient();
	const detail = alertDetail();
	const keys = {
		alert: ["console", "alert", "hashbrown", ALERT],
		drawnPerf: ["console", "perf", "hashbrown", "drawn"],
		perf: ["console", "perf", "hashbrown", "kept"],
		otherPerf: ["console", "perf", "rosti", "kept"],
		alerts: ["console", "alerts", "hashbrown", "", 8, 0, 0],
		drawnAlerts: ["console", "alerts", "hashbrown", "status=all", 8, 0, 0],
		thresholdAlerts: ["console", "threshold", "hashbrown", uuid(3), "alerts"],
		report: ["console", "report", "hashbrown", uuid(2), "", 8, 1],
		start: ["console", "start", "hashbrown", "alerts"],
	} as const;
	const alerted = { uuid: ALERT, limit: "upper", status: "active" } as const;
	client.setQueryData(keys.report, {
		report: reportFixture([
			lineFixture({
				alert: alerted as never,
				history: { alerts: [{ index: 2, ...alerted }] as never },
			}),
		]),
		group: "none",
	});
	client.setQueryData(keys.start, [{ uuid: ALERT }, { uuid: uuid(9) }]);
	client.setQueryData(keys.alert, detail);
	client.setQueryData(keys.drawnPerf, detail.perf);
	client.setQueryData(keys.perf, detail.perf);
	client.setQueryData(keys.otherPerf, detail.perf);
	client.setQueryData(keys.alerts, { alerts: alertsWith(ALERT), bounds: {} });
	client.setQueryData(keys.drawnAlerts, {
		alerts: alertsWith(ALERT),
		bounds: {},
	});
	client.setQueryData(keys.thresholdAlerts, alertsWith(ALERT));
	draw(client, keys.alert);
	draw(client, keys.drawnPerf);
	draw(client, keys.drawnAlerts);

	expect(
		await sendChange(api(1), client, "hashbrown", ALERT, "dismissed"),
	).toBe(1);

	const written = client.getQueryData<JsonConsoleAlertDetail>(keys.alert);
	expect(written?.status).toBe("dismissed");
	expect(written?.perf).toBe(detail.perf);
	expect(client.getQueryData(keys.drawnPerf)).toBe(detail.perf);
	expect(perfStatus(client.getQueryData(keys.perf))).toBe("dismissed");
	expect(client.getQueryData(keys.otherPerf)).toBe(detail.perf);
	expect(
		alertStatus(
			client.getQueryData<{ alerts: JsonConsoleAlerts }>(keys.alerts)?.alerts,
		),
	).toBe("dismissed");
	expect(
		alertStatus(
			client.getQueryData<{ alerts: JsonConsoleAlerts }>(keys.drawnAlerts)
				?.alerts,
		),
	).toBe("active");
	expect(
		alertStatus(client.getQueryData<JsonConsoleAlerts>(keys.thresholdAlerts)),
	).toBe("dismissed");
	const line = client.getQueryData<ReportBatch>(keys.report)?.report.lines[0];
	expect([line?.alert?.status, line?.history.alerts[0]?.status]).toEqual([
		"dismissed",
		"dismissed",
	]);
	expect(
		client.getQueryData<JsonAlert[]>(keys.start)?.map(({ uuid }) => uuid),
	).toEqual([uuid(9)]);
});

// Kills a drawn answer never written once its page goes (so a return to the
// page draws the old status until it reads again), one written while another
// page still draws it, one written over a newer read, and one left fresh.
test("writes a drawn answer once no page draws it, unless it read again", async () => {
	const client = new QueryClient();
	const detail = alertDetail();
	const keys = {
		alert: ["console", "alert", "hashbrown", ALERT],
		perf: ["console", "perf", "hashbrown", "left"],
		shared: ["console", "perf", "hashbrown", "shared"],
		reread: ["console", "perf", "hashbrown", "reread"],
	} as const;
	for (const key of Object.values(keys)) {
		client.setQueryData(key, key === keys.alert ? detail : detail.perf);
	}
	const leaves = [keys.alert, keys.perf, keys.shared, keys.reread].map((key) =>
		draw(client, key),
	);
	const stays = draw(client, keys.shared);

	await sendChange(api(1), client, "hashbrown", ALERT, "dismissed");
	// The API's answer, read again while the page still draws it.
	client.setQueryData(keys.reread, { ...detail.perf });
	for (const leave of leaves) {
		leave();
	}
	await tick();

	const written = client.getQueryData<JsonConsoleAlertDetail>(keys.alert);
	expect(written?.status).toBe("dismissed");
	expect(perfStatus(written?.perf)).toBe("dismissed");
	expect(perfStatus(client.getQueryData(keys.perf))).toBe("dismissed");
	expect(client.getQueryState(keys.perf)?.isInvalidated).toBe(true);
	expect(client.getQueryData(keys.shared)).toBe(detail.perf);
	expect(perfStatus(client.getQueryData(keys.reread))).toBe("active");

	stays();
	await tick();
	expect(perfStatus(client.getQueryData(keys.shared))).toBe("dismissed");
});

// Kills a drawn answer that keeps only the first of the changes waiting on
// it, taking that change's write for a read: a Dismiss then a Reactivate, or
// two alerts changed on one plot, would return as the first change alone.
test("writes every change waiting on a drawn answer once its page goes", async () => {
	const client = new QueryClient();
	const { perf } = alertDetail();
	const [line] = perf.lines;
	const [point] = line?.series.alerts ?? [];
	if (!(line && point)) {
		throw new Error("No alert");
	}
	const other = uuid(8);
	const twice = ["console", "perf", "hashbrown", "twice"];
	const both = ["console", "perf", "hashbrown", "both"];
	client.setQueryData(twice, perf);
	client.setQueryData(both, {
		...perf,
		lines: [
			{
				...line,
				series: {
					...line.series,
					alerts: [point, { ...point, index: point.index - 1, uuid: other }],
				},
			},
		],
	});
	const leaves = [draw(client, twice), draw(client, both)];

	await sendChange(api(1), client, "hashbrown", ALERT, "dismissed");
	await sendChange(api(1), client, "hashbrown", ALERT, "active");
	await sendChange(api(1), client, "hashbrown", other, "dismissed");
	for (const leave of leaves) {
		leave();
	}
	await tick();

	expect(perfStatus(client.getQueryData(twice))).toBe("active");
	expect(
		client
			.getQueryData<JsonConsolePerf>(both)
			?.lines[0]?.series.alerts.map(({ status }) => status),
	).toEqual(["active", "dismissed"]);
});

// Kills a list page's change written again into its own batches, whose rows
// it keeps, and every other answer left with the old status.
test("leaves the kind a page writes itself", async () => {
	const client = new QueryClient();
	const detail = alertDetail();
	const alerts = { alerts: alertsWith(ALERT), bounds: {} };
	const keys = {
		alerts: ["console", "alerts", "hashbrown", "", 8, 0, 0],
		perf: ["console", "perf", "hashbrown", "q"],
	} as const;
	client.setQueryData(keys.alerts, alerts);
	client.setQueryData(keys.perf, detail.perf);
	writeStatuses(
		client,
		"hashbrown",
		new Map([[ALERT, "dismissed"]]),
		Date.now(),
		"alerts",
	);
	expect(client.getQueryData(keys.alerts)).toBe(alerts);
	expect(perfStatus(client.getQueryData(keys.perf))).toBe("dismissed");
});

// Kills a view of alerts left fresh after the change, which a return to it
// would draw from the cache without reading again, and another project's
// views made stale.
test("marks every view of the project's alerts stale, and nothing else", async () => {
	const client = new QueryClient();
	const detail = alertDetail();
	const answers: [unknown[], unknown][] = [
		[["console", "alert", "hashbrown", ALERT], detail],
		[["console", "alerts", "hashbrown", ""], { alerts: alertsWith(ALERT) }],
		[["console", "perf", "hashbrown", "q"], detail.perf],
		[["console", "plots", "hashbrown", null, 8, 1], { order: [], plots: [] }],
		[["console", "report", "hashbrown", uuid(2)], {}],
		[["console", "reports", "hashbrown", ""], []],
		[["console", "run", "hashbrown", uuid(5), 32, 1], {}],
		[["console", "runs", "hashbrown", "", 32, 1], { runs: [], total: 0 }],
		[["console", "start", "hashbrown", "alerts"], []],
		[["console", "threshold", "hashbrown", uuid(3)], {}],
		[["console", "thresholds", "hashbrown", ""], []],
	];
	const stale = answers.map(([key]) => key);
	const fresh = [
		["console", "project", "hashbrown"],
		["console", "variants", "hashbrown", uuid(4)],
		["console", "alerts", "rosti", ""],
	];
	for (const [key, data] of answers) {
		client.setQueryData(key, data);
	}
	for (const key of fresh) {
		client.setQueryData(key, []);
	}
	await sendChange(api(1), client, "hashbrown", ALERT, "dismissed");
	expect(stale.map((key) => client.getQueryState(key)?.isInvalidated)).toEqual(
		stale.map(() => true),
	);
	expect(fresh.map((key) => client.getQueryState(key)?.isInvalidated)).toEqual(
		fresh.map(() => false),
	);
});

// Kills writing a change the API did not make.
test("writes nothing when the API changed nothing", async () => {
	const client = new QueryClient();
	const detail = alertDetail();
	const key = ["console", "alert", "hashbrown", ALERT];
	client.setQueryData(key, detail);
	expect(
		await sendChange(api(0), client, "hashbrown", ALERT, "dismissed"),
	).toBe(0);
	expect(client.getQueryData(key)).toBe(detail);
	expect(client.getQueryState(key)?.isInvalidated).toBe(false);
});
