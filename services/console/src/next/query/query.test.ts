import { describe, expect, test } from "vitest";
import {
	type ExploreQuery,
	blankQuery,
	decodeQuery,
	encodeQuery,
} from "./query";

const uuid = (n: number) =>
	`00000000-0000-4000-8000-${n.toString(16).padStart(12, "0")}`;

const DAY = 24 * 60 * 60;

const FULL: ExploreQuery = {
	branches: [{ uuid: uuid(1), head: uuid(11) }, { uuid: uuid(2) }],
	testbeds: [{ uuid: uuid(3) }, { uuid: uuid(4), spec: uuid(14) }],
	benchmarks: [uuid(5), uuid(6)],
	sets: [
		{ input_bytes: 65536, simd: "avx2" },
		{},
		{ fast: true, label: "a,b&c=d %2C+" },
	],
	measures: [uuid(7), uuid(8)],
	metrics: ["value", "p99, tail & more"],
	xAxis: "version",
	yScale: "log",
	window: { seconds: 7 * DAY, end: 1_757_800_000_000 },
	layout: "stacked",
	hide: ["k1", "k2"],
	only: ["k3", "k4"],
	focus: "k3",
	report: uuid(9),
	plot: uuid(10),
};

const roundTrip = (query: ExploreQuery) => decodeQuery(encodeQuery(query));

describe("encodeQuery and decodeQuery", () => {
	// Kills dropping any field on either side of the codec.
	test("round trip every field", () => {
		expect(roundTrip(FULL)).toEqual(FULL);
	});

	// Kills a field that only survives with a leading question mark, or only without one.
	test("read a query string with or without its question mark", () => {
		const search = encodeQuery(FULL);
		expect(search.startsWith("?")).toBe(true);
		expect(decodeQuery(search.slice(1))).toEqual(FULL);
	});

	// Kills two values of a view control encoding to one.
	test("round trip every x axis, y scale, and layout", () => {
		for (const xAxis of ["date", "version"] as const) {
			for (const yScale of ["auto", "linear", "log"] as const) {
				for (const layout of ["dual", "stacked"] as const) {
					const query = { ...FULL, xAxis, yScale, layout };
					expect(roundTrip(query)).toEqual(query);
				}
			}
		}
	});

	// Kills a rolling window that only survives at the presets, or loses its end.
	test("round trip rolling windows, with and without an end", () => {
		for (const seconds of [
			7 * DAY,
			28 * DAY,
			92 * DAY,
			10 * DAY,
			6 * 7 * DAY,
			3_601,
		]) {
			for (const window of [{ seconds }, { seconds, end: 1_757_800_000_000 }]) {
				const query = { ...FULL, window };
				expect(roundTrip(query)).toEqual(query);
			}
		}
	});

	// Kills a custom window read as a rolling one, or one that needs an end.
	test("round trip a custom window, with and without an end", () => {
		for (const window of [
			{ start: 1_757_000_000_000, end: 1_757_800_000_000 },
			{ start: 1_757_000_000_000 },
		]) {
			const query = { ...FULL, window };
			expect(roundTrip(query)).toEqual(query);
		}
	});

	// Kills writing the defaults, which turns a blank Explore link into a long one.
	test("write nothing for the blank query", () => {
		expect(encodeQuery(blankQuery())).toBe("");
	});

	// Kills a default that differs from Explore opened with nothing chosen.
	test("read an empty query as nothing chosen over the last four weeks", () => {
		expect(decodeQuery("")).toEqual({
			branches: [],
			testbeds: [],
			benchmarks: [],
			sets: [],
			measures: [],
			metrics: [],
			xAxis: "date",
			yScale: "auto",
			window: { seconds: 28 * DAY },
			layout: "dual",
			hide: [],
		});
		expect(blankQuery()).toEqual(decodeQuery(""));
	});

	// Kills splitting a list before it is decoded, which breaks a link something re-encoded.
	test("read a link whose separators and braces were percent encoded again", () => {
		const reencoded = new URLSearchParams(encodeQuery(FULL)).toString();
		expect(reencoded).toContain("%2C");
		expect(decodeQuery(reencoded)).toEqual(FULL);
	});

	// Kills escaping what reads fine in a link, or writing a window in seconds; the address bar is the query.
	test("write a link a reader can read", () => {
		const search = encodeQuery({
			...blankQuery(),
			branches: [{ uuid: uuid(1) }, { uuid: uuid(2) }],
			sets: [{ simd: "avx2" }],
			window: { seconds: 7 * DAY },
		});
		expect(search).toContain(`branches=${uuid(1)},${uuid(2)}`);
		expect(search).toContain("parameters={%22simd%22:%22avx2%22}");
		expect(search).toContain("window=1w");
		expect(
			encodeQuery({ ...blankQuery(), window: { seconds: 10 * DAY } }),
		).toBe("?window=10d");
	});

	// Kills reading a parameter value as text, which makes 1 and "1" one set.
	test("keep each parameter value's JSON type", () => {
		const query = {
			...FULL,
			sets: [{ n: 1 }, { n: "1" }, { b: true }, { b: "true" }, { x: 1.5 }],
		};
		expect(roundTrip(query)).toEqual(query);
	});

	// Kills writing a set's keys in the order given, which makes one set two links.
	test("write a set's keys in one order", () => {
		const forward = { ...FULL, sets: [{ a: 1, b: "x", C: true }] };
		const backward = { ...FULL, sets: [{ C: true, b: "x", a: 1 }] };
		expect(encodeQuery(forward)).toBe(encodeQuery(backward));
	});

	// Kills merging or dropping a repeated set; the box keeps the sets as chosen.
	test("keep a set chosen twice", () => {
		const query = { ...FULL, sets: [{ a: 1 }, { a: 1 }] };
		expect(roundTrip(query)).toEqual(query);
	});
});

