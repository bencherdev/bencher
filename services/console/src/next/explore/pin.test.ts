import { describe, expect, test } from "vitest";
import { type PinTitleLine, defaultPinTitle, pinWindow } from "./pin";

const BLAKE3_VARIANTS = [
	{ input_bytes: 65536, threads: 1, simd: "avx2" },
	{ input_bytes: 65536, threads: 4, simd: "avx2" },
	{ input_bytes: 1048576, threads: 1, simd: "avx2" },
	{ input_bytes: 4096, threads: 1, simd: "scalar" },
];

const LINE: PinTitleLine = {
	benchmark: "blake3",
	parameters: { input_bytes: 65536, threads: 1, simd: "avx2" },
	variants: BLAKE3_VARIANTS,
	measure: "Latency",
	branch: "main",
};

const utf8Length = (text: string) => new TextEncoder().encode(text).length;

describe("defaultPinTitle", () => {
	// Kills naming only the parameters that vary across the drawn lines, which a lone line has none of.
	test("name a line with every parameter that varies across its benchmark's variants", () => {
		expect(defaultPinTitle([LINE])).toBe(
			"blake3 input_bytes=65536 threads=1 simd=avx2, Latency on main",
		);
	});

	// Kills naming every parameter the variant carries.
	test("leave out a parameter every variant shares", () => {
		const variants = BLAKE3_VARIANTS.map((variant) => ({
			...variant,
			simd: "avx2",
		}));
		expect(defaultPinTitle([{ ...LINE, variants }])).toBe(
			"blake3 input_bytes=65536 threads=1, Latency on main",
		);
	});

	// Kills skipping a key that only some variants carry.
	test("name a parameter some variants lack", () => {
		const line = {
			...LINE,
			parameters: { threads: 1 },
			variants: [{ threads: 1 }, {}],
		};
		expect(defaultPinTitle([line])).toBe("blake3 threads=1, Latency on main");
	});

	// Kills comparing values as text, which makes 1 and "1" one value.
	test("tell a number from the same text", () => {
		const line = {
			...LINE,
			parameters: { threads: 1 },
			variants: [{ threads: 1 }, { threads: "1" }],
		};
		expect(defaultPinTitle([line])).toBe("blake3 threads=1, Latency on main");
	});

	// Kills naming parameters of a benchmark that has nothing to tell apart.
	test("name no parameter of a benchmark with one variant", () => {
		expect(defaultPinTitle([{ ...LINE, variants: [] }])).toBe(
			"blake3, Latency on main",
		);
	});

	// Kills leaving the line's own variant out of the comparison.
	test("compare the line's variant with the others", () => {
		const line = {
			...LINE,
			parameters: { threads: 1 },
			variants: [{ threads: 4 }],
		};
		expect(defaultPinTitle([line])).toBe("blake3 threads=1, Latency on main");
	});

	// Kills dropping the metric when the caller names it.
	test("name the metric before its measure when given", () => {
		expect(defaultPinTitle([{ ...LINE, variants: [], metric: "p99" }])).toBe(
			"blake3, p99 Latency on main",
		);
	});

	// Kills taking the first line's parameters for the whole plot.
	test("name several lines by what they share", () => {
		const lines: PinTitleLine[] = [
			LINE,
			{ ...LINE, parameters: { input_bytes: 65536, threads: 4, simd: "avx2" } },
			{
				...LINE,
				benchmark: "xxh3",
				parameters: { input_bytes: 65536, simd: "avx2" },
				variants: [
					{ input_bytes: 65536, simd: "avx2" },
					{ input_bytes: 4096, simd: "avx2" },
				],
			},
		];
		expect(defaultPinTitle(lines)).toBe(
			"blake3 and xxh3 input_bytes=65536 simd=avx2, Latency on main",
		);
	});

	// Kills listing every name, which runs past the title's length.
	test("count what does not fit in a short list", () => {
		const lines = ["blake3", "crc32c", "sha256"].flatMap((benchmark) =>
			["Latency", "Throughput", "Instructions"].map((measure) => ({
				...LINE,
				benchmark,
				measure,
				variants: [],
			})),
		);
		expect(defaultPinTitle(lines)).toBe(
			"blake3 and 2 more, Latency and 2 more on main",
		);
		expect(
			defaultPinTitle([
				{ ...LINE, variants: [], measure: "Latency" },
				{ ...LINE, variants: [], measure: "Throughput", branch: "412/merge" },
			]),
		).toBe("blake3, Latency and Throughput on main and 412/merge");
	});

	// Kills a title the API refuses: longer than 64 bytes, or ending in a space.
	test("fit the API's 64 byte limit, cutting between words", () => {
		const title = defaultPinTitle([
			{ ...LINE, benchmark: "a_benchmark_with_a_longer_name" },
		]);
		expect(utf8Length(title)).toBeLessThanOrEqual(64);
		expect(title).toBe(
			"a_benchmark_with_a_longer_name input_bytes=65536 threads=1…",
		);
	});

	// Kills backing off a word that ends right at the limit, or keeping the comma before the cut.
	test("keep a word that ends at the limit, without its comma", () => {
		const exact = defaultPinTitle([
			{ ...LINE, benchmark: "a_benchmark_with_a_longer_name_xx" },
		]);
		expect(exact).toBe(
			"a_benchmark_with_a_longer_name_xx input_bytes=65536 threads=1…",
		);
		expect(utf8Length(exact)).toBe(64);
		const comma = defaultPinTitle([
			{ ...LINE, benchmark: "x".repeat(60), variants: [] },
		]);
		expect(comma).toBe(`${"x".repeat(60)}…`);
	});

	// Kills cutting by characters, which leaves a multibyte title over the limit or splits a character.
	test("cut a multibyte title on a character boundary", () => {
		const title = defaultPinTitle([
			{ ...LINE, benchmark: "🦀".repeat(40), variants: [] },
		]);
		expect(utf8Length(title)).toBeLessThanOrEqual(64);
		expect(title).toBe(`${"🦀".repeat(15)}…`);
	});
});

describe("pinWindow", () => {
	const DAY = 24 * 60 * 60;

	// Kills saving the anchor of a rolling window, or saving milliseconds.
	test("keep a rolling window's duration", () => {
		expect(pinWindow({ seconds: 28 * DAY, end: 1_757_800_000_000 }, 0)).toBe(
			28 * DAY,
		);
	});

	// Kills saving a custom range's start or end instead of its length.
	test("save a custom range as its length", () => {
		expect(
			pinWindow(
				{ start: 1_757_000_000_000, end: 1_757_000_000_000 + 10 * DAY * 1000 },
				0,
			),
		).toBe(10 * DAY);
	});

	// Kills ignoring the clock for a range that runs to now.
	test("measure an open range to now", () => {
		expect(pinWindow({ start: 1_000_000 }, 1_000_000 + 3 * DAY * 1000)).toBe(
			3 * DAY,
		);
	});

	// Kills a zero window, which the API refuses.
	test("save at least one second", () => {
		expect(pinWindow({ start: 5_000, end: 5_000 }, 0)).toBe(1);
	});
});
