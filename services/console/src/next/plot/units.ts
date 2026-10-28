// The tiers of bencher_valid's unit scaling, with seconds read in nanoseconds so they reach below a second.

export interface Tier {
	factor: number;
	symbol: string;
}

const NANOSECONDS: readonly Tier[] = [
	{ factor: 1, symbol: "ns" },
	{ factor: 1e3, symbol: "µs" },
	{ factor: 1e6, symbol: "ms" },
	{ factor: 1e9, symbol: "s" },
	{ factor: 60e9, symbol: "m" },
	{ factor: 3.6e12, symbol: "h" },
];

const BYTES: readonly Tier[] = [
	{ factor: 1, symbol: "B" },
	{ factor: 1e3, symbol: "KB" },
	{ factor: 1e6, symbol: "MB" },
	{ factor: 1e9, symbol: "GB" },
	{ factor: 1e12, symbol: "TB" },
	{ factor: 1e15, symbol: "PB" },
];

/** A unit's tiers, and how many of the first tier one of the unit is. */
export interface Family {
	tiers: readonly Tier[];
	base: number;
}

const FAMILIES: Readonly<Record<string, Family>> = {
	"nanoseconds (ns)": { tiers: NANOSECONDS, base: 1 },
	"seconds (s)": { tiers: NANOSECONDS, base: 1e9 },
	"bytes (B)": { tiers: BYTES, base: 1 },
};

export const familyOf = (units: string): Family | undefined => FAMILIES[units];

/** The text in a unit's first parentheses, else the whole unit. */
export const symbolOf = (units: string): string =>
	(/\(([^)]*)\)/.exec(units)?.[1] ?? units).trim();
