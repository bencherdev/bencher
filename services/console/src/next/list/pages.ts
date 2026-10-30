import type { QueryClient } from "@tanstack/solid-query";

/** One page of a list as the cache holds it: its rows and every row the list holds. */
export interface Page<T> {
	rows: T[];
	total: number;
	page: number;
}

/** The rows a list holds, from the API's `X-Total-Count`, else the rows it answered. */
export const totalOf = (headers: Headers, rows: readonly unknown[]) => {
	const total = Number(headers.get("X-Total-Count"));
	return headers.has("X-Total-Count") && Number.isFinite(total)
		? total
		: rows.length;
};

export type FetchPage<T> = (
	page: number,
	perPage: number,
	signal: AbortSignal,
) => Promise<Omit<Page<T>, "page">>;

/** One page of a list as its own query, which a link can start before the list asks. */
export const pageQuery = <T>(
	queryKey: readonly unknown[],
	perPage: number,
	page: number,
	fetchPage: FetchPage<T>,
) => ({
	queryKey: [...queryKey, perPage, page],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Page<T>> => ({
		...(await fetchPage(page, perPage, signal)),
		page,
	}),
});

/** Change every cached page under `queryKey`, as the page sees itself. */
export const editPages = <T>(
	client: QueryClient,
	queryKey: readonly unknown[],
	edit: (page: Page<T>) => Page<T>,
) => {
	for (const [key, page] of client.getQueriesData<Page<T>>({ queryKey })) {
		if (page) {
			client.setQueryData<Page<T>>(key, edit(page));
		}
	}
};

/** The row with `uuid` in the cached pages under `queryKey`, and where it is. */
export const findRow = <T extends { uuid: string }>(
	client: QueryClient,
	queryKey: readonly unknown[],
	uuid: string,
) => {
	for (const [, page] of client.getQueriesData<Page<T>>({ queryKey })) {
		const index = page?.rows.findIndex((row) => row.uuid === uuid) ?? -1;
		const row = page?.rows[index];
		if (page && row) {
			return { row, page: page.page, index };
		}
	}
	return undefined;
};

/** Drop the row with `uuid` from its page, and one from every page's count; returns the undo. */
export const dropRow = <T extends { uuid: string }>(
	client: QueryClient,
	queryKey: readonly unknown[],
	uuid: string,
) => {
	const found = findRow<T>(client, queryKey, uuid);
	if (!found) {
		return () => {};
	}
	editPages<T>(client, queryKey, (page) => ({
		...page,
		rows: page.rows.filter((row) => row.uuid !== uuid),
		total: page.total - 1,
	}));
	// The undo puts back this row alone: another change may have landed since.
	return () => putRow(client, queryKey, found.row, found.page, found.index);
};

/** Put `row` in page `at` at `index`, and one on every page's count, unless a page holds it already. */
export const putRow = <T extends { uuid: string }>(
	client: QueryClient,
	queryKey: readonly unknown[],
	row: T,
	at = 1,
	index = 0,
) => {
	if (findRow(client, queryKey, row.uuid)) {
		return;
	}
	editPages<T>(client, queryKey, (page) => ({
		...page,
		rows:
			page.page === at
				? [...page.rows.slice(0, index), row, ...page.rows.slice(index)]
				: page.rows,
		total: page.total + 1,
	}));
};
