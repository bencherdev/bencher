import { describe, expect, test } from "vitest";
import {
	type JsonConsolePerf,
	type JsonPlot,
	PlotLayout,
	XAxis,
	YAxis,
} from "../../types/bencher";
import { type ExploreQuery, blankQuery, decodeQuery } from "../query/query";
import {
	newPlot,
	pinTitle,
	plotPatch,
	plotSearch,
	queryFromPlot,
	shownAmong,
	unsaved,
} from "./pinned";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const DAY = 24 * 60 * 60;
const NOW = 1_789_344_000_000;

const PLOT: JsonPlot = {
	uuid: uuid(50),
	project: uuid(51),
	title: "blake3 size=64, Latency on main",
	lower_value: false,
	upper_value: false,
	lower_boundary: false,
	upper_boundary: false,
	x_axis: XAxis.Version,
	y_axis: YAxis.Log,
	layout: PlotLayout.Stacked,
	window: 7 * DAY,
	branches: [uuid(1)],
	testbeds: [uuid(3)],
	benchmarks: [uuid(5), uuid(6)],
	parameters: [{ size: 64 }],
	measures: [uuid(7), uuid(8)],
	metrics: ["value"],
	hidden: ["k1"],
	focus: "k2",
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
};

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1), head: uuid(11) }],
	testbeds: [{ uuid: uuid(3), spec: uuid(13) }],
	benchmarks: [uuid(5)],
	sets: [{ size: 64 }],
	measures: [uuid(7)],
	metrics: ["value"],
	window: { start: NOW - 3 * DAY * 1000, end: NOW },
	hide: ["k1", "gone"],
	focus: "k2",
};

describe("queryFromPlot", () => {
	// Kills dropping or misreading any field a pin saves.
	test("read every field a pin saves", () => {
		expect(queryFromPlot(PLOT)).toEqual({
			...blankQuery(),
			branches: [{ uuid: uuid(1) }],
			testbeds: [{ uuid: uuid(3) }],
			benchmarks: [uuid(5), uuid(6)],
			sets: [{ size: 64 }],
			measures: [uuid(7), uuid(8)],
			metrics: ["value"],
			xAxis: "version",
			yScale: "log",
			window: { seconds: 7 * DAY },
			layout: "stacked",
			hide: ["k1"],
			focus: "k2",
			plot: uuid(50),
		});
	});

	// Kills reading an absent filter, list, or layout as something narrower.
	test("read what a pin leaves out as the defaults", () => {
		const {
			parameters: _parameters,
			metrics: _metrics,
			hidden: _hidden,
			focus: _focus,
			layout: _layout,
			...bare
		} = PLOT;
		const query = queryFromPlot({
			...bare,
			x_axis: XAxis.DateTime,
			y_axis: YAxis.Auto,
		});
		expect(query).toMatchObject({
			sets: [],
			metrics: [],
			hide: [],
			layout: "dual",
			xAxis: "date",
			yScale: "auto",
		});
		expect(query.focus).toBeUndefined();
	});
});

describe("plotSearch", () => {
	// Kills opening a pin without its query or without naming the pin it edits.
	test("open a pin as its query, editing that pin", () => {
		expect(decodeQuery(plotSearch(PLOT))).toEqual(queryFromPlot(PLOT));
	});
});

describe("newPlot", () => {
	// Kills a pin that loses the view, saves a stale or `only` key, or a window that is not rolling.
	test("pin the query with its view, its hidden lines among those drawn, and a rolling window", () => {
		const drawn = ["k1", "k2", "k3"];
		expect(
			newPlot(
				{
					...QUERY,
					only: ["k2"],
					xAxis: "version",
					yScale: "log",
					layout: "stacked",
				},
				"Title",
				drawn,
				NOW,
			),
		).toEqual({
			title: "Title",
			lower_value: false,
			upper_value: false,
			lower_boundary: false,
			upper_boundary: false,
			x_axis: XAxis.Version,
			y_axis: YAxis.Log,
			layout: PlotLayout.Stacked,
			window: 3 * DAY,
			branches: [uuid(1)],
			testbeds: [uuid(3)],
			benchmarks: [uuid(5)],
			parameters: [{ size: 64 }],
			measures: [uuid(7)],
			metrics: ["value"],
			hidden: ["k1", "k3"],
			focus: "k2",
		});
	});

	// Kills sending a focus the query does not have.
	test("leave the focus out when there is none", () => {
		const { focus: _focus, ...unfocused } = QUERY;
		expect(newPlot(unfocused, "Title", [], NOW)).not.toHaveProperty("focus");
	});
});

