import { PRESETS } from "../explore/window";
import {
	type ExploreQuery,
	type QueryWindow,
	decodeQuery,
	encodeQuery,
} from "../query/query";

/** The public plot of a project, where its version 1 perf page lives until version 0 retires. */
export const publicPlotPath = (slug: string) => `/next/perf/${slug}`;

/**
 * The window the public plot offers for `window`: the smallest preset that
 * holds it, at most three months, the API's reach without a sign in.
 */
export const publicWindow = (window: QueryWindow, now: number): QueryWindow => {
	const seconds =
		"start" in window
			? ((window.end ?? now) - window.start) / 1000
			: window.seconds;
	const preset =
		PRESETS.find((entry) => entry.seconds >= seconds) ?? PRESETS.at(-1);
	return {
		seconds: preset?.seconds ?? seconds,
		...(window.end === undefined ? {} : { end: window.end }),
	};
};

/** Explore's query for a link, with the window one the public plot offers. */
export const publicQuery = (search: string, now: number): ExploreQuery => {
	const query = decodeQuery(search);
	return { ...query, window: publicWindow(query.window, now) };
};

/** The public plot for a link to the classic perf page or the public plot, in its own spelling. */
export const publicPlotHref = (slug: string, search: string, now: number) =>
	`${publicPlotPath(slug)}${encodeQuery(publicQuery(search, now))}`;
