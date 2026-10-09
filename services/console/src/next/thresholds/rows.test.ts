import { describe, expect, test } from "vitest";
import { rowsOf } from "./rows";
import { thresholdsFixture } from "./testing";

describe("rowsOf", () => {
	const batch = thresholdsFixture([
		{},
		{ branch: 1, measure: 1, metric: "p99", parameters: [{ n: 1 }] },
	]);
	const [latency, throughput] = rowsOf(batch);

	// Kills every row read against the tables' first entries.
	test("resolves each row's dimensions from the batch's tables", () => {
		expect(latency?.branch.name).toBe("main");
		expect(throughput?.branch).toBe(batch.branches[1]);
		expect(throughput?.measure.name).toBe("Throughput");
		expect(throughput?.parameters).toEqual([{ n: 1 }]);
	});

	// Kills a threshold on the conventional metric drawn with no metric at all.
	test("names the conventional metric when the API leaves it out", () => {
		expect(latency?.metric).toBe("value");
		expect(throughput?.metric).toBe("p99");
	});

	// Kills a row whose dimension is missing from the tables drawn half empty.
	test("drops a row whose dimensions the batch does not hold", () => {
		expect(rowsOf(thresholdsFixture([{ testbed: 5 }]))).toEqual([]);
	});
});
