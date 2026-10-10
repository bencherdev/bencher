// A port of bencher_valid's unit scaling, whose WASM build is 352 KB gzipped.

export interface UnitScale {
	factor: number;
	symbol: string;
}

const NANOSECONDS: readonly UnitScale[] = [
	{ factor: 1, symbol: "ns" },
	{ factor: 1e3, symbol: "µs" },
	{ factor: 1e6, symbol: "ms" },
	{ factor: 1e9, symbol: "s" },
	{ factor: 60e9, symbol: "m" },
	{ factor: 3.6e12, symbol: "h" },
];

const SECONDS: readonly UnitScale[] = [
	{ factor: 1, symbol: "s" },
	{ factor: 60, symbol: "m" },
	{ factor: 3600, symbol: "h" },
];

const BYTES: readonly UnitScale[] = [
	{ factor: 1, symbol: "B" },
	{ factor: 1e3, symbol: "KB" },
	{ factor: 1e6, symbol: "MB" },
	{ factor: 1e9, symbol: "GB" },
	{ factor: 1e12, symbol: "TB" },
	{ factor: 1e15, symbol: "PB" },
];

const POWERS = [1, 1e3, 1e6, 1e9, 1e12, 1e15];

const FIXED: Readonly<Record<string, readonly UnitScale[]>> = {
	"nanoseconds (ns)": NANOSECONDS,
	"seconds (s)": SECONDS,
	"bytes (B)": BYTES,
};

/** The units to show values in when the smallest is `min`. */
export const unitScale = (min: number, units: string): UnitScale => {
	const fixed = FIXED[units];
	if (fixed) {
		return fixed[
			tier(
				min,
				fixed.map(({ factor }) => factor),
			)
		] as UnitScale;
	}
	const power = tier(min, POWERS);
	const factor = POWERS[power] as number;
	const suffix = power === 0 ? "" : `x 1e${power * 3}`;
	const symbol = symbolOf(units);
	if (symbol === undefined) {
		return { factor, symbol: suffix };
	}
	return { factor, symbol: suffix ? `${symbol} ${suffix}` : symbol };
};

// NaN fails every comparison and lands on the last tier, as the Rust match does.
const tier = (min: number, factors: readonly number[]): number => {
	for (let index = 1; index < factors.length; index++) {
		if (min < (factors[index] as number)) {
			return index - 1;
		}
	}
	return factors.length - 1;
};

const symbolOf = (units: string): string | undefined => {
	const open = units.indexOf("(");
	if (open < 0) {
		return undefined;
	}
	const close = units.indexOf(")", open + 1);
	return close < 0 ? undefined : units.slice(open + 1, close);
};
