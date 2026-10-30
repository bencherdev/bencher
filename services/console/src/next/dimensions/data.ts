import type { QueryClient } from "@tanstack/solid-query";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonConsoleBenchmarkRow,
	JsonConsoleBranchRow,
	JsonConsoleMeasureRow,
	JsonConsoleRuns,
	JsonConsoleTestbedRow,
	JsonMeasure,
	JsonTestbed,
	JsonThreshold,
	JsonVariant,
} from "../../types/bencher";
import type { Api } from "../api";
import { NARROW } from "../plot/narrow";
import { type FetchPage, type Page, pageQuery, totalOf } from "../list/pages";
import { batchSize } from "../list/rows";
import { type Dimension, KINDS } from "./dimension";
import { type ListSearch, apiParams, encodeSearch } from "./search";

export type Row =
	| JsonConsoleBranchRow
	| JsonConsoleTestbedRow
	| JsonConsoleBenchmarkRow
	| JsonConsoleMeasureRow;

export interface RowBatch {
	rows: Row[];
	/** The rows the search matches, over every batch. */
	total: number;
	/** Every active row, whatever the search. */
	active: number;
	/** Every archived row, whatever the search. */
	archived: number;
	batch: ListBatch;
}

/** A list's first batch, its second, and so on, each of `perPage` rows. */
export interface ListBatch {
	ordinal: number;
	perPage: number;
}

/** The thresholds that archive and come back with a row; none for a benchmark. */
export const thresholdsOf = (row: Row) =>
	"thresholds" in row ? row.thresholds : undefined;

/** The thresholds on a row that another archived dimension keeps archived. */
export const heldOf = (row: Row) =>
	"held_thresholds" in row ? row.held_thresholds : 0;

/** The fixed height a row is drawn at, wide and narrow. */
export const ROW_HEIGHT = { wide: 40, narrow: 60 } as const;

export const screenBatch = () =>
	batchSize(
		window.innerHeight,
		window.matchMedia(NARROW).matches ? ROW_HEIGHT.narrow : ROW_HEIGHT.wide,
	);

const project = (slug: string) => `/v0/projects/${encodeURIComponent(slug)}`;
const consolePath = (slug: string) =>
	`/v0/console/projects/${encodeURIComponent(slug)}`;

const listPrefix = (dimension: Dimension, slug: string) => [
	"console",
	dimension,
	slug,
	"list",
];

/**
 * One batch of a dimension's list, its own query so it revalidates and retries
 * on its own. It starts where `offset` says once it is asked: the rows before
 * it can change on screen after it is made.
 */
export const listQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	search: ListSearch,
	batch: ListBatch,
	offset: () => number | Promise<number> = () => 0,
) => ({
	queryKey: [
		...listPrefix(dimension, slug),
		encodeSearch(search),
		batch.perPage,
		batch.ordinal,
	],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<RowBatch> => {
		const params = apiParams(search, {
			offset: await offset(),
			perPage: batch.perPage,
		});
		const { data } = await api.get<
			Pick<RowBatch, "total" | "active" | "archived"> &
				Partial<Record<Dimension, Row[]>>
		>(`${consolePath(slug)}/${dimension}?${params}`, signal);
		return {
			rows: data[dimension] ?? [],
			total: data.total,
			active: data.active,
			archived: data.archived,
			batch,
		};
	},
	// A new search keeps the rows on screen, marked busy, until its first batch answers.
	placeholderData: (
		previous: RowBatch | undefined,
		query: { queryKey: readonly unknown[] } | undefined,
	) =>
		batch.ordinal === 1 &&
		query?.queryKey[1] === dimension &&
		query.queryKey[2] === slug
			? previous
			: undefined,
});

/**
 * Show a row archived at `archived`, or active when it is undefined, in every
 * cached batch of its dimension. The totals move only when that changes the
 * row's shown state: `was` when the caller knows it, else as the cache shows
 * the row.
 */
export const markArchived = (
	client: QueryClient,
	slug: string,
	dimension: Dimension,
	uuid: string,
	archived: number | undefined,
	was?: boolean,
) => {
	const batches = client.getQueriesData<RowBatch>({
		queryKey: listPrefix(dimension, slug),
	});
	const before =
		was ??
		Boolean(
			batches
				.flatMap(([, batch]) => batch?.rows ?? [])
				.find((row) => row.uuid === uuid)?.archived,
		);
	const after = archived !== undefined;
	const moved = before === after ? 0 : after ? 1 : -1;
	for (const [key, batch] of batches) {
		if (batch) {
			client.setQueryData<RowBatch>(key, {
				...batch,
				rows: batch.rows.map((row) =>
					row.uuid === uuid ? withArchived(row, archived) : row,
				),
				active: batch.active - moved,
				archived: batch.archived + moved,
			});
		}
	}
};

