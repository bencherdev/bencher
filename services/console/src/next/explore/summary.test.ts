import { expect, test } from "vitest";
import type { JsonConsolePerf } from "../../types/bencher";
import { type ExploreQuery, blankQuery } from "../query/query";
import { chipSummary } from "./summary";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const QUERY: ExploreQuery = {
	...blankQuery(),
	branches: [{ uuid: uuid(1) }, { uuid: uuid(2) }],
	testbeds: [{ uuid: uuid(3) }],
	benchmarks: [uuid(5)],
	measures: [uuid(7)],
};

const PERF = {
	branches: [
		{ uuid: uuid(1), name: "main", slug: "main", head: uuid(11) },
		{ uuid: uuid(2), name: "feature", slug: "feature", head: uuid(12) },
	],
	testbeds: [],
	benchmarks: [],
	measures: [],
} as unknown as JsonConsolePerf;

// Kills a chip that names a parameter set by a stand-in rather than its tags.
test("the parameters chip names a lone set by its tags, and counts several", () => {
	const sets = (query: Partial<ExploreQuery>) =>
		chipSummary({ ...QUERY, ...query }, "sets", PERF, false);
	expect(sets({ sets: [{ input_bytes: 65536, threads: 1 }] })).toBe(
		"input_bytes=65536 threads=1",
	);
	expect(sets({ sets: [{ simd: "avx2" }, { threads: 1 }] })).toBe("2 sets");
	expect(sets({ sets: [] })).toBe("every variant");
});

// Kills a chip that counts values the plot has named, and one that names a
// value the plot has not, by its UUID.
test("a chip names its values once the plot has named them all, and counts them until then", () => {
	expect(chipSummary(QUERY, "branches", PERF, false)).toBe("main, feature");
	expect(chipSummary(QUERY, "testbeds", PERF, false)).toBe("1 chosen");
	expect(chipSummary(QUERY, "benchmarks", undefined, false)).toBe("1 chosen");
});
