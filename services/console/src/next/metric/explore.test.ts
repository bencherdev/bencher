import { expect, test } from "vitest";
import type { JsonOneMetric } from "../../types/bencher";
import { lineKey } from "../query/line";
import { decodeQuery } from "../query/query";
import { exploreSearchOf } from "./explore";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const METRIC = {
	uuid: uuid(1),
	report: uuid(2),
	iteration: 0,
	start_time: "2026-09-13T23:00:00Z",
	end_time: "2026-09-13T23:02:00Z",
	branch: { uuid: uuid(3), head: { uuid: uuid(4) } },
	testbed: { uuid: uuid(5), spec: { uuid: uuid(6) } },
	benchmark: { uuid: uuid(7) },
	variant: { uuid: uuid(8), parameters: { simd: "avx2", threads: 1 } },
	measure: { uuid: uuid(9) },
	name: "p99",
	value: 1,
} as unknown as JsonOneMetric;

// Kills a line on the branch's current head rather than its report's, the
// wrong variant or metric, a window that does not end at the report, and the
// report left unmarked.
test("draws exactly the metric's line on its report's head, with its report marked", () => {
	const query = decodeQuery(exploreSearchOf(METRIC));
	const key = lineKey({
		branch: uuid(3),
		testbed: uuid(5),
		benchmark: uuid(7),
		parameters: { simd: "avx2", threads: 1 },
		measure: uuid(9),
		metric: "p99",
	});
	expect(query).toMatchObject({
		branches: [{ uuid: uuid(3), head: uuid(4) }],
		testbeds: [{ uuid: uuid(5) }],
		benchmarks: [uuid(7)],
		sets: [{ simd: "avx2", threads: 1 }],
		measures: [uuid(9)],
		metrics: ["p99"],
		only: [key],
		focus: key,
		window: {
			seconds: 28 * 86_400,
			end: Date.parse("2026-09-13T23:02:00Z"),
		},
		report: uuid(2),
	});
});