export const withArchived = <T extends { archived?: number | string }>(
	row: T,
	archived: T["archived"],
): T => {
	const { archived: _was, ...rest } = row;
	return (archived === undefined ? rest : { ...rest, archived }) as T;
};

/** Everything cached for the project but this dimension's lists is stale once a row's state changes. */
export const afterArchive = (
	client: QueryClient,
	slug: string,
	dimension: Dimension,
) => {
	client.invalidateQueries({
		predicate: ({ queryKey }) =>
			queryKey[0] === "console" &&
			queryKey[2] === slug &&
			!(queryKey[1] === dimension && queryKey[3] === "list"),
	});
	// The rows on screen keep the change in place until the reader leaves.
	client.invalidateQueries({
		queryKey: listPrefix(dimension, slug),
		refetchType: "none",
	});
};

/** A row changed on the page, as the page last set it, and the row it followed then. */
export interface KeptRow<T extends { uuid: string } = Row> {
	row: T;
	after: string | undefined;
}

/**
 * The rows listed, each kept row as the page last set it, and each kept row
 * the list no longer holds back after the row it followed.
 */
export const withKept = <T extends { uuid: string }>(
	listed: readonly T[],
	kept: readonly KeptRow<T>[],
): T[] => {
	const shown = new Map(kept.map(({ row }) => [row.uuid, row]));
	const held = new Set(listed.map(({ uuid }) => uuid));
	const following = new Map<string | undefined, T[]>();
	for (const { row, after } of kept) {
		if (!held.has(row.uuid)) {
			following.set(after, [...(following.get(after) ?? []), row]);
		}
	}
	const drawn = new Map<string, T>();
	const place = (row: T) => {
		if (!drawn.has(row.uuid)) {
			drawn.set(row.uuid, shown.get(row.uuid) ?? row);
			for (const next of following.get(row.uuid) ?? []) {
				place(next);
			}
		}
	};
	for (const row of [
		...(following.get(undefined) ?? []),
		...listed,
		...[...following.values()].flat(),
	]) {
		place(row);
	}
	return [...drawn.values()];
};

/** The archived flag the API takes for one of a dimension's rows. */
export const archivePath = (slug: string, dimension: Dimension, uuid: string) =>
	`${project(slug)}/${dimension}/${uuid}`;

export type Detail = JsonBranch | JsonTestbed | JsonBenchmark | JsonMeasure;

const detailKey = (dimension: Dimension, slug: string, ref: string) => [
	"console",
	dimension,
	slug,
	"detail",
	ref,
];

/** One dimension, by the slug or UUID its page's path names. */
export const detailQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	ref: string,
) => ({
	queryKey: detailKey(dimension, slug, ref),
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(
			await api.get<Detail>(
				`${project(slug)}/${dimension}/${encodeURIComponent(ref)}`,
				signal,
			)
		).data,
});

/** Show a dimension's page archived at `archived`, or active; returns whether it showed archived before. */
export const markDetailArchived = (
	client: QueryClient,
	slug: string,
	dimension: Dimension,
	ref: string,
	archived: string | undefined,
) => {
	const key = detailKey(dimension, slug, ref);
	const before = Boolean(client.getQueryData<Detail>(key)?.archived);
	client.setQueryData<Detail>(key, (detail) =>
		detail ? withArchived(detail, archived) : detail,
	);
	return before;
};

// The API's largest page.
const SEARCH_PAGE = 255;

/**
 * A dimension's row in its list, with what it carries there: the last report,
 * the start point's name, and the thresholds that would archive with it. The
 * list only searches, so the exact row is picked from what it matches, a page
 * at a time until it turns up.
 */
export const rowQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	ref: string,
	archived: boolean,
) => ({
	queryKey: ["console", dimension, slug, "row", ref, archived],
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const search = new URLSearchParams({
			search: ref,
			per_page: String(SEARCH_PAGE),
		});
		if (archived) {
			search.set("archived", "true");
		}
		for (let page = 1; ; page++) {
			search.set("page", String(page));
			const { data } = await api.get<
				{ total: number } & Partial<Record<Dimension, Row[]>>
			>(`${consolePath(slug)}/${dimension}?${search}`, signal);
			const rows = data[dimension] ?? [];
			const found = rows.find((row) => row.slug === ref || row.uuid === ref);
			if (
				found ||
				rows.length < SEARCH_PAGE ||
				page * SEARCH_PAGE >= data.total
			) {
				return found ?? null;
			}
		}
	},
	placeholderData: keepPrevious,
});

