import { shortHash } from "../reports/format";
import { formatCount } from "../plot/count";

/** A run by its version: the short hash, else the version number. */
export const runHead = (run: { hash?: string | undefined; number: number }) =>
	shortHash(run.hash) ?? `v${run.number}`;

/** A list the API names at most a few of, with how many it holds in all. */
export const allNames = (names: readonly string[], count: number) => {
	const rest = count - names.length;
	return rest > 0
		? `${names.join(", ")}, and ${formatCount(rest)} more`
		: names.join(", ");
};

/** The time of day to the second, which tells reports started together apart. */
export const secondsTime = (time: number, timeZone?: string) =>
	new Intl.DateTimeFormat("en-US", {
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		hourCycle: "h23",
		timeZone,
	}).format(time);
