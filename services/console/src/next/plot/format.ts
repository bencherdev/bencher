import { familyOf, symbolOf } from "./units";

/** The side or sides of a line that its threshold guards. */
export type Guard = "lower" | "upper" | "both";

export interface Delta {
	text: string;
	arrow: "↑" | "↓" | null;
	tone: "worse" | "better" | null;
}

/** The most characters a number prints in, its symbol included. */
export const NUMBER_MAX = 12;

/**
 * A measure's value in its own tier of the unit, two decimals from a tenth to
 * a thousand of it, else as a power of ten. Past twelve characters it sheds
 * decimals while two significant digits remain, then its symbol, never a digit.
 */
export const formatMeasure = (value: number, units: string): string => {
	if (!Number.isFinite(value)) {
		return "n/a";
	}
	if (value === 0) {
		return "0";
	}
	const place = placeOf(Math.abs(value), units);
	if ("factor" in place) {
		const mantissa = value / place.factor;
		return bounded(
			(decimals) => mantissa.toFixed(decimals),
			cut(place.symbol),
			Math.floor(Math.log10(Math.abs(mantissa))) + 1,
		);
	}
	const exponent = Number(value.toExponential().split("e")[1]);
	return bounded(
		(decimals) => exponential(value, decimals, place.step),
		cut(place.symbol),
		exponent - Math.floor(exponent / place.step) * place.step + 1,
	);
};

const SYMBOL_MAX = 5;

/** A value's symbol in five characters at most; an axis title carries it whole. */
const cut = (symbol: string) => {
	const characters = [...symbol];
	return characters.length > SYMBOL_MAX
		? `${characters.slice(0, SYMBOL_MAX - 1).join("")}…`
		: symbol;
};

/** How an axis prints its ticks: in `factor` of the unit, and past its unit's tiers over a shared `e{exponent}`. */
export interface AxisTier {
	factor: number;
	symbol: string;
	exponent: number;
}

/** The tier of an axis whose largest tick is `largest`. */
export const axisTier = (largest: number, units: string): AxisTier => {
	const size = Math.abs(largest);
	const place =
		size > 0 && Number.isFinite(size)
			? placeOf(size, units)
			: { factor: 1, symbol: symbolOf(units) };
	return "factor" in place
		? { ...place, exponent: 0 }
		: {
				factor: 1,
				symbol: place.symbol,
				exponent: exponentOf(size, place.step),
			};
};

/**
 * A y axis's labels; `values` and `step`, the distance between ticks or 0 when
 * they have none in common, are in `factor` of the unit. When one label needs
 * its power of ten, every label takes its own, so an axis reads in one notation.
 */
export const formatTicks = (
	values: readonly number[],
	step: number,
	tier: AxisTier,
): string[] => {
	const labels = values.map((value) => tickLabel(value, step, tier));
	return labels.every((label) => label !== undefined)
		? (labels as string[])
		: values.map((value) =>
				value === 0 ? "0" : power(Number(value.toPrecision(3))),
			);
};

/** A tick's label, or none when it needs its power of ten: past six decimals or a million with no common step, or past twelve characters with one. */
const tickLabel = (
	value: number,
	step: number,
	tier: AxisTier,
): string | undefined => {
	if (step === 0) {
		const shown = Number(value.toPrecision(3));
		return shown !== 0 && (Math.abs(shown) < 1e-3 || Math.abs(shown) >= 1e6)
			? undefined
			: tickFormat(decimalsOf(shown)).format(shown);
	}
	const scale = 10 ** tier.exponent;
	let decimals = decimalsOf(step / scale);
	let text = label(value / scale, decimals, tier.exponent);
	while (text.length > NUMBER_MAX && decimals > 0) {
		text = label(value / scale, --decimals, tier.exponent);
	}
	return text.length > NUMBER_MAX ? undefined : text;
};

const label = (value: number, decimals: number, exponent: number) => {
	const text = tickFormat(decimals).format(value);
	return exponent === 0 || Number(text.replaceAll(",", "")) === 0
		? text
		: `${text}e${exponent}`;
};

const power = (value: number) => value.toExponential().replace("e+", "e");

