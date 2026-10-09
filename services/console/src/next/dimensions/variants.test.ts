import { expect, test } from "vitest";
import type { JsonVariant } from "../../types/bencher";
import { parametersInUse, shownVariants, tagsOf } from "./variants";

const variant = (parameters: JsonVariant["parameters"], uuid = "v") =>
	({ uuid, parameters }) as JsonVariant;

// Kills keys or values out of order, numbers compared as text, and counts
// that tally variants without the key.
test("parametersInUse lists each key's values with how many variants carry each", () => {
	expect(
		parametersInUse([
			variant({ input_bytes: 1048576, simd: "avx2" }),
			variant({ input_bytes: 4096, simd: "avx2", threads: 4 }),
			variant({ input_bytes: 4096, simd: "sse4.2" }),
		]),
	).toEqual([
		{
			key: "input_bytes",
			values: [
				{ value: "4096", variants: 2 },
				{ value: "1048576", variants: 1 },
			],
		},
		{
			key: "simd",
			values: [
				{ value: "avx2", variants: 2 },
				{ value: "sse4.2", variants: 1 },
			],
		},
		{ key: "threads", values: [{ value: "4", variants: 1 }] },
	]);
});

// Kills tags in insertion order, which differs between runs of one variant.
test("a variant's tags read in key order", () => {
	expect(tagsOf(variant({ threads: 4, input_bytes: 1024 }))).toEqual([
		"input_bytes=1024",
		"threads=4",
	]);
});

// Kills the empty variant every benchmark is born with listed beside the
// variants a run reported, and a benchmark without parameters left empty.
test("the empty variant shows only when it is the benchmark's only one", () => {
	const empty = variant({}, "empty");
	const tagged = variant({ threads: 1 }, "tagged");
	expect(shownVariants([empty, tagged])).toEqual([tagged]);
	expect(shownVariants([empty])).toEqual([empty]);
});
