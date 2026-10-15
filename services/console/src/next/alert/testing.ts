import {
	Adapter,
	AlertStatus,
	BoundaryLimit,
	type JsonConsoleAlertDetail,
	ModelTest,
} from "../../types/bencher";

const DAY = 86_400_000;
const START = Date.UTC(2026, 8, 15, 21, 6);

export const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

export const ALERT = uuid(1);
export const REPORT = uuid(2);
export const THRESHOLD = uuid(3);

/** An alert on blake3's last point of five, a day apart, over its upper limit, unless `detail` says otherwise. */
export const alertDetail = (
	detail: Partial<JsonConsoleAlertDetail> = {},
): JsonConsoleAlertDetail => ({
	uuid: ALERT,
	limit: BoundaryLimit.Upper,
	status: AlertStatus.Active,
	created: START + 4 * DAY + 60_000,
	modified: START + 4 * DAY + 60_000,
	report: {
		uuid: REPORT,
		start_time: START + 4 * DAY,
		hash: "9c1f2e4a0b",
		adapter: Adapter.Json,
	},
	value: 20_620,
	baseline: 18_170,
	upper_limit: 18_730,
	perf: {
		window: {
			start_time: START - 24 * DAY,
			end_time: START + 4 * DAY + 60_000,
			clamped: false,
		},
		total: 1,
		lines: [
			{
				branch: 0,
				testbed: 0,
				benchmark: 0,
				variant: 0,
				measure: 0,
				metric: "value",
				model: 0,
				series: {
					y: [18_100, 18_300, 18_000, 18_200, 20_620],
					baseline: [null, 18_100, 18_200, 18_130, 18_170],
					upper: [null, 18_600, 18_700, 18_650, 18_730],
					alerts: [
						{
							index: 4,
							uuid: ALERT,
							limit: BoundaryLimit.Upper,
							status: detail.status ?? AlertStatus.Active,
						},
					],
				},
			},
		],
		points: {
			x: Array.from({ length: 5 }, (_, index) => START + index * DAY),
			report: [0, 1, 2, 3, 4],
		},
		reports: Array.from({ length: 5 }, (_, index) => ({
			uuid: index === 4 ? REPORT : uuid(10 + index),
			version: index + 1,
			hash: index === 4 ? "9c1f2e4a0b" : `aaaaaa${index}`,
		})),
		branches: [{ uuid: uuid(20), name: "main", slug: "main", head: uuid(21) }],
		testbeds: [
			{ uuid: uuid(22), name: "ubuntu-latest", slug: "ubuntu-latest" },
		],
		benchmarks: [{ uuid: uuid(23), name: "blake3", slug: "blake3" }],
		variants: [
			{
				uuid: uuid(24),
				benchmark: 0,
				parameters: { simd: "avx2", input: "64KiB", threads: 1 },
			},
		],
		measures: [
			{
				uuid: uuid(25),
				name: "Latency",
				slug: "latency",
				units: "nanoseconds (ns)",
			},
		],
		models: [
			{
				uuid: uuid(26),
				threshold: THRESHOLD,
				test: ModelTest.TTest,
				min_sample_size: 5,
				upper_boundary: 0.99,
			},
		],
	},
	...detail,
});

/** An Explore answer whose one alerting point is `alert`'s. */
export const perfHolding = (alert: string) => {
	const { perf } = alertDetail();
	return {
		...perf,
		lines: perf.lines.map((line) => ({
			...line,
			series: {
				...line.series,
				alerts: line.series.alerts.map((each) => ({ ...each, uuid: alert })),
			},
		})),
	};
};

/** The status of the first alerting point an Explore answer holds. */
export const perfStatusOf = (perf: unknown) =>
	(perf as ReturnType<typeof perfHolding> | undefined)?.lines[0]?.series
		.alerts[0]?.status;
