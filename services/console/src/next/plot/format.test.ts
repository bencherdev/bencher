import { describe, expect, test } from "vitest";
import {
	axisTier,
	changeOf,
	formatChange,
	formatDate,
	formatMeasure,
	formatTicks,
	formatWhen,
	formatWhenSeconds,
	NUMBER_MAX,
} from "./format";
import { familyOf } from "./units";
import { formatCount } from "./count";

const NS = "nanoseconds (ns)";
const SECONDS = "seconds (s)";
const REQUESTS = "requests per second (req/s)";

/** The value `text` stands for: its number, times its tier when its symbol names one. */
const readBack = (text: string, units: string) => {
	const [number = "", symbol] = text.split(" ");
	const family = familyOf(units);
	const tier = family?.tiers.find((each) => each.symbol === symbol);
	return Number(number) * (tier && family ? tier.factor / family.base : 1);
};

describe("formatMeasure", () => {
	// Kills an unbounded string, a lost sign or magnitude, a sub-second tier left out of seconds, or a tier taken past the fixed range.
	test.each([
		[1e300, NS, "1.00e300 ns"],
		[1.5e300, NS, "1.50e300 ns"],
		[1e-300, NS, "1.00e-300 ns"],
		[-1e-300, NS, "-1.0e-300 ns"],
		[0, NS, "0"],
		[-0, NS, "0"],
		[-5, NS, "-5.00 ns"],
		[-123456789.123, "", "-123.46e6"],
		[-123456789.123, "counts (cnt)", "-123.5e6 cnt"],
		[1234567890123, NS, "20.58 m"],
		[123456.78, NS, "123.46 µs"],
		[0.003, SECONDS, "3.00 ms"],
		[0.001, SECONDS, "1.00 ms"],
		[5e9, NS, "5.00 s"],
		[Number.NaN, NS, "n/a"],
		[Number.POSITIVE_INFINITY, NS, "n/a"],
		[5.1e6, "operations (ops)", "5.10e6 ops"],
		[1262.3, REQUESTS, "1.26e3 req/s"],
		[-1.5e300, REQUESTS, "-1.5e300"],
		[0.012, REQUESTS, "12e-3 req/s"],
	] as const)("%d in %s reads %s", (value, units, text) => {
		const printed = formatMeasure(value, units);
		expect(printed).toBe(text);
		expect(printed.length).toBeLessThanOrEqual(NUMBER_MAX);
	});

	// Kills a rounding carry to the next power of ten that leaves one significant digit.
	test("keeps two significant digits when rounding carries to the next power", () => {
		expect(formatMeasure(-9.9951e-304, REQUESTS)).toBe("-1.0e-303");
	});

	// Kills a mantissa rounded to one digit that a carry dresses as two, as 9.6 read as 10.
	test.each([
		[9.6e-12, REQUESTS, "9.6e-12"],
		[-9.6e-12, REQUESTS, "-9.6e-12"],
		[9.504e-156, "ops/sec", "9.5e-156"],
	])("%d in %s reads %s", (value, units, text) => {
		expect(formatMeasure(value, units)).toBe(text);
	});

	// Kills any output that reads back more than 5% off, or past twelve characters.
	test("every value reads back within 5% in at most twelve characters", () => {
		const MANTISSAS = [1, 1.234, 2.5, 4.999, 5, 9.5, 9.504, 9.6, 9.95, 9.996];
		const UNITS = [
			NS,
			SECONDS,
			"bytes (B)",
			REQUESTS,
			"ops/sec",
			"",
			`(${"u".repeat(62)})`,
		];
		for (const units of UNITS) {
			for (let exponent = -307; exponent <= 307; exponent++) {
				for (const mantissa of MANTISSAS) {
					for (const value of [
						mantissa * 10 ** exponent,
						-mantissa * 10 ** exponent,
					]) {
						const text = formatMeasure(value, units);
						const at = `${value} ${units}: ${text}`;
						expect(text.length, at).toBeLessThanOrEqual(NUMBER_MAX);
						expect(
							Math.abs(readBack(text, units) - value),
							at,
						).toBeLessThanOrEqual(Math.abs(value) * 0.05);
					}
				}
			}
		}
	});

	// Kills a value printed in the tier it rounds out of, as 1000.00 ns.
	test("takes the next tier when rounding reaches it", () => {
		expect(formatMeasure(999.999, NS)).toBe("1.00 µs");
		expect(formatMeasure(59.999, SECONDS)).toBe("1.00 m");
	});

	// Kills a unit with no parentheses printed with no symbol, or one past five characters.
	test("names a unit by its first parenthesized symbol, else by its start in five characters", () => {
		expect(formatMeasure(1, "a (b) (c)")).toBe("1.00 b");
		expect(formatMeasure(1, "ops/sec")).toBe("1.00 ops/…");
		expect(formatMeasure(1, "hits")).toBe("1.00 hits");
	});
});

