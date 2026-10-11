/** The API pages at most this many. */
const MAX_BATCH = 255;
/** Before the window has a height, a laptop screen. */
const FALLBACK_SCREEN = 720;

/** One batch of rows: the screen's worth, plus half again so a scroll never waits. */
export const batchSize = (screen: number, rowHeight: number) => {
	const fit = Math.ceil((screen > 0 ? screen : FALLBACK_SCREEN) / rowHeight);
	return Math.min(MAX_BATCH, fit + Math.ceil(fit / 2));
};

export interface Range {
	start: number;
	end: number;
}

/**
 * The rows to draw: those on screen plus `overscan` either side. `top` is where
 * the first row's top edge sits, relative to the top of the screen.
 */
export const visibleRange = ({
	top,
	viewport,
	rowHeight,
	count,
	overscan,
}: {
	top: number;
	viewport: number;
	rowHeight: number;
	count: number;
	overscan: number;
}): Range => {
	const first = Math.floor(Math.max(0, -top) / rowHeight) - overscan;
	const last = Math.ceil((viewport - top) / rowHeight) + overscan;
	const start = Math.min(count, Math.max(0, first));
	return { start, end: Math.min(count, Math.max(start, last)) };
};

/**
 * Ask for the next batch once the last row on screen comes within a quarter
 * batch of the last loaded: the first batch leaves half a screen below it, so
 * a quarter waits for the reader to scroll.
 */
export const loadMore = ({
	end,
	loaded,
	batch,
}: {
	end: number;
	loaded: number;
	batch: number;
}) => loaded - end <= batch / 4;

/** Whether a batch follows `last`: it came back full and the rows loaded fall short of the total. */
export const hasMore = (
	last: { reports: unknown[]; total: number; batch: { perPage: number } },
	loaded: number,
) => loaded < last.total && last.reports.length === last.batch.perPage;
