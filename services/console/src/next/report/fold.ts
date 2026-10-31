/** The table widths, in px, at or below which each list's Line column holds under 16 characters, so its rows fold into two lines; where the Report column widens, from 1280 px, the table is wider than its fold. */
export const FOLD = {
	report: 880,
	alerts: 1042,
	run: 950,
	threshold: 1042,
} as const;

/** The table widths, in px, at or below which a folded row's name holds under 16 characters beside its numbers, so they take a line of their own; with a checkbox and without. */
export const STACK = { select: 381, plain: 349 } as const;
