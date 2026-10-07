import type { UnitScale } from "./units";

/** The side or sides of a line that its threshold guards. */
export type Guard = "lower" | "upper" | "both";

export interface Delta {
	text: string;
	tone: "worse" | "better" | null;
}

const TWO_DECIMALS = new Intl.NumberFormat("en-US", {
	minimumFractionDigits: 2,
	maximumFractionDigits: 2,
});

export const formatValue = (value: number, scale: UnitScale): string => {
	const number = TWO_DECIMALS.format(value / scale.factor);
	return scale.symbol ? `${number} ${scale.symbol}` : number;
};

// A change under one percent names no direction.
const TONE_THRESHOLD = 0.01;

export const formatDelta = (
	value: number,
	baseline: number,
	guard: Guard,
): Delta | null => {
	if (baseline === 0) {
		return null;
	}
	const delta = (value - baseline) / baseline;
	const percent = Math.abs(delta * 100).toFixed(1);
	const text = `${delta < 0 && percent !== "0.0" ? "-" : "+"}${percent}%`;
	if (Math.abs(delta) < TONE_THRESHOLD) {
		return { text, tone: null };
	}
	const worse = guard === "both" || (guard === "upper" ? delta > 0 : delta < 0);
	return { text, tone: worse ? "worse" : "better" };
};

const whenFormats = new Map<string, Intl.DateTimeFormat>();
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

/** A date axis label, "Sep 13". */
export const formatDate = (ms: number, timeZone?: string): string =>
	cached(dateFormats, timeZone, { month: "short", day: "numeric" }).format(ms);

const tickFormats = new Map<number, Intl.NumberFormat>();

/** A y axis label; `step` is the distance between ticks, or 0 when they have none in common. */
export const formatTick = (value: number, step: number): string => {
	if (step === 0) {
		return tickFormat(decimalsOf(Number(value.toPrecision(3)))).format(
			Number(value.toPrecision(3)),
		);
	}
	return tickFormat(decimalsOf(step)).format(value);
};

const tickFormat = (decimals: number): Intl.NumberFormat => {
	let format = tickFormats.get(decimals);
	if (!format) {
		format = new Intl.NumberFormat("en-US", {
			minimumFractionDigits: decimals,
			maximumFractionDigits: decimals,
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
