import { describe, expect, test } from "vitest";
import { axisTicks } from "./axis";
import { formatTicks, NUMBER_MAX } from "./format";

const SIZES = [
	0,
	1e-300,
	1e-150,
	1e-12,
	0.003,
	1,
	999.999,
	1234.5,
	5e9,
	1e12,
	1e150,
	1e300,
	Number.MAX_VALUE,
];
const VALUES = [
	...SIZES,
	...SIZES.filter((size) => size > 0).map((size) => -size),
];
const PAIRS: [number, number][] = [
	...VALUES.flatMap((min) =>
		VALUES.filter((max) => max >= min).map((max): [number, number] => [
			min,
			max,
		]),
	),
	// A range so narrow that its step takes every decimal a label shows.
	[-1000.000001, -1000],
];
/** A tick in its own power of ten, as an axis that needs one writes every tick. */
const powerOf = (tick: number) =>
	Number(tick.toPrecision(3)).toExponential().replace("e+", "e");

const UNITS = [
	"nanoseconds (ns)",
	"seconds (s)",
	"bytes (B)",
	"requests per second (req/s)",
	"",
];

describe("axisTicks", () => {
	// Kills a tick label past twelve characters at any magnitude, scale, or unit, an axis in two notations, or a range that spins or fails.
	test.each(["auto", "linear", "log"] as const)(
		"every %s axis labels its ticks in twelve characters and one notation",
		(mode) => {
			for (const [min, max] of PAIRS) {
				for (const units of UNITS) {
					for (const count of [3, 4, 5]) {
						const drawn = axisTicks([{ y: [min, max] }], units, mode, count);
						if (!drawn) {
							continue;
						}
						const labels = formatTicks(drawn.ticks, drawn.step, drawn.tier);
						const at = `${min} to ${max} ${units} ${count}: ${labels.join(" | ")}`;
						for (const label of labels) {
							expect(label.length, at).toBeLessThanOrEqual(NUMBER_MAX);
							expect(label, at).not.toMatch(/NaN|Infinity/);
						}
						// One notation per axis: no tick in a power of ten, every tick in its own, or every one over a shared one.
						const nonzero = drawn.ticks.flatMap((tick, index) => {
							const label = labels[index] ?? "";
							return Number(label.replaceAll(",", "").split("e")[0]) === 0
								? []
								: [{ tick, label }];
						});
						const suffixes = new Set(
							nonzero.map(({ label }) => /e-?\d+$/.exec(label)?.[0]),
						);
						const own = nonzero.every(
							({ tick, label }) => label === powerOf(tick),
						);
						expect(own || suffixes.size <= 1, at).toBe(true);
					}
				}
			}
		},
	);
});
