import { describe, expect, test } from "vitest";
import { lineKey, lineVisible } from "./line";
import { decodeQuery } from "./query";
import { type SelectedLine, exploreSearch } from "./selection";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const BRANCH = { uuid: uuid(1), head: uuid(11) };
const TESTBED = { uuid: uuid(2), spec: uuid(12) };
const BLAKE3 = uuid(3);
const XXH3 = uuid(4);
const LATENCY = uuid(5);
const THROUGHPUT = uuid(6);

const line = (overrides: Partial<SelectedLine>): SelectedLine => ({
	branch: BRANCH,
	testbed: TESTBED,
	benchmark: BLAKE3,
	parameters: { input_bytes: 65536, simd: "avx2", threads: 1 },
	measure: LATENCY,
	metric: "value",
	...overrides,
});

const keyOf = ({ branch, testbed, ...rest }: SelectedLine) =>
	lineKey({ ...rest, branch: branch.uuid, testbed: testbed.uuid });

// Open 3 from a report: the alerting blake3 line, blake3 at four threads, and xxh3.
const ALERTING = line({ alerting: true });
const FOUR_THREADS = line({
	parameters: { input_bytes: 65536, simd: "avx2", threads: 4 },
});
const XXH3_LINE = line({
	benchmark: XXH3,
	parameters: { input_bytes: 65536, simd: "avx2" },
});
const OPEN_3 = [FOUR_THREADS, ALERTING, XXH3_LINE];

const open = (
	lines: SelectedLine[],
	context?: Parameters<typeof exploreSearch>[1],
) => decodeQuery(exploreSearch(lines, context));

describe("exploreSearch", () => {
	// Kills naming a value twice or out of the order the lines were selected in.
	test("name each value of the selection once, in order", () => {
		const query = open([
			...OPEN_3,
			line({ measure: THROUGHPUT, metric: "p99" }),
		]);
		expect(query.branches).toEqual([BRANCH]);
		expect(query.testbeds).toEqual([TESTBED]);
		expect(query.benchmarks).toEqual([BLAKE3, XXH3]);
		expect(query.measures).toEqual([LATENCY, THROUGHPUT]);
		expect(query.metrics).toEqual(["value", "p99"]);
	});

	// Kills a set wider than its variant, or a variant's set added once per line.
	test("add one set per selected variant, with all of its parameters", () => {
		const query = open([...OPEN_3, line({ measure: THROUGHPUT })]);
		expect(query.sets).toEqual([
			FOUR_THREADS.parameters,
			ALERTING.parameters,
			XXH3_LINE.parameters,
		]);
	});

	// Kills drawing the extras of the cross product, which the builder cannot enumerate.
	test("draw exactly the selected lines", () => {
		const query = open(OPEN_3);
		for (const selected of OPEN_3) {
			expect(lineVisible(query, keyOf(selected))).toBe(true);
		}
		const extras = [
			line({ parameters: { input_bytes: 65536, simd: "avx2", threads: 8 } }),
			line({ benchmark: XXH3, parameters: ALERTING.parameters }),
			line({ benchmark: BLAKE3, parameters: XXH3_LINE.parameters }),
		];
		for (const extra of extras) {
			expect(lineVisible(query, keyOf(extra))).toBe(false);
		}
	});

	// Kills focusing a line other than the one that alerted.
	test("focus the alerting line", () => {
		expect(open(OPEN_3).focus).toBe(keyOf(ALERTING));
	});

	// Kills leaving a single selected line unfocused, or focusing one of several.
	test("focus a lone line, and no line among several quiet ones", () => {
		expect(open([FOUR_THREADS]).focus).toBe(keyOf(FOUR_THREADS));
		expect(open([FOUR_THREADS, XXH3_LINE]).focus).toBeUndefined();
	});

	// Kills dropping the window the lines were viewed at, or the report they came from.
	test("carry the window and the report", () => {
		const window = { seconds: 7 * 24 * 60 * 60, end: 1_757_800_000_000 };
		const query = open(OPEN_3, { window, report: uuid(9) });
		expect(query.window).toEqual(window);
		expect(query.report).toBe(uuid(9));
	});

	// Kills passing more sets than the API reads, which drops selected lines from the plot.
	test("widen to the parameters every variant shares past eight variants", () => {
		const nine = Array.from({ length: 9 }, (_, threads) =>
			line({ parameters: { input_bytes: 65536, simd: "avx2", threads } }),
		);
		const query = open(nine);
		expect(query.sets).toEqual([{ input_bytes: 65536, simd: "avx2" }]);
		for (const selected of nine) {
			expect(lineVisible(query, keyOf(selected))).toBe(true);
		}
		const scattered = Array.from({ length: 9 }, (_, n) =>
			line({ parameters: { [`k${n}`]: n } }),
		);
		expect(open(scattered).sets).toEqual([]);
		const eight = nine.slice(0, 8);
		expect(open(eight).sets).toEqual(eight.map(({ parameters }) => parameters));
	});
});
