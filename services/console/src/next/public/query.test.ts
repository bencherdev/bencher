import { describe, expect, test } from "vitest";
import { decodeQuery } from "../query/query";
import { publicPlotHref, publicQuery, publicWindow } from "./query";

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
			publicPlotHref("hashbrown", `?${classic}`, NOW),
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

	// Kills a classic range that reaches the public plot unchanged, which no
	// Window position names and the API cuts short without saying.
	test("a classic range becomes the public window that holds it", () => {
		const href = publicPlotHref(
			"hashbrown",
			`?start_time=${NOW - 10 * DAY * 1000}&end_time=${NOW}`,
			NOW,
		);
		expect(decodeQuery(new URL(href, "https://bencher.dev").search).window).toEqual({
			seconds: 28 * DAY,
			end: NOW,
		});
	});
});

describe("publicQuery", () => {
	// Kills a long window drawn past the reach the public plot offers.
	test("a window past three months comes back as three months", () => {
		expect(publicQuery("?window=26w", NOW).window).toEqual({
			seconds: 92 * DAY,
		});
		expect(
			publicQuery(`?start_time=${NOW - 365 * DAY * 1000}`, NOW).window,
		).toEqual({ seconds: 92 * DAY });
	});
});

describe("publicWindow", () => {
	// Kills always taking the longest or the shortest preset, a window moved
	// off where it ended, and a preset rewritten.
	test("the smallest preset that holds the window, ending where it ended", () => {
		expect(publicWindow({ seconds: 3 * DAY }, NOW)).toEqual({
			seconds: 7 * DAY,
		});
		expect(publicWindow({ seconds: 7 * DAY, end: NOW }, NOW)).toEqual({
			seconds: 7 * DAY,
			end: NOW,
		});
		expect(publicWindow({ seconds: 8 * DAY }, NOW)).toEqual({
			seconds: 28 * DAY,
		});
		expect(
			publicWindow({ start: NOW - 40 * DAY * 1000, end: NOW - DAY }, NOW),
		).toEqual({ seconds: 92 * DAY, end: NOW - DAY });
		expect(publicWindow({ start: NOW - 5 * DAY * 1000 }, NOW)).toEqual({
			seconds: 7 * DAY,
		});
		expect(
			publicWindow({ start: NOW - 30 * DAY * 1000, end: NOW - 25 * DAY * 1000 }, NOW),
		).toEqual({ seconds: 7 * DAY, end: NOW - 25 * DAY * 1000 });
	});
});