/** Past a thousand percent, a change reads as the ratio of the value to its baseline. */
const PERCENT_MAX = 1000;

/** How far `value` moved from `baseline`: `+12.3%`, `×50.0`, or `from 0`. */
export const formatChange = (value: number, baseline: number): string => {
	if (!Number.isFinite(value) || !Number.isFinite(baseline)) {
		return "n/a";
	}
	if (value === baseline) {
		return "+0.0%";
	}
	if (baseline === 0) {
		return "from 0";
	}
	const change = changeRatio(value, baseline) * 100;
	const percent = Math.abs(change).toFixed(1);
	const sign = change < 0 && percent !== "0.0" ? "-" : "+";
	if (Number(percent) < PERCENT_MAX) {
		return `${sign}${percent}%`;
	}
	// Past the largest double only the power of ten is left.
	const powers = Math.round(
		Math.log10(Math.abs(value)) - Math.log10(Math.abs(baseline)),
	);
	// A ratio has a reading only between values of one sign.
	if (Math.sign(value) === Math.sign(baseline)) {
		const ratio = value / baseline;
		return Number.isFinite(ratio)
			? `×${ratio < 999.5 ? ratio.toPrecision(3) : exponential(ratio, 2, 1)}`
			: `×1e${powers}`;
	}
	return `${sign}${Number.isFinite(change) ? exponential(Math.abs(change), 1, 1) : `1e${powers + 2}`}%`;
};

// Over the baseline's size, so a negative baseline keeps the sign of the move, as the API's delta sort does; divided first, so two values near the largest double do not overflow.
const changeRatio = (value: number, baseline: number) =>
	value / Math.abs(baseline) - Math.sign(baseline);

// A change under one percent names no direction.
const NOISE = 0.01;

/** The change from `baseline` to `value`, and whether the side `guard` watches makes it worse. */
export const changeOf = (
	value: number,
	baseline: number,
	guard: Guard,
): Delta => {
	const text = formatChange(value, baseline);
	const delta = value === baseline ? 0 : changeRatio(value, baseline);
	if (!(Math.abs(delta) >= NOISE)) {
		return { text, arrow: null, tone: null };
	}
	const worse = guard === "both" || (guard === "upper" ? delta > 0 : delta < 0);
	return {
		text,
		arrow: delta > 0 ? "↑" : "↓",
		tone: worse ? "worse" : "better",
	};
};

const whenFormats = new Map<string, Intl.DateTimeFormat>();
const secondFormats = new Map<string, Intl.DateTimeFormat>();
const dateFormats = new Map<string, Intl.DateTimeFormat>();

const cached = (
	formats: Map<string, Intl.DateTimeFormat>,
	timeZone: string | undefined,
	options: Intl.DateTimeFormatOptions,
): Intl.DateTimeFormat => {
	const key = timeZone ?? "";
	let format = formats.get(key);
	if (!format) {
		format = new Intl.DateTimeFormat("en-US", { ...options, timeZone });
		formats.set(key, format);
	}
	return format;
};

/** A report's time, "Sep 13, 21:06", in the reader's time zone unless one is given. */
export const formatWhen = (ms: number, timeZone?: string): string =>
	cached(whenFormats, timeZone, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
		hourCycle: "h23",
	}).format(ms);

/** A report's time to the second, "Oct 18, 17:08:05", in the reader's time zone unless one is given. */
export const formatWhenSeconds = (ms: number, timeZone?: string): string =>
	cached(secondFormats, timeZone, {
		month: "short",
		day: "numeric",
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		hourCycle: "h23",
	}).format(ms);

/** A date axis label, "Sep 13". */
export const formatDate = (ms: number, timeZone?: string): string =>
	cached(dateFormats, timeZone, { month: "short", day: "numeric" }).format(ms);

/** Where a size prints: in a tier of its unit, `factor` of the unit to one of the tier, or as a power of ten, a multiple of `step`. */
type Place =
	| { factor: number; symbol: string }
	| { step: 1 | 3; symbol: string };

/** Under a tenth of the first tier, or at a thousand of the last, a size leaves the tiers. */
const FIXED_MIN = 0.1;
const FIXED_MAX = 1000;

