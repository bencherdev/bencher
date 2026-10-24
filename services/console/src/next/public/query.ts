/** A public project's plot. */
export const publicPlotPath = (slug: string) => `/perf/${slug}`;

/** The public plot's embed, for a frame on another site. */
export const embedPath = (slug: string) => `${publicPlotPath(slug)}/embed`;