describe("axisTier", () => {
	// Kills an axis in a tier its ticks overflow, past its unit's tiers with no power of ten, or a title symbol cut as a value's is.
	test.each([
		[2.5e6, NS, { factor: 1e6, symbol: "ms", exponent: 0 }],
		[0.003, SECONDS, { factor: 1e-3, symbol: "ms", exponent: 0 }],
		[1e300, NS, { factor: 1, symbol: "ns", exponent: 300 }],
		[0.012, REQUESTS, { factor: 1, symbol: "req/s", exponent: -3 }],
		[0, NS, { factor: 1, symbol: "ns", exponent: 0 }],
		[
			1,
			"requests/second",
			{ factor: 1, symbol: "requests/second", exponent: 0 },
		],
	])("an axis to %d in %s", (largest, units, tier) => {
		expect(axisTier(largest, units)).toEqual(tier);
	});
});

describe("formatChange", () => {
	// Kills an unbounded percent, a ratio that overflows to Infinity, or a blank at a zero baseline.
	test.each([
		[150, 100, "+50.0%"],
		[86.2, 100, "-13.8%"],
		[5000, 100, "×50.0"],
		[247_000, 1, "×2.47e5"],
		[1e300, 1e-300, "×1e600"],
		[5, 0, "from 0"],
	])("%d against %d reads %s", (value, baseline, text) => {
		const printed = formatChange(value, baseline);
		expect(printed).toBe(text);
		expect(printed.length).toBeLessThanOrEqual(NUMBER_MAX);
	});

	// Kills a negative ratio, which has no reading, or a percent across a sign that is not bounded.
	test.each([
		[-100, 100, "-200.0%"],
		[100, -100, "+200.0%"],
		[-1e6, 100, "-1.0e6%"],
		[1e6, -1, "+1.0e8%"],
		[1e300, -1e-300, "+1e602%"],
	])("%d against %d, across a sign, reads %s", (value, baseline, text) => {
		const printed = formatChange(value, baseline);
		expect(printed).toBe(text);
		expect(printed.length).toBeLessThanOrEqual(NUMBER_MAX);
	});

	// Kills dividing by a negative baseline's sign, which flips the move, so a row and the readout disagree.
	test("keeps the sign of the move over a negative baseline", () => {
		expect(formatChange(-110, -100)).toBe("-10.0%");
		expect(formatChange(-80, -100)).toBe("+20.0%");
	});

	// Kills a difference taken first, which overflows to Infinity between two values near the largest double.
	test("reads a change between the largest doubles", () => {
		expect(formatChange(1e308, -1e308)).toBe("+200.0%");
	});
});

describe("changeOf", () => {
	// Kills the sign, the arrow, or the side the threshold guards.
	test.each([
		[113.8, 100, "upper", "+13.8%", "↑", "worse"],
		[86.2, 100, "upper", "-13.8%", "↓", "better"],
		[86.2, 100, "lower", "-13.8%", "↓", "worse"],
		[113.8, 100, "lower", "+13.8%", "↑", "better"],
		[86.2, 100, "both", "-13.8%", "↓", "worse"],
		[113.8, 100, "both", "+13.8%", "↑", "worse"],
		[-120, -100, "lower", "-20.0%", "↓", "worse"],
	] as const)(
		"%d against %d guarded %s reads %s %s %s",
		(value, baseline, guard, text, arrow, tone) => {
			expect(changeOf(value, baseline, guard)).toEqual({ text, arrow, tone });
		},
	);

	// Kills naming a direction for noise, or a cutoff other than one percent.
	test("names no direction under one percent", () => {
		expect(changeOf(100.9, 100, "upper")).toEqual({
			text: "+0.9%",
			arrow: null,
			tone: null,
		});
		expect(changeOf(101, 100, "upper")).toEqual({
			text: "+1.0%",
			arrow: "↑",
			tone: "worse",
		});
		expect(changeOf(99.96, 100, "upper")).toEqual({
			text: "+0.0%",
			arrow: null,
			tone: null,
		});
	});

	// Kills a rise from nothing that names no direction.
	test("a rise from a zero baseline is worse above it", () => {
		expect(changeOf(5, 0, "upper")).toEqual({
			text: "from 0",
			arrow: "↑",
			tone: "worse",
		});
	});
});

