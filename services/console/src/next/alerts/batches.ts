import type {
	DefaultError,
	QueryKey,
	QueryObserverOptions,
	QueryObserverResult,
} from "@tanstack/solid-query";
import {
	type Accessor,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import type { JsonConsoleAlerts } from "../../types/bencher";
import { useQueryResult } from "../query";

/** Where a batch starts, and how many of the view's batches started there before it, since alerts that left the list ahead of it, or a change since its offset was counted, make an offset read again. */
export interface Start {
	offset: number;
	round: number;
}

/** The first batch of every view, one object so its query is kept, and re-keyed, rather than started again. */
export const FIRST: Start = { offset: 0, round: 0 };

/**
 * A list of alerts a batch at a time, each its own query, for a page whose rows
 * change status in place: the next batch starts after the loaded rows still in
 * the view, once no change is in flight, and one a change overtook is asked for
 * again from there once that change lands. Later batches never read again on
 * their own, since a change ahead of one moves its offset; a first batch read
 * again that pushes rows still in view past its end, short of the next batch,
 * has every later batch asked for again.
 */
export const useAlertBatches = <T>(list: {
	/** The view the batches belong to; a new one starts again from its first batch. */
	view: Accessor<string>;
	query: (
		start: Start,
	) => QueryObserverOptions<T, DefaultError, T, T, QueryKey>;
	alerts: (batch: T) => JsonConsoleAlerts;
	perPage: number;
	/** The loaded rows still in the view on the server. */
	inView: () => number;
	/** The keys of a batch's rows still in the view on the server. */
	keysInView: (batch: T) => readonly string[];
	/** The changes still out. */
	inFlight: Accessor<number>;
}) => {
	const view = createMemo(list.view);
	// Later batches belong to the view that asked for them, so a new view drops them before they could ask again.
	const [more, setMore] = createSignal({ view: "", starts: [FIRST] });
	/** The view's later batches the API read after a change since their offset was counted, or a reread set aside, so they may start late. */
	const [overtaken, setOvertaken] = createSignal<ReadonlySet<Start>>(new Set());
	createComputed(
		on(
			view,
			(key) => {
				setMore({ view: key, starts: [FIRST] });
				setOvertaken(new Set<Start>());
			},
			{ defer: true },
		),
	);
	const starts = () => (more().view === view() ? more().starts : [FIRST]);
	let sent = 0;
	createEffect(
		on(list.inFlight, (count, previous = 0) => {
			sent += Math.max(0, count - previous);
		}),
	);
	/** The changes sent when each later batch's offset was counted. */
	const counted = new WeakMap<Start, number>();
	const read = (start: Start) => {
		const options = list.query(start);
		const ask = options.queryFn;
		if (start === FIRST || typeof ask !== "function") {
			return options;
		}
		const queryFn: typeof ask = async (context) => {
			const batch = await ask(context);
			if (sent !== counted.get(start)) {
				setOvertaken(new Set([...overtaken(), start]));
			}
			return batch;
		};
		return {
			...options,
			queryFn,
			refetchOnWindowFocus: false,
			refetchOnReconnect: false,
		};
	};
	const queries = createMemo(
		mapArray(starts, (start) => useQueryResult(() => read(start))),
	);
	const batches = () => queries().map((batch) => batch());
	/** The batches loaded so far, up to the first one still loading, failed, or overtaken. */
	const loaded = createMemo(() => {
		const ready: QueryObserverResult<T>[] = [];
		const from = starts();
		for (const [index, batch] of batches().entries()) {
			const start = from[index];
			if (
				batch.data === undefined ||
				(start !== undefined && overtaken().has(start))
			) {
				break;
			}
			ready.push(batch);
		}
		return ready;
	});
	/** The batch after the loaded ones, while it loads or once it failed. */
	const next = () => batches()[loaded().length];
	/** The batch after the loaded ones was overtaken, so it is asked for again. */
	const again = () => {
		const start = starts()[loaded().length];
		return start !== undefined && overtaken().has(start);
	};
	const setAside = () =>
		setOvertaken(new Set([...overtaken(), ...starts().slice(1)]));
	// Alerts raised ahead push the first batch's last rows into no batch.
	createComputed(
		on(
			() => queries()[0]?.().data,
			(fresh, old) => {
				if (
					fresh === old ||
					fresh === undefined ||
					old === undefined ||
					starts().length < 2
				) {
					return;
				}
				const held = new Set(list.keysInView(fresh));
				if (list.keysInView(old).every((key) => held.has(key))) {
					return;
				}
				const after = loaded()[1]?.data;
				if (
					after === undefined ||
					!list.keysInView(after).some((key) => held.has(key))
				) {
					setAside();
				}
			},
			{ defer: true },
		),
	);

	const [wantMore, setWantMore] = createSignal(false);
	const nearEnd = () => {
		const last = loaded().at(-1)?.data;
		if (last === undefined || (next() !== undefined && !again())) {
			return;
		}
		if (list.inFlight() > 0) {
			setWantMore(true);
			return;
		}
		const { groups, total } = list.alerts(last);
		const offset = list.inView();
		const full =
			groups.reduce((sum, group) => sum + group.alerts.length, 0) ===
			list.perPage;
		const kept = starts().slice(0, loaded().length);
		if (full && offset < total) {
			// An overtaken batch keeps its key, so a batch from its offset counts it.
			const round = [...new Set([...starts(), ...overtaken()])].filter(
				(start) => start.offset === offset,
			).length;
			const start = { offset, round };
			counted.set(start, sent);
			setMore({ view: view(), starts: [...kept, start] });
		} else if (kept.length < starts().length) {
			setMore({ view: view(), starts: kept });
		}
	};
	createEffect(
		on(list.inFlight, (count) => {
			if (count === 0 && wantMore()) {
				setWantMore(false);
				nearEnd();
			}
		}),
	);
	createEffect(
		on(again, (overtook) => {
			if (overtook) {
				nearEnd();
			}
		}),
	);

	return {
		batches,
		loaded,
		next,
		/** The view's first batch, once it answered. */
		first: () => loaded()[0]?.data,
		/** The rows on screen are the last view's, until the new one answers. */
		busy: () => batches()[0]?.isPlaceholderData === true,
		/** The batch after the loaded ones is on its way, or waits to be asked for again. */
		loading: () => next()?.isFetching === true || again(),
		/** The rows drawn reach the last one loaded: ask for the next batch. */
		nearEnd,
		/** Read the view again from its first batch; once it answers, each later batch is asked for again from the offset the batches before it count. */
		reread: async () => {
			const first = await batches()[0]?.refetch();
			if (first?.isError !== false) {
				return;
			}
			setAside();
		},
	};
};
