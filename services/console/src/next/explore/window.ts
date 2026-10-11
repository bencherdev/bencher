import type { QueryWindow } from "../query/query";

const DAY = 24 * 60 * 60;

export const PRESETS = [
	{ value: "1w", seconds: 7 * DAY },
	{ value: "4w", seconds: 28 * DAY },
	// The longest three calendar months run, as the API's reach for a public plot.
	{ value: "3m", seconds: 92 * DAY },
] as const;

export type WindowChoice = (typeof PRESETS)[number]["value"] | "custom";

/** The preset a window is, `custom` for a range, or none for another rolling length. */
export const windowChoice = (window: QueryWindow): WindowChoice | undefined =>
	"start" in window
		? "custom"
		: PRESETS.find(({ seconds }) => seconds === window.seconds)?.value;

/** The window a choice makes from `window`, ending where it ended. */
export const pickWindow = (
	window: QueryWindow,
	choice: WindowChoice,
	now: number,
): QueryWindow => {
	const end = window.end;
	const ending = end === undefined ? {} : { end };
	if (choice === "custom") {
		return "start" in window
			? window
			: { start: (end ?? now) - window.seconds * 1000, end: end ?? now };
	}
	const preset = PRESETS.find(({ value }) => value === choice);
	return { seconds: preset?.seconds ?? 28 * DAY, ...ending };
};

const DATE = /^(\d{4})-(\d{2})-(\d{2})$/;

/** A date input's value for `time`, in the reader's time zone. */
export const dateValue = (time: number) => {
	const date = new Date(time);
	const pad = (n: number) => String(n).padStart(2, "0");
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
};

/** The whole days from `from` through `to`, or none when they are not a range. */
export const customRange = (
	from: string,
	to: string,
): QueryWindow | undefined => {
	const first = DATE.exec(from);
	const last = DATE.exec(to);
	if (!(first && last)) {
		return undefined;
	}
	const day = ([, y, m, d]: RegExpExecArray, offset = 0) =>
		new Date(Number(y), Number(m) - 1, Number(d) + offset).getTime();
	const start = day(first);
	const end = day(last, 1) - 1;
	return start < end ? { start, end } : undefined;
};

/** A pin's rolling window as a reader names it. */
export const rollingName = (seconds: number): string => {
	const preset = PRESETS.find((entry) => entry.seconds === seconds);
	if (preset) {
		return preset.value;
	}
	const { count, unit } = rollingParts(seconds);
	return count === 1 ? `1 ${unit.slice(0, -1)}` : `${count} ${unit}`;
};

export type RollingUnit = "days" | "weeks";

/** The longest rolling window a pin keeps, as the console's report window. */
const MAX_DAYS = 366;

/** A rolling window of `count` whole days or weeks, in seconds. */
export const rollingSeconds = (count: number, unit: RollingUnit): number => {
	const days =
		(Number.isFinite(count) ? Math.max(1, Math.round(count)) : 1) *
		(unit === "weeks" ? 7 : 1);
	return Math.min(MAX_DAYS, days) * DAY;
};

/** A rolling window in whole weeks when it is, else in days. */
export const rollingParts = (
	seconds: number,
): { count: number; unit: RollingUnit } => {
	const days = Math.max(1, Math.round(seconds / DAY));
	return days % 7 === 0
		? { count: days / 7, unit: "weeks" }
		: { count: days, unit: "days" };
};
