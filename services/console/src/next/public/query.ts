import { type ExploreQuery, encodeQuery } from "../query/query";

/** A public project's plot. */
export const publicPlotPath = (slug: string) => `/perf/${slug}`;

/** The public plot's embed, for a frame on another site. */
export const embedPath = (slug: string) => `${publicPlotPath(slug)}/embed`;

/** A link to `query` at `path` for a reader outside the console, where the pin the query came from is not theirs to open. */
export const publicUrl = (
	origin: string,
	path: string,
	query: ExploreQuery,
): string => {
	const { plot: _plot, ...shared } = query;
	return `${origin}${path}${encodeQuery(shared)}`;
};
