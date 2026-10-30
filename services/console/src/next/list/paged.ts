import { useQueryClient } from "@tanstack/solid-query";
import {
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import { useQueryResult } from "../query";
import { type FetchPage, type Page, pageQuery } from "./pages";
import { type Range, loadMore } from "./rows";

const keep = <T>(previous: T | undefined) => previous;

/**
 * A list read a page at a time, its count and rows from the same answers so
 * they never disagree. Each page is its own query under `queryKey`, and a
 * new key starts over at the first page. `drain` reads every page at once.
 */
export const createPaged = <T extends { uuid: string }>(props: {
	queryKey: () => readonly unknown[];
	perPage: number;
	fetchPage: FetchPage<T>;
	enabled?: () => boolean;
	/** A new key keeps the rows shown until its first page answers. */
	keep?: boolean;
	drain?: boolean;
}) => {
	const client = useQueryClient();
	const [count, setCount] = createSignal(1);
	const key = createMemo(() => JSON.stringify(props.queryKey()));
	createComputed(on(key, () => setCount(1), { defer: true }));
	const read = (page: number) =>
		useQueryResult(() => ({
			...pageQuery(props.queryKey(), props.perPage, page, props.fetchPage),
			enabled: props.enabled?.() ?? true,
			...(props.keep && page === 1 ? { placeholderData: keep } : {}),
		}));
	const first = read(1);
	const later = createMemo(
		mapArray(
			() => Array.from({ length: count() - 1 }, (_, index) => index + 2),
			read,
		),
	);
	const pages = () => [first(), ...later().map((page) => page())];
	/** The pages answered so far, up to the first one still loading or failed. */
	const loaded = createMemo(() => {
		const ready: Page<T>[] = [];
		for (const page of pages()) {
			if (page.data === undefined) {
				break;
			}
			ready.push(page.data);
		}
		return ready;
	});
	const next = () => pages()[loaded().length];
	// A row that arrives between pages shifts the later ones by one.
	const rows = createMemo(() => {
		const seen = new Set<string>();
		const all: T[] = [];
		for (const row of loaded().flatMap((page) => page.rows)) {
			if (!seen.has(row.uuid)) {
				seen.add(row.uuid);
				all.push(row);
			}
		}
		return all;
	});
	const total = () => loaded()[0]?.total;
	const load = () => {
		const last = loaded().at(-1);
		// A row put in at once can fill a page past its size, which still means more follow.
		if (
			last &&
			next() === undefined &&
			rows().length < last.total &&
			last.rows.length >= props.perPage
		) {
			setCount(pages().length + 1);
		}
	};
	// A change cancels the reads in flight, which would land over it; a next page cancelled that way asks again.
	createEffect(() => {
		const asked = next();
		if (
			asked?.isPending &&
			!asked.isFetching &&
			!asked.isError &&
			props.enabled?.() !== false
		) {
			asked.refetch();
		}
	});
	if (props.drain) {
		createEffect(on(loaded, load));
	}
	const more = () => Math.max(0, (total() ?? 0) - rows().length);
	return {
		rows,
		total,
		/** The rows the list holds past those loaded. */
		more,
		/** The first page's read, for its first load and its error. */
		first,
		/** The page after the loaded ones is on its way. */
		loading: () => next()?.isFetching ?? false,
		/** The page after the loaded ones failed: asks for it again. */
		retry: () =>
			next()?.isLoadingError ? () => void next()?.refetch() : undefined,
		load,
		/** A row the API no longer lists moved the later pages up by one: read the loaded ones again. */
		refill: () => {
			if (more() > 0) {
				client.invalidateQueries({ queryKey: props.queryKey() });
			}
		},
		/** The rows on screen changed: the next page loads as they near the last loaded. */
		onRange: (range: Range) => {
			if (
				loadMore({
					end: range.end,
					loaded: rows().length,
					batch: props.perPage,
				})
			) {
				load();
			}
		},
	};
};

export type Paged<T extends { uuid: string }> = ReturnType<
	typeof createPaged<T>
>;
