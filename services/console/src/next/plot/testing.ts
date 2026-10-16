import type { PlotData, PlotLine } from "./types";

export const DAY = 86_400_000;
export const START = Date.UTC(2026, 8, 1, 12);

export const testLine = (
	line: Partial<PlotLine> & Pick<PlotLine, "id">,
): PlotLine => ({
	benchmark: "blake3",
	parameters: {},
	measure: 0,
	metric: "value",
	branch: "main",
	testbed: "linux",
	alerting: false,
	y: [],
	alerts: [],
	...line,
});

/** One report a day from `START`, versions counting up from 100. */
export const testData = (
	lines: PlotLine[],
	measures: PlotData["measures"] = [
		{ name: "Latency", units: "nanoseconds (ns)" },
	],
): PlotData => {
	const points = Math.max(0, ...lines.map((line) => line.y.length));
	const x = Array.from({ length: points }, (_, index) => START + index * DAY);
	return {
		x,
		report: x.map((_, index) => index),
		reports: x.map((_, index) => ({
			uuid: `report-${index}`,
			version: 100 + index,
			hash: `${index}`.padStart(7, "a"),
		})),
		measures,
		lines,
	};
};

// One report run once, then two run three times; the last raised an alert on its second iteration.
export const RUNS: PlotData = {
	x: [START, START + DAY, START + DAY, START + DAY].concat(
		Array(3).fill(START + 2 * DAY),
	),
	report: [0, 1, 1, 1, 2, 2, 2],
	iteration: [0, 0, 1, 2, 0, 1, 2],
	reports: [0, 1, 2].map((index) => ({
		uuid: `report-${index}`,
		version: 100 + index,
	})),
	measures: [{ name: "Latency", units: "nanoseconds (ns)" }],
	lines: [
		testLine({
			id: "a",
			y: [10, 11, 14, 12, 10, 13, 9],
			upper: [15, 15, 15, 15, 12, 12, 12],
			alerts: [5],
			raised: [{ index: 5, uuid: "alert", status: "dismissed" }],
		}),
	],
};