describe("decodeQuery", () => {
	// Kills passing a malformed value on to the API, which refuses the query or drops the set.
	test("drop what it cannot read", () => {
		const search = [
			`branches=nope,${uuid(1)}`,
			`testbeds=${uuid(3)},`,
			`benchmarks=${uuid(5)},x`,
			`measures=,${uuid(7)}`,
			"parameters=not json",
			"parameters=[1]",
			'parameters={"a":[1]}',
			'parameters={"a":null}',
			'parameters={"a":1e999}',
			'parameters={"a":{"b":1}}',
			'parameters={"":1}',
			'parameters={"a":" padded"}',
			`parameters={${Array.from({ length: 9 }, (_, n) => `"k${n}":${n}`).join(",")}}`,
			'parameters={"ok":1}',
			"metrics=",
			"metrics=%20padded",
			`metrics=${"m".repeat(65)}`,
			`metrics=${"é".repeat(33)}`,
			"metrics=value",
			`metrics=${"é".repeat(32)}`,
			"x_axis=sideways",
			"y_axis=sqrt",
			"layout=grid",
			"window=fortnight",
			"end_time=soon",
			"hide=K1,ok,toolongforalinekey0",
			"only=",
			"focus=-",
			"report=nope",
			"plot=nope",
		].join("&");
		expect(decodeQuery(search)).toEqual({
			...blankQuery(),
			branches: [{ uuid: uuid(1) }],
			testbeds: [{ uuid: uuid(3) }],
			benchmarks: [uuid(5)],
			measures: [uuid(7)],
			sets: [{ ok: 1 }],
			metrics: ["value", "é".repeat(32)],
			hide: ["ok"],
		});
	});

	// Kills a box holding more than the API reads, which draws less than the box shows.
	test("read at most eight values a box and eight sets", () => {
		const many = Array.from({ length: 9 }, (_, n) => uuid(n + 1));
		const search = [
			`branches=${many.join(",")}`,
			`testbeds=${many.join(",")}`,
			`benchmarks=${many.join(",")}`,
			`measures=${many.join(",")}`,
			...many.map((_, n) => `metrics=m${n}`),
			...many.map((_, n) => `parameters={"n":${n}}`),
		].join("&");
		const query = decodeQuery(search);
		expect(query.branches.map(({ uuid }) => uuid)).toEqual(many.slice(0, 8));
		expect(query.testbeds.map(({ uuid }) => uuid)).toEqual(many.slice(0, 8));
		expect(query.benchmarks).toEqual(many.slice(0, 8));
		expect(query.measures).toEqual(many.slice(0, 8));
		expect(query.metrics).toEqual(many.slice(0, 8).map((_, n) => `m${n}`));
		expect(query.sets).toEqual(many.slice(0, 8).map((_, n) => ({ n })));
	});

	// Kills a value named twice taking two of the eight places in its box.
	test("read a value named twice in a box once", () => {
		const search = [
			`branches=${uuid(1)},${uuid(1)},${uuid(2)}`,
			`benchmarks=${uuid(5)},${uuid(5)}`,
			"metrics=value&metrics=value",
			"hide=k1,k1",
		].join("&");
		const query = decodeQuery(search);
		expect(query.branches).toEqual([{ uuid: uuid(1) }, { uuid: uuid(2) }]);
		expect(query.benchmarks).toEqual([uuid(5)]);
		expect(query.metrics).toEqual(["value"]);
		expect(query.hide).toEqual(["k1"]);
	});

	// Kills pairing heads and specs by anything but position.
	test("pair each head with its branch and each spec with its testbed", () => {
		const search = [
			`branches=${uuid(1)},${uuid(2)},${uuid(3)}`,
			`heads=,${uuid(12)}`,
			`testbeds=${uuid(4)}`,
			`specs=${uuid(14)},${uuid(15)}`,
		].join("&");
		const query = decodeQuery(search);
		expect(query.branches).toEqual([
			{ uuid: uuid(1) },
			{ uuid: uuid(2), head: uuid(12) },
			{ uuid: uuid(3) },
		]);
		expect(query.testbeds).toEqual([{ uuid: uuid(4), spec: uuid(14) }]);
	});

	// Kills pairing after dropping, which hands a dropped branch's head to the next one.
	test("keep a head with its branch when an earlier branch is dropped", () => {
		const search = [
			`branches=nope,${uuid(2)},${uuid(2)},${uuid(3)}`,
			`heads=${uuid(11)},${uuid(12)},${uuid(13)},${uuid(14)}`,
		].join("&");
		expect(decodeQuery(search).branches).toEqual([
			{ uuid: uuid(2), head: uuid(12) },
			{ uuid: uuid(3), head: uuid(14) },
		]);
	});

	// Kills a window the API refuses: none at all, or past its 32 bit count of seconds.
	test("read a rolling window only within the API's range", () => {
		for (const window of ["0d", "0s", "4294967296s", "7102w"]) {
			expect(decodeQuery(`window=${window}`).window).toEqual({
				seconds: 28 * DAY,
			});
		}
		expect(decodeQuery("window=4294967295s").window).toEqual({
			seconds: 4_294_967_295,
		});
	});

	// Kills ignoring start_time, or letting a preset override it.
	test("read start_time as a custom window and end_time alone as the end of a rolling one", () => {
		expect(
			decodeQuery("start_time=1000&end_time=2000&window=1w").window,
		).toEqual({
			start: 1000,
			end: 2000,
		});
		expect(decodeQuery("window=1w&end_time=2000").window).toEqual({
			seconds: 7 * DAY,
			end: 2000,
		});
		expect(decodeQuery("end_time=2000").window).toEqual({
			seconds: 28 * DAY,
			end: 2000,
		});
	});
});