describe("plotPatch", () => {
	// Kills a save that cannot clear the filter, the metrics, the hidden lines, or the focus.
	test("save an empty box or no focus as a clear", () => {
		const { focus: _focus, ...unfocused } = QUERY;
		expect(
			plotPatch({ ...unfocused, sets: [], metrics: [], hide: [] }, ["k1"], NOW),
		).toMatchObject({
			parameters: [],
			metrics: [],
			hidden: [],
			focus: null,
			window: 3 * DAY,
		});
	});
});

describe("unsaved", () => {
	const opened = queryFromPlot(PLOT);
	const drawn = ["k1", "k2", "k3"];

	// Kills a pin marked changed the moment it opens, or by a key it never drew.
	test("match the pin as it opens, whatever order or stale keys its hidden lines hold", () => {
		expect(unsaved(opened, PLOT, drawn)).toBe(false);
		expect(unsaved({ ...opened, hide: ["k1", "gone"] }, PLOT, drawn)).toBe(
			false,
		);
		expect(unsaved({ ...opened, report: uuid(9) }, PLOT, drawn)).toBe(false);
	});

	// Kills an edit that leaves the mark off.
	test("differ once a box, the view, or the key changes", () => {
		for (const change of [
			{ benchmarks: [uuid(5)] },
			{ sets: [] },
			{ metrics: [] },
			{ window: { seconds: DAY } },
			{ xAxis: "date" },
			{ yScale: "auto" },
			{ layout: "dual" },
			{ hide: [] },
			{ hide: ["k1", "k3"] },
			{ focus: "k3" },
		] as const) {
			expect(unsaved({ ...opened, ...change }, PLOT, drawn)).toBe(true);
		}
	});
});

describe("pinTitle", () => {
	const PERF: JsonConsolePerf = {
		window: { start_time: 0, end_time: 1, clamped: false },
		total: 2,
		points: { x: [1], report: [0] },
		reports: [{ uuid: uuid(20), version: 1 }],
		branches: [{ uuid: uuid(1), name: "main", slug: "main", head: uuid(11) }],
		testbeds: [{ uuid: uuid(3), name: "linux", slug: "linux" }],
		benchmarks: [{ uuid: uuid(5), name: "blake3", slug: "blake3" }],
		variants: [{ uuid: uuid(30), benchmark: 0, parameters: { size: 64 } }],
		measures: [
			{ uuid: uuid(7), name: "Latency", slug: "latency", units: "ns" },
		],
		models: [],
		lines: ["value", "p99"].map((metric) => ({
			branch: 0,
			testbed: 0,
			benchmark: 0,
			variant: 0,
			measure: 0,
			metric,
			series: { y: [1], alerts: [] },
		})),
	};

	// Kills naming the parameters from the drawn variants alone, which hides what varies.
	test("name the parameters that vary across the benchmark's variants", () => {
		const variants = new Map([[uuid(5), [{ size: 64 }, { size: 1024 }]]]);
		expect(pinTitle(PERF, ({ metric }) => metric === "value", variants)).toBe(
			"blake3 size=64, Latency on main",
		);
	});

	// Kills a title naming a line the query hides.
	test("show the drawn lines the query does not hide", () => {
		const shown = shownAmong({ ...QUERY, hide: ["p99-key"] }, [
			"value-key",
			"p99-key",
		]);
		expect(PERF.lines.map(shown)).toEqual([true, false]);
	});

	// Kills naming hidden lines, or leaving out the metric when the shown lines differ in it.
	test("name the shown lines, with the metric when they differ in it", () => {
		const shown =
			(metrics: string[]) =>
			({ metric }: { metric: string }) =>
				metrics.includes(metric);
		const variants = new Map([[uuid(5), [{ size: 64 }]]]);
		expect(pinTitle(PERF, shown(["p99"]), variants)).toBe(
			"blake3, Latency on main",
		);
		expect(pinTitle(PERF, shown(["value", "p99"]), variants)).toBe(
			"blake3, value and p99 Latency on main",
		);
	});
});