const keepPrevious = <T>(previous: T | undefined) => previous;

// A dimension carries a handful of thresholds: a page holds most lists whole.
export const THRESHOLDS_PAGE = 64;

export const thresholdsKey = (
	slug: string,
	dimension: Dimension,
	ref: string,
	archived: boolean,
) => ["console", "thresholds", slug, "on", dimension, ref, archived];

export const thresholdsPage =
	(
		api: Api,
		slug: string,
		dimension: Dimension,
		ref: string,
		archived: boolean,
	): FetchPage<JsonThreshold> =>
	async (page, perPage, signal) => {
		const search = new URLSearchParams({
			[KINDS[dimension].threshold ?? dimension]: ref,
			archived: String(archived),
			per_page: String(perPage),
			page: String(page),
		});
		const { data, headers } = await api.get<JsonThreshold[]>(
			`${project(slug)}/thresholds?${search}`,
			signal,
		);
		return { rows: data, total: totalOf(headers, data) };
	};

/** The first page of the thresholds on a branch, testbed, or measure that share its archived state. */
export const thresholdsQuery = (
	api: Api,
	slug: string,
	dimension: Dimension,
	ref: string,
	archived: boolean,
) =>
	pageQuery(
		thresholdsKey(slug, dimension, ref, archived),
		THRESHOLDS_PAGE,
		1,
		thresholdsPage(api, slug, dimension, ref, archived),
	);

const RECENT_RUNS = 5;

/** A branch's or a testbed's newest runs, whenever they ran. */
export const recentRunsQuery = (
	api: Api,
	slug: string,
	dimension: "branches" | "testbeds",
	ref: string,
) => ({
	queryKey: ["console", "runs", slug, "recent", dimension, ref],
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const search = new URLSearchParams({
			[dimension === "branches" ? "branch" : "testbed"]: ref,
			per_page: String(RECENT_RUNS),
		});
		const { data } = await api.get<JsonConsoleRuns>(
			`${consolePath(slug)}/runs?${search}`,
			signal,
		);
		return data.runs;
	},
	placeholderData: keepPrevious,
});

export const variantsPrefix = (slug: string, ref: string) => [
	"console",
	"benchmarks",
	slug,
	"variants",
	ref,
];

// The API's largest page.
export const VARIANTS_PAGE = 255;

export const variantsPage =
	(
		api: Api,
		slug: string,
		ref: string,
		archived: boolean,
	): FetchPage<JsonVariant> =>
	async (page, perPage, signal) => {
		const { data, headers } = await api.get<JsonVariant[]>(
			`${project(slug)}/benchmarks/${encodeURIComponent(ref)}/variants?archived=${archived}&per_page=${perPage}&page=${page}`,
			signal,
		);
		return { rows: data, total: totalOf(headers, data) };
	};

/** The first page of a benchmark's variants with one archived state. */
export const variantsQuery = (
	api: Api,
	slug: string,
	ref: string,
	archived: boolean,
) =>
	pageQuery(
		[...variantsPrefix(slug, ref), archived],
		VARIANTS_PAGE,
		1,
		variantsPage(api, slug, ref, archived),
	);

/**
 * Show a variant archived at `archived`, or active, in its benchmark's lists.
 * The totals move only when that changes the variant's shown state.
 */
export const markVariantArchived = (
	client: QueryClient,
	slug: string,
	ref: string,
	uuid: string,
	archived: string | undefined,
) => {
	const prefix = variantsPrefix(slug, ref);
	const pages = client.getQueriesData<Page<JsonVariant>>({ queryKey: prefix });
	const before = Boolean(
		pages
			.flatMap(([, page]) => page?.rows ?? [])
			.find((variant) => variant.uuid === uuid)?.archived,
	);
	const after = archived !== undefined;
	for (const [key, page] of pages) {
		if (page) {
			const holdsArchived = key[prefix.length] === true;
			const moved = before === after ? 0 : after === holdsArchived ? 1 : -1;
			client.setQueryData<Page<JsonVariant>>(key, {
				...page,
				rows: page.rows.map((variant) =>
					variant.uuid === uuid ? withArchived(variant, archived) : variant,
				),
				total: page.total + moved,
			});
		}
	}
};
