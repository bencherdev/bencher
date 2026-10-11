import {
	AlertStatus,
	BoundaryLimit,
	type JsonConsolePlot,
	type JsonPlot,
	type PlotLayout,
	XAxis,
	YAxis,
} from "../../types/bencher";

const DAY = 86_400_000;
const START = Date.UTC(2026, 8, 1, 12);

/** A pin drawing `lines` variants of one benchmark over `points` daily reports. */
export const testTile = ({
	uuid = "plot-1",
	title = "blake3, Latency on main",
	lines = 2,
	points = 5,
	alerting = false,
	window = 28 * 86_400,
	measures = 1,
	layout,
}: {
	uuid?: string;
	title?: string;
	lines?: number;
	points?: number;
	alerting?: boolean;
	window?: number;
	measures?: number;
	layout?: PlotLayout;
} = {}): JsonConsolePlot => {
	const x = Array.from({ length: points }, (_, index) => START + index * DAY);
	const plot: JsonPlot = {
		uuid,
		project: "project-1",
		title,
		lower_value: false,
		upper_value: false,
		lower_boundary: false,
		upper_boundary: false,
		x_axis: XAxis.DateTime,
		y_axis: YAxis.Auto,
		...(layout === undefined ? {} : { layout }),
		window,
		branches: ["branch-1"],
		testbeds: ["testbed-1"],
		benchmarks: ["benchmark-1"],
		measures: Array.from(
			{ length: measures },
			(_, index) => `measure-${index}`,
		),
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	};
	return {
		plot,
		perf: {
			window: {
				start_time: x[0] ?? START,
				end_time: START + 30 * DAY,
				clamped: false,
			},
			total: lines * measures,
			lines: Array.from({ length: lines * measures }, (_, index) => ({
				branch: 0,
				testbed: 0,
				benchmark: 0,
				variant: index % lines,
				measure: Math.floor(index / lines),
				metric: "value",
				series: {
					y: x.map((_, point) => 10 + index + point),
					alerts:
						alerting && index === 0
							? [
									{
										index: points - 1,
										uuid: "alert-1",
										limit: BoundaryLimit.Upper,
										status: AlertStatus.Active,
									},
								]
							: [],
				},
			})),
			points: { x, report: x.map((_, index) => index) },
			reports: x.map((_, index) => ({
				uuid: `report-${index}`,
				version: index + 1,
			})),
			branches: [
				{ uuid: "branch-1", name: "main", slug: "main", head: "head-1" },
			],
			testbeds: [
				{ uuid: "testbed-1", name: "ubuntu-latest", slug: "ubuntu-latest" },
			],
			benchmarks: [{ uuid: "benchmark-1", name: "blake3", slug: "blake3" }],
			variants: Array.from({ length: lines }, (_, index) => ({
				uuid: `variant-${index}`,
				benchmark: 0,
				parameters: { size: index + 1 },
			})),
			measures: Array.from({ length: measures }, (_, index) => ({
				uuid: `measure-${index}`,
				name: index === 0 ? "Latency" : `Measure ${index}`,
				slug: `measure-${index}`,
				units: "nanoseconds (ns)",
			})),
			models: [],
		},
	};
};
