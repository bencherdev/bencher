import { describe, expect, test } from "vitest";
import {
	type LineIdentity,
	lineKey,
	lineVisible,
	settleVisibility,
} from "./line";
import { blankQuery } from "./query";

const LINE: LineIdentity = {
	branch: "00000000-0000-4000-8000-000000000001",
	testbed: "00000000-0000-4000-8000-000000000002",
	benchmark: "00000000-0000-4000-8000-000000000003",
	parameters: { input_bytes: 65536, simd: "avx2", threads: 1 },
	measure: "00000000-0000-4000-8000-000000000004",
	metric: "value",
};

describe("lineKey", () => {
	// Kills a change to the key's recipe, which would orphan every hidden line saved in a link or a pin.
	test("stay the same across releases", () => {
		expect(lineKey(LINE)).toBe("1gav3bmyc9r");
		// Uppercase sorts before lowercase in UTF-16, which a locale aware sort reverses.
		expect(
			lineKey({ ...LINE, parameters: { Mode: "fast", alpha: 1, Beta: true } }),
		).toBe("23cinte3o5n");
	});

	// Kills a long key, or one with characters a comma separated list in a link cannot carry.
	test("be short and need no escaping", () => {
		expect(lineKey(LINE)).toMatch(/^[0-9a-z]{1,11}$/);
	});

	// Kills hashing the parameters in the order they arrive.
	test("not depend on the order of the parameters", () => {
		expect(
			lineKey({
				...LINE,
				parameters: { threads: 1, simd: "avx2", input_bytes: 65536 },
			}),
		).toBe(lineKey(LINE));
	});

	// Kills leaving a dimension out of the key.
	test("differ in every dimension", () => {
		const other = "00000000-0000-4000-8000-0000000000ff";
		const variations: LineIdentity[] = [
			{ ...LINE, branch: other },
			{ ...LINE, testbed: other },
			{ ...LINE, benchmark: other },
			{ ...LINE, parameters: { ...LINE.parameters, threads: 4 } },
			{ ...LINE, parameters: { input_bytes: 65536, simd: "avx2" } },
			{ ...LINE, measure: other },
			{ ...LINE, metric: "p99" },
		];
		const keys = new Set([LINE, ...variations].map(lineKey));
		expect(keys.size).toBe(variations.length + 1);
	});

	// Kills turning values into text, which makes threads=1 and threads="1" one line.
	test("tell a number from the same text", () => {
		expect(
			lineKey({ ...LINE, parameters: { ...LINE.parameters, threads: "1" } }),
		).not.toBe(lineKey(LINE));
	});

	// Kills joining the dimensions without a boundary between them.
	test("keep the boundary between dimensions", () => {
		expect(lineKey({ ...LINE, parameters: { a: "b" }, metric: "c" })).not.toBe(
			lineKey({ ...LINE, parameters: { a: "bc" }, metric: "" }),
		);
	});

	// Kills a hash too narrow for the lines a project holds.
	test("not collide across many lines", () => {
		const keys = new Set<string>();
		for (let threads = 0; threads < 2_000; threads++) {
			for (const metric of ["value", "p50", "p99", "max", "min"]) {
				keys.add(
					lineKey({
						...LINE,
						parameters: { ...LINE.parameters, threads },
						metric,
					}),
				);
			}
		}
		expect(keys.size).toBe(10_000);
	});
});

describe("lineVisible", () => {
	const query = blankQuery();

	// Kills ignoring the hidden lines.
	test("hide the lines the reader hid", () => {
		const hidden = { ...query, hide: ["a"] };
		expect(lineVisible(hidden, "a")).toBe(false);
		expect(lineVisible(hidden, "b")).toBe(true);
	});

	// Kills ignoring `only`, which draws every extra line a selection did not pick.
	test("draw only the listed lines when the query lists them", () => {
		const only = { ...query, only: ["a", "b"], hide: ["b"] };
		expect(lineVisible(only, "a")).toBe(true);
		expect(lineVisible(only, "b")).toBe(false);
		expect(lineVisible(only, "c")).toBe(false);
	});
});

describe("settleVisibility", () => {
	// Kills keeping `only`, which hides every line a later edit adds, or losing a hidden line not drawn.
	test("turn the listed lines into hidden extras", () => {
		const query = { ...blankQuery(), only: ["a", "c"], hide: ["c", "z"] };
		const settled = settleVisibility(query, ["a", "b", "c", "d"]);
		expect(settled.only).toBeUndefined();
		expect(settled.hide).toEqual(["b", "c", "d", "z"]);
		for (const key of ["a", "b", "c", "d"]) {
			expect(lineVisible(settled, key)).toBe(lineVisible(query, key));
		}
	});

	// Kills pruning hidden lines the response did not carry, such as those past the line cap.
	test("leave a query without listed lines as it is", () => {
		const query = { ...blankQuery(), hide: ["z"] };
		expect(settleVisibility(query, ["a"])).toEqual(query);
	});
});