const placeOf = (size: number, units: string): Place => {
	const symbol = symbolOf(units);
	const family = familyOf(units);
	if (!family) {
		return size >= FIXED_MIN && rounded(size) < FIXED_MAX
			? { factor: 1, symbol }
			: { step: 3, symbol };
	}
	const { tiers, base } = family;
	const scaled = size * base;
	let index = tiers.length - 1;
	while (index > 0 && scaled < (tiers[index]?.factor ?? 0)) {
		index--;
	}
	// Rounding carries a size into the next tier, so 59.999 seconds reads as a minute.
	const next = tiers[index + 1];
	if (
		next &&
		rounded(scaled / (tiers[index]?.factor ?? 1)) >=
			next.factor / (tiers[index]?.factor ?? 1)
	) {
		index++;
	}
	const tier = tiers[index];
	if (
		!tier ||
		(index === 0 && scaled / tier.factor < FIXED_MIN) ||
		(index === tiers.length - 1 && rounded(scaled / tier.factor) >= FIXED_MAX)
	) {
		return { step: 1, symbol };
	}
	return { factor: tier.factor / base, symbol: tier.symbol };
};

const DECIMALS = 2;

const rounded = (value: number) => Number(value.toFixed(DECIMALS));

/** `number` at the fewest decimals that fit with its symbol and keep two significant digits of a mantissa with `whole` digits before its point, as it was before rounding. */
const bounded = (
	number: (decimals: number) => string,
	symbol: string,
	whole: number,
): string => {
	const room = NUMBER_MAX - (symbol ? [...symbol].length + 1 : 0);
	let text = number(DECIMALS);
	for (
		let decimals = DECIMALS - 1;
		text.length > room && decimals >= 0 && whole + decimals >= 2;
		decimals--
	) {
		text = number(decimals);
	}
	return text.length > room || !symbol ? text : `${text} ${symbol}`;
};

/** `value` as a mantissa with `decimals` times a power of ten that is a multiple of `step`: 1 for scientific, 3 for engineering. */
const exponential = (value: number, decimals: number, step: 1 | 3): string => {
	let exponent = Number(value.toExponential().split("e")[1]);
	let shift = exponent - Math.floor(exponent / step) * step;
	let [mantissa = "", written = ""] = value
		.toExponential(shift + decimals)
		.split("e");
	// Rounding up to the next power of ten gives the mantissa another whole digit, or moves it to the next power, where a lone digit takes a decimal.
	if (Number(written) !== exponent) {
		exponent = Number(written);
		shift = exponent - Math.floor(exponent / step) * step;
		[mantissa = ""] = Number(`${mantissa}e${written}`)
			.toExponential(shift + Math.max(decimals, 1 - shift))
			.split("e");
	}
	const sign = mantissa.startsWith("-") ? "-" : "";
	const digits = mantissa.replace("-", "").replace(".", "");
	const whole = digits.slice(0, shift + 1);
	const fraction = digits.slice(shift + 1);
	return `${sign}${whole}${fraction ? `.${fraction}` : ""}e${exponent - shift}`;
};

/** The power of ten, a multiple of `step`, of a positive finite `size` at two decimals. */
const exponentOf = (size: number, step: 1 | 3) => {
	const exponent = Number(size.toExponential(DECIMALS).split("e")[1]);
	return Math.floor(exponent / step) * step;
};

const tickFormats = new Map<number, Intl.NumberFormat>();

const tickFormat = (decimals: number): Intl.NumberFormat => {
	let format = tickFormats.get(decimals);
	if (!format) {
		format = new Intl.NumberFormat("en-US", {
			minimumFractionDigits: decimals,
			maximumFractionDigits: decimals,
			signDisplay: "negative",
		});
		tickFormats.set(decimals, format);
	}
	return format;
};

const MAX_DECIMALS = 6;

const decimalsOf = (value: number): number => {
	for (let decimals = 0; decimals < MAX_DECIMALS; decimals++) {
		const scaled = Math.abs(value) * 10 ** decimals;
		if (Math.abs(scaled - Math.round(scaled)) < 1e-6 * Math.max(1, scaled)) {
			return decimals;
		}
	}
	return MAX_DECIMALS;
};
