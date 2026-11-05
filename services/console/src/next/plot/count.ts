// Apart from the measure formatters, so a page that prints only counts loads none of them.

const COUNT = new Intl.NumberFormat("en-US");

/** The most a badge counts before it reads `99+`. */
export const BADGE_MAX = 99;

/** A count with its thousands grouped, `max+` past the `max` a badge holds, and nothing while it is unknown. */
export const formatCount = (count: number | undefined, max?: number): string =>
	count === undefined
		? ""
		: max !== undefined && count > max
			? `${COUNT.format(max)}+`
			: COUNT.format(count);
