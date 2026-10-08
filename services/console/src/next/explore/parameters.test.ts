import { describe, expect, test } from "vitest";
import { matchedVariants, matchesSet, parameterRows } from "./parameters";

// blake3's variants on the canvas: input bytes, threads, and SIMD.
const VARIANTS = [
	{ input_bytes: 65536, threads: 1, simd: "scalar" },
	{ input_bytes: 65536, threads: 1, simd: "avx2" },
	{ input_bytes: 65536, threads: 4, simd: "avx2" },
	{ input_bytes: 1048576, threads: 1, simd: "avx2" },
	{ input_bytes: 4096, threads: 1, simd: "sse41" },
];

describe("matchesSet", () => {
	// Kills requiring a variant to equal the set rather than carry it.
	test("match a variant that carries every tag of the set", () => {
		expect(matchesSet(VARIANTS[2] ?? {}, { threads: 4 })).toBe(true);
		expect(matchesSet(VARIANTS[2] ?? {}, { threads: 4, simd: "avx2" })).toBe(
			true,
		);
	});

	// Kills matching on any one tag instead of all of them.
	test("not match a variant missing a tag or holding another value", () => {
		expect(matchesSet(VARIANTS[0] ?? {}, { threads: 1, simd: "avx2" })).toBe(
			false,
		);
		expect(matchesSet({ threads: 1 }, { threads: 1, simd: "avx2" })).toBe(
			false,
		);
	});

	// Kills comparing values as text, which the API does not.
	test("tell a number from the same text", () => {
		expect(matchesSet({ threads: 1 }, { threads: "1" })).toBe(false);
		expect(matchesSet({ fast: true }, { fast: "true" })).toBe(false);
	});

	// Kills an empty set matching nothing; a row with no tags is every variant.
	test("match every variant with an empty set", () => {
		expect(VARIANTS.every((variant) => matchesSet(variant, {}))).toBe(true);
	});
});

describe("parameterRows", () => {
	// Kills counting the union, or a row's count shrinking for rows before it.
	test("count the variants each row matches on its own", () => {
		const rows = parameterRows([{ threads: 1 }, { simd: "avx2" }], VARIANTS);
		expect(rows.map(({ variants }) => variants)).toEqual([4, 3]);
	});

	// Kills the flipped subset check, which flags the wider set instead of the narrower.
	test("flag a set that holds every tag of a wider set", () => {
		const rows = parameterRows(
			[{ input_bytes: 65536, threads: 1 }, { input_bytes: 65536 }],
			VARIANTS,
		);
		expect(rows[0]?.coveredBy).toBe(1);
		expect(rows[1]?.coveredBy).toBeUndefined();
		expect(rows[0]?.variants).toBe(2);
	});

	// Kills flagging both copies of a set chosen twice, which leaves neither adding a variant.
	test("flag the later of two equal sets", () => {
		const rows = parameterRows([{ simd: "avx2" }, { simd: "avx2" }], VARIANTS);
		expect(rows.map(({ coveredBy }) => coveredBy)).toEqual([undefined, 0]);
	});

	// Kills a row with no tags left out of the covering, though it is every variant.
	test("let a row with no tags cover every other row", () => {
		const rows = parameterRows(
			[{ threads: 4 }, {}, { simd: "sse41" }],
			VARIANTS,
		);
		expect(rows.map(({ coveredBy }) => coveredBy)).toEqual([1, undefined, 1]);
	});

	// Kills flagging a set that only overlaps another; each still adds variants of its own.
	test("not flag overlapping sets", () => {
		const rows = parameterRows([{ threads: 1 }, { simd: "avx2" }], VARIANTS);
		expect(rows.map(({ coveredBy }) => coveredBy)).toEqual([
			undefined,
			undefined,
		]);
	});
});

describe("matchedVariants", () => {
	// Kills summing the rows, which counts a variant two rows match twice.
	test("count each variant any row matches once", () => {
		expect(matchedVariants([{ threads: 1 }, { simd: "avx2" }], VARIANTS)).toBe(
			5,
		);
		expect(
			matchedVariants([{ threads: 4 }, { input_bytes: 4096 }], VARIANTS),
		).toBe(2);
	});

	// Kills reading no rows as no variants; an empty box is every variant.
	test("count every variant when there are no rows", () => {
		expect(matchedVariants([], VARIANTS)).toBe(VARIANTS.length);
	});
});