describe("formatCount", () => {
	// Kills an ungrouped count, or a badge that grows past its two digits.
	test.each([
		[4096, undefined, "4,096"],
		[1_000_000, undefined, "1,000,000"],
		[1_000_000, 99, "99+"],
		[99, 99, "99"],
	])("%d with a badge of %s reads %s", (count, max, text) => {
		const printed = formatCount(count, max);
		expect(printed).toBe(text);
		expect(printed.length).toBeLessThanOrEqual(NUMBER_MAX);
	});
});

describe("formatWhen", () => {
	// Kills a twelve hour clock or a dropped minute.
	test("reads month, day, and a 24 hour time", () => {
		expect(formatWhen(Date.UTC(2026, 8, 13, 21, 6), "UTC")).toBe(
			"Sep 13, 21:06",
		);
	});
});

describe("formatWhenSeconds", () => {
	// Kills a dropped second or a twelve hour clock.
	test("reads month, day, and a 24 hour time to the second", () => {
		expect(formatWhenSeconds(Date.UTC(2026, 9, 18, 17, 8, 5), "UTC")).toBe(
			"Oct 18, 17:08:05",
		);
	});
});

describe("formatDate", () => {
	test("reads month and day", () => {
		expect(formatDate(Date.UTC(2026, 0, 2, 23, 59), "UTC")).toBe("Jan 2");
	});
});

describe("formatTicks", () => {
	const BASE = axisTier(1, "");
	const one = (value: number, step: number, tier = BASE) =>
		formatTicks([value], step, tier)[0];

	// Kills decimals that ignore the tick step.
	test.each([
		[20, 5, "20"],
		[2.5, 0.5, "2.5"],
		[0.25, 0.05, "0.25"],
		[12_500, 2500, "12,500"],
	])("labels %d at step %d as %s", (value, step, label) => {
		expect(one(value, step)).toBe(label);
	});

	// A power or log scale's ticks have no common step, so each keeps its own precision.
	test("labels a tick with no common step by its own precision", () => {
		expect(one(0.02, 0)).toBe("0.02");
		expect(one(5000, 0)).toBe("5,000");
		expect(one(123_456, 0)).toBe("123,000");
	});

	// Kills a label past twelve characters for a tick no tier bounds: it sheds decimals, then takes its power of ten.
	test("bounds any tick to twelve characters", () => {
		expect(one(-1000.000001, 1e-6)).toBe("-1,000.00000");
		expect(one(1e12, 1)).toBe("1e12");
	});

	// Kills ticks past the tiers printed in hundreds of digits, or every tiny tick printed as 0.
	test("labels ticks past the tiers by their power of ten", () => {
		const huge = axisTier(1e300, NS);
		expect(formatTicks([0, 0.5e300], 0.25e300, huge)).toEqual([
			"0.00",
			"0.50e300",
		]);
		expect(one(1e-300, 0, huge)).toBe("1e-300");
		expect(one(1.2e-300, 0.2e-300, axisTier(1.2e-300, NS))).toBe("1.2e-300");
		expect(one(1e-6, 0, axisTier(1e6, NS))).toBe("1e-6");
	});

	// Kills an axis that mixes notations, as 1e-97 beside 10,000.
	test("gives every label its power of ten when one needs it", () => {
		expect(formatTicks([1e-97, 1e4, 1e105], 0, BASE)).toEqual([
			"1e-97",
			"1e4",
			"1e105",
		]);
	});
});
