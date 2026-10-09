import type {
	JsonConsoleAlerts,
	JsonConsoleThreshold,
	JsonConsoleThresholdRow,
	JsonConsoleThresholds,
} from "../../types/bencher";

const START = Date.parse("2026-09-13T21:16:00Z");
const DAY = 86_400_000;

export const THRESHOLD = "00000000-0000-4000-8000-0000000000a1";

/** A page of thresholds: the seed's Latency threshold, and any rows given after it. */
export const thresholdsFixture = (
	rows: Partial<JsonConsoleThresholdRow>[] = [{}],
	fields: Partial<JsonConsoleThresholds> = {},
): JsonConsoleThresholds =>
	({
		total: rows.length,
		thresholds: rows.map((row, index) => ({
			uuid: `00000000-0000-4000-8000-${String(index + 1).padStart(12, "0")}`,
			branch: 0,
			testbed: 0,
			measure: 0,
			model: {
				test: "t_test",
				min_sample_size: 4,
				max_sample_size: 64,
				upper_boundary: 0.99,
			},
			raised: 2,
			active: 1,
			...row,
		})),
		branches: [
			{ uuid: "main-uuid", name: "main", slug: "main" },
			{
				uuid: "pr-uuid",
				name: "412/merge",
				slug: "412-merge",
				start_point: "main",
				archived: Date.parse("2026-09-08T12:00:00Z"),
			},
		],
		testbeds: [
			{ uuid: "testbed-uuid", name: "ubuntu-latest", slug: "ubuntu-latest" },
		],
		measures: [
			{
				uuid: "latency-uuid",
				name: "Latency",
				slug: "latency",
				units: "nanoseconds (ns)",
			},
			{
				uuid: "throughput-uuid",
				name: "Throughput",
				slug: "throughput",
				units: "bytes per nanosecond",
			},
		],
		...fields,
	}) as JsonConsoleThresholds;

/** The seed's Latency threshold as its page reads it, with one replaced model. */
export const thresholdFixture = (
	fields: Partial<JsonConsoleThreshold> = {},
): JsonConsoleThreshold => {
	const current = {
		uuid: "model-2",
		test: "t_test",
		min_sample_size: 4,
		max_sample_size: 64,
		upper_boundary: 0.99,
		created: Date.parse("2026-08-30T10:00:00Z"),
	} as const;
	return {
		uuid: THRESHOLD,
		branch: { uuid: "main-uuid", name: "main", slug: "main" },
		testbed: {
			uuid: "testbed-uuid",
			name: "ubuntu-latest",
			slug: "ubuntu-latest",
		},
		measure: {
			uuid: "latency-uuid",
			name: "Latency",
			slug: "latency",
			units: "nanoseconds (ns)",
		},
		model: current,
		models: [
			current,
			{
				uuid: "model-1",
				test: "z_score",
				max_sample_size: 30,
				upper_boundary: 0.98,
				created: Date.parse("2026-08-02T10:00:00Z"),
				replaced: Date.parse("2026-08-30T10:00:00Z"),
			},
		],
		created: Date.parse("2026-08-02T10:00:00Z"),
		modified: Date.parse("2026-08-30T10:00:00Z"),
		...fields,
	} as JsonConsoleThreshold;
};

const line = (variant: number, alert: { uuid: string; status: string }) => ({
	benchmark: variant,
	variant,
	measure: 0,
	metric: "value",
	value: 20.6,
	model: 0,
	baseline: 19.4,
	upper_limit: 19.9,
	alert: { ...alert, limit: "upper" },
	history: {
		y: [19.3, 20.6],
		alerts: [{ index: 1, ...alert, limit: "upper" }],
	},
});

/**
 * A page of the threshold's alerts: an active one on blake3 in the newest
 * report, then a dismissed one on sha256 and one on blake3 again in an older one.
 */
export const alertsFixture = (
	fields: Partial<JsonConsoleAlerts> = {},
): JsonConsoleAlerts =>
	({
		total: 3,
		counts: { active: 1, dismissed: 1, silenced: 1 },
		groups: [
			{
				uuid: "report-new",
				branch: 0,
				testbed: 0,
				version: { number: 24, hash: `9c1f2e4${"0".repeat(33)}` },
				start_time: START,
				end_time: START + 120_000,
				created: START + 200_000,
				adapter: "json",
				total: 1,
				window: {
					start_time: START - 28 * DAY,
					end_time: START,
					clamped: false,
				},
				points: { x: [START - DAY, START], report: [0, 1] },
				alerts: [
					{
						line: line(0, { uuid: "alert-1", status: "active" }),
						modified: START,
					},
				],
			},
			{
				uuid: "report-old",
				branch: 0,
				testbed: 0,
				version: { number: 15 },
				start_time: START - 18 * DAY,
				end_time: START - 18 * DAY + 120_000,
				created: START - 18 * DAY,
				adapter: "json",
				total: 2,
				window: {
					start_time: START - 46 * DAY,
					end_time: START - 18 * DAY,
					clamped: false,
				},
				points: { x: [START - 19 * DAY, START - 18 * DAY], report: [2, 3] },
				alerts: [
					{
						line: line(1, { uuid: "alert-2", status: "dismissed" }),
						modified: START - 17 * DAY,
					},
					{
						line: line(0, { uuid: "alert-3", status: "silenced" }),
						modified: START - 17 * DAY,
					},
				],
			},
		],
		reports: [
			{ uuid: "r0", version: 23, hash: "aaaaaaa" },
			{ uuid: "report-new", version: 24, hash: "9c1f2e4" },
			{ uuid: "r2", version: 14 },
			{ uuid: "report-old", version: 15 },
		],
		branches: [{ uuid: "main-uuid", name: "main", slug: "main", head: "head" }],
		testbeds: [
			{ uuid: "testbed-uuid", name: "ubuntu-latest", slug: "ubuntu-latest" },
		],
		benchmarks: [
			{ uuid: "blake3-uuid", name: "blake3", slug: "blake3" },
			{ uuid: "sha256-uuid", name: "sha256", slug: "sha256" },
		],
		variants: [
			{ uuid: "v-blake3", benchmark: 0, parameters: { input_bytes: 65536 } },
			{ uuid: "v-sha256", benchmark: 1, parameters: { input_bytes: 1024 } },
		],
		measures: [
			{
				uuid: "latency-uuid",
				name: "Latency",
				slug: "latency",
				units: "nanoseconds (ns)",
			},
		],
		models: [
			{
				uuid: "model-2",
				threshold: THRESHOLD,
				test: "t_test",
				upper_boundary: 0.99,
			},
		],
		...fields,
	}) as unknown as JsonConsoleAlerts;

/** A batch of `count` active alerts from the `from`th, all in the newest report, out of `total`. */
export const alertsPage = (from: number, count: number, total: number) => {
	const fixture = alertsFixture();
	return {
		...fixture,
		total,
		groups: fixture.groups.slice(0, 1).map((group) => ({
			...group,
			total: count,
			alerts: Array.from({ length: count }, (_, index) => ({
				line: line(0, { uuid: `alert-${from + index}`, status: "active" }),
				modified: START,
			})),
		})),
	} as unknown as JsonConsoleAlerts;
};
