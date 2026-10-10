import type { ParameterValue, Parameters, QueryWindow } from "../query/query";

/** A pinned line, named as a reader sees it. */
export interface PinTitleLine {
	readonly benchmark: string;
	readonly parameters: Parameters;
	/** The benchmark's variants, which decide the parameters that name the line. */
	readonly variants: readonly Parameters[];
	readonly measure: string;
	/** Named only when the caller shows more than one metric name. */
	readonly metric?: string;
	readonly branch: string;
}

/** The API's limit on a plot title. */
const MAX_TITLE_BYTES = 64;
const ELLIPSIS = "…";

/**
 * The benchmark with the parameters that vary across its variants, the measure, and the branch.
 * Several lines name what they share, and list at most two of anything.
 */
export const defaultPinTitle = (lines: readonly PinTitleLine[]): string => {
	const [first] = lines;
	if (first === undefined) {
		return "";
	}
	const varying = lines.map(varyingKeys);
	const tags = Object.entries(first.parameters)
		.filter(
			([key, value]) =>
				lines.every((line) => line.parameters[key] === value) &&
				varying.some((keys) => keys.has(key)),
		)
		.map(([key, value]) => `${key}=${String(value)}`);
	const named = (name: (line: PinTitleLine) => string | undefined) =>
		listed([...new Set(lines.flatMap((line) => name(line) ?? []))]);
	const head = [named(({ benchmark }) => benchmark), ...tags].join(" ");
	const what = [named(({ metric }) => metric), named(({ measure }) => measure)]
		.filter((part) => part.length > 0)
		.join(" ");
	return withinLimit(`${head}, ${what} on ${named(({ branch }) => branch)}`);
};

const listed = (names: readonly string[]): string => {
	const [first = "", second] = names;
	if (names.length > 2) {
		return `${first} and ${names.length - 1} more`;
	}
	return second === undefined ? first : `${first} and ${second}`;
};

/** The keys whose values differ across the benchmark's variants, a missing key counting as a value. */
const varyingKeys = (line: PinTitleLine): Set<string> => {
	const variants = [line.parameters, ...line.variants];
	const keys = new Set(variants.flatMap((variant) => Object.keys(variant)));
	return new Set(
		[...keys].filter(
			(key) =>
				new Set(variants.map((variant) => valueKey(variant[key]))).size > 1,
		),
	);
};

const valueKey = (value: ParameterValue | undefined): string =>
	value === undefined ? "" : JSON.stringify(value);

const encoder = new TextEncoder();
const bytes = (text: string): number => encoder.encode(text).length;

/** Cut at the last whole word that fits the API's limit, and mark the cut. */
const withinLimit = (title: string): string => {
	if (bytes(title) <= MAX_TITLE_BYTES) {
		return title;
	}
	const room = MAX_TITLE_BYTES - bytes(ELLIPSIS);
	let kept = "";
	for (const character of title) {
		if (bytes(kept + character) > room) {
			break;
		}
		kept += character;
	}
	const space = kept.lastIndexOf(" ");
	const whole =
		title[kept.length] === " " || space === -1 ? kept : kept.slice(0, space);
	return `${whole.replace(/[\s,]+$/u, "")}${ELLIPSIS}`;
};

/** The window a pin saves: the duration in seconds, rolling from whenever the pin is drawn. */
export const pinWindow = (window: QueryWindow, now: number): number =>
	"seconds" in window
		? window.seconds
		: Math.max(1, Math.round(((window.end ?? now) - window.start) / 1000));
