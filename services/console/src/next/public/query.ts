import { decodeQuery, encodeQuery } from "../query/query";

/** The public plot of a project, where its version 1 perf page lives until version 0 retires. */
export const publicPlotPath = (slug: string) => `/next/perf/${slug}`;

/** The public plot's embed, for a frame on another site. */
export const embedPath = (slug: string) => `${publicPlotPath(slug)}/embed`;

/**
 * The public plot for a link to the classic perf page or the public plot, in
 * Explore's spelling and with the link's own window: the API cuts one past
 * three months to its last three, and the page says so.
 */
export const publicPlotHref = (slug: string, search: string) =>
	`${publicPlotPath(slug)}${encodeQuery(decodeQuery(search))}`;
