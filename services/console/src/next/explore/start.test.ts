import { describe, expect, test } from "vitest";
import {
	AlertStatus,
	BoundaryLimit,
	type JsonAlert,
} from "../../types/bencher";
import { alertLine } from "./start";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const named = (n: number, name: string) => ({
	uuid: uuid(n),
	project: uuid(99),
	name,
	slug: name,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
});

const ALERT = {
	uuid: uuid(60),
	report: uuid(61),
	iteration: 0,
	benchmark: named(5, "blake3"),
	variant: {
		uuid: uuid(30),
		benchmark: uuid(5),
		parameters: { size: 64 },
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	},
	value: 20.6,
	threshold: {
		uuid: uuid(40),
		project: uuid(99),
		branch: { ...named(1, "main"), head: { uuid: uuid(11) } },
		testbed: named(3, "linux"),
		measure: { ...named(7, "Latency"), units: "ns" },
		created: "2026-09-01T00:00:00Z",
		modified: "2026-09-01T00:00:00Z",
	},
	boundary: { baseline: 18.1, upper_limit: 19.4 },
	limit: BoundaryLimit.Upper,
	status: AlertStatus.Active,
	created: "2026-09-13T21:16:00Z",
	modified: "2026-09-13T21:16:00Z",
} as unknown as JsonAlert;

describe("alertLine", () => {
	// Kills reading a dimension from the wrong part of the alert, or a metric other than the threshold's.
	test("name the line an alert fired on", () => {
		expect(alertLine(ALERT)).toEqual({
			branch: { uuid: uuid(1) },
			testbed: { uuid: uuid(3) },
			benchmark: uuid(5),
			parameters: { size: 64 },
			measure: uuid(7),
			metric: "value",
			alerting: true,
		});
		expect(
			alertLine({
				...ALERT,
				threshold: { ...ALERT.threshold, metric: "p99" },
			}).metric,
		).toBe("p99");
	});
});
