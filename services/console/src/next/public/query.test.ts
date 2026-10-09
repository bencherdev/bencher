import { describe, expect, test } from "vitest";
import { decodeQuery } from "../query/query";
import { publicPlotHref } from "./query";

const DAY = 24 * 60 * 60;
const NOW = Date.UTC(2026, 8, 14);

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

describe("publicPlotHref", () => {
	// Kills a classic link carried over as it was, keeping the classic page's
	// own paging and toggles, or one that loses its boxes or its view.
	test("a classic perf link keeps its lines and view under Explore's names", () => {
		const classic = new URLSearchParams({
			branches: uuid(1),
			heads: uuid(11),
			testbeds: uuid(3),
			benchmarks: `${uuid(5)},${uuid(6)}`,
			measures: uuid(7),
			x_axis: "version",
			y_axis: "log",
			tab: "benchmarks",
			key: "true",
			lower_boundary: "true",
			reports_page: "2",
		});
		const href = new URL(
			publicPlotHref("hashbrown", `?${classic}`),
			"https://bencher.dev",
		);
		expect(href.pathname).toBe("/next/perf/hashbrown");
		expect([...href.searchParams.keys()].sort()).toEqual([
			"benchmarks",
			"branches",
			"heads",
			"measures",
			"testbeds",
			"x_axis",
			"y_axis",
		]);
		expect(decodeQuery(href.search)).toMatchObject({
			branches: [{ uuid: uuid(1), head: uuid(11) }],
			testbeds: [{ uuid: uuid(3) }],
			benchmarks: [uuid(5), uuid(6)],
			measures: [uuid(7)],
			xAxis: "version",
			yScale: "log",
			window: { seconds: 28 * DAY },
		});
	});

	// Kills a link's own range moved to a preset, so a pull request's link
	// draws another window than its run's, and a range cut short here rather
	// than by the API, which says so.
	test("a classic range keeps its own start and end, past three months too", () => {
		for (const days of [10, 120]) {
			const start = NOW - days * DAY * 1000;
			const href = publicPlotHref(
				"hashbrown",
				`?start_time=${start}&end_time=${NOW - DAY * 1000}`,
			);
			expect(
				decodeQuery(new URL(href, "https://bencher.dev").search).window,
			).toEqual({ start, end: NOW - DAY * 1000 });
		}
	});
});
