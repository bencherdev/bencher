import type {
	JsonConsoleAlertGroup,
	JsonConsoleAlertLine,
	JsonConsoleAlerts,
} from "../../types/bencher";

const START = Date.parse("2026-09-13T21:16:00Z");
const DAY = 86_400_000;
/** When the API read every fixture list: an hour after its newest report. */
export const READ = START + 3_600_000;

/** A report's UUID by its number. */
export const reportId = (index: number) =>
	`00000000-0000-4000-8000-${String(index).padStart(12, "0")}`;
/** An alert's UUID by its number. */
export const alertId = (index: number) =>
	`00000000-0000-4000-8000-a${String(index).padStart(11, "0")}`;

interface AlertFields {
	alert: number;
	status?: "active" | "dismissed" | "silenced";
	/** An index into the fixture's benchmarks: blake3, then sha256. */
	benchmark?: number;
	modified?: number;
}

/** One alert as the list returns it: a latency line over three points, its value over its upper limit. */
const alertFixture = ({
	alert,
	status = "active",
	benchmark = 0,
	modified = START,
}: AlertFields): JsonConsoleAlertLine =>
	({
		line: {
			benchmark,
			variant: benchmark,
			measure: 0,
			metric: "value",
			value: 20.6,
			model: 0,
			baseline: 19.4,
			upper_limit: 19.9,
			alert: { uuid: alertId(alert), limit: "upper", status },
			history: {
				y: [19.3, 19.5, 20.6],
				upper: [19.9, 19.9, 19.9],
				alerts: [{ index: 2, uuid: alertId(alert), limit: "upper", status }],
			},
		},
		modified,
	}) as JsonConsoleAlertLine;

interface GroupFields {
	report: number;
	alerts: AlertFields[];
	/** An index into the fixture's branches: main, then feature. */
	branch?: number;
	/** How many hours before the seed's newest report it ran. */
	hours?: number;
	total?: number;
}

/** One report's alerts on a page. */
const groupFixture = ({
	report,
	alerts,
	branch = 0,
	hours = 0,
	total = alerts.length,
}: GroupFields): JsonConsoleAlertGroup => {
	const start = START - hours * 3_600_000;
	return {
		uuid: reportId(report),
		branch,
		testbed: 0,
		version: { number: 24, hash: `9c1f2e4${"0".repeat(33)}` },
		start_time: start,
		end_time: start + 120_000,
		created: start + 125_000,
		adapter: "json",
		total,
		window: { start_time: start - 28 * DAY, end_time: start, clamped: false },
		points: { x: [start - 2 * DAY, start - DAY, start], report: [0, 1, 2] },
		alerts: alerts.map(alertFixture),
	} as JsonConsoleAlertGroup;
};

/** A page of the alerts list over the fixture's tables. */
export const alertsFixture = (
	groups: GroupFields[],
	fields: Partial<JsonConsoleAlerts> = {},
): JsonConsoleAlerts => {
	const all = groups.flatMap(({ alerts }) => alerts);
	const count = (status: string) =>
		all.filter((alert) => (alert.status ?? "active") === status).length;
	return {
		total: all.length,
		counts: {
			active: count("active"),
			dismissed: count("dismissed"),
			silenced: count("silenced"),
		},
		read_time: READ,
		groups: groups.map(groupFixture),
		reports: [
			{ uuid: "r0", version: 22, hash: "aaaaaaa" },
			{ uuid: "r1", version: 23, hash: "bbbbbbb" },
			{ uuid: reportId(0), version: 24, hash: "9c1f2e4" },
		],
		branches: [
			{ uuid: "main-uuid", name: "main", slug: "main", head: "main-head" },
			{
				uuid: "feature-uuid",
				name: "feature",
				slug: "feature",
				head: "feature-head",
			},
		],
		testbeds: [
			{ uuid: "testbed-uuid", name: "ubuntu-latest", slug: "ubuntu-latest" },
		],
		benchmarks: [
			{ uuid: "blake3-uuid", name: "blake3", slug: "blake3" },
			{ uuid: "sha256-uuid", name: "sha256", slug: "sha256" },
		],
		variants: [
			{ uuid: "v-blake3", benchmark: 0, parameters: { n: 0 } },
			{ uuid: "v-sha256", benchmark: 1, parameters: { n: 0 } },
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
				uuid: "model",
				threshold: "threshold",
				test: "static",
				upper_boundary: 19.9,
			},
		],
		...fields,
	} as JsonConsoleAlerts;
};
