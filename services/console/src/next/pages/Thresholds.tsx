import { useLocation, useNavigate } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import type { QueryObserverResult } from "@tanstack/solid-query";
import {
	type Accessor,
	For,
	Match,
	Show,
	Switch,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import type { JsonConsoleThresholds } from "../../types/bencher";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { type Range, loadMore } from "../reports/rows";
import Controls, { type FilterNames } from "../thresholds/Controls";
import {
	type Dimension,
	nameQuery,
	screenBatch,
	thresholdsQuery,
} from "../thresholds/query";
import { type ThresholdRow, rowsOf } from "../thresholds/rows";
import {
	DEFAULT_SEARCH,
	type ThresholdsSearch,
	decodeSearch,
	encodeSearch,
	filterCount,
	windowLabel,
	windowPhrase,
} from "../thresholds/search";
import ThresholdsTable from "../thresholds/ThresholdsTable";

/** Skeleton rows for a list never fetched, which show only once the wait is noticeable. */
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<JsonConsoleThresholds>;

/** A project's thresholds, one row each, with the alerts each raised in a window. */
const Thresholds = () => {
	const { api, slug } = useProject();
	const location = useLocation();
	const navigate = useNavigate();
	const search = createMemo(() =>
		decodeSearch(new URLSearchParams(location.search)),
	);
	const setSearch = (next: ThresholdsSearch) => {
		const query = encodeSearch(next);
		// Router paths are relative to the console's base.
		navigate(`/${slug()}/thresholds${query ? `?${query}` : ""}`, {
			scroll: false,
		});
	};
	const names = useNames(search);

	const perPage = screenBatch();
	const [count, setCount] = createSignal(1);
	createComputed(on(search, () => setCount(1), { defer: true }));
	const queries = createMemo(
		mapArray(
			() => Array.from({ length: count() }, (_, index) => index + 1),
			(page) =>
				useQueryResult(() =>
					thresholdsQuery(api, slug(), search(), { page, perPage }),
				),
		),
	);
	const batches = () => queries().map((batch) => batch());
	/** The batches loaded so far, up to the first one still loading or failed. */
	const loaded = createMemo(() => {
		const ready: Batch[] = [];
		for (const batch of batches()) {
			if (batch.data === undefined) {
				break;
			}
			ready.push(batch);
		}
		return ready;
	});
	/** The batch after the loaded ones, while it loads or once it failed. */
	const next = () => batches()[loaded().length];
	const rows = createMemo(() =>
		loaded().flatMap((batch) => (batch.data ? rowsOfBatch(batch.data) : [])),
	);
	const first = () => loaded()[0]?.data;
	/** The rows on screen are the last search's, until the new one answers. */
	const busy = () => batches()[0]?.isPlaceholderData === true;
	const total = () => first()?.total;
	const [now, setNow] = createSignal(Date.now());
	createComputed(
		on(
			() => batches()[0]?.dataUpdatedAt,
			() => setNow(Date.now()),
		),
	);

	const [range, setRange] = createSignal<Range>();
	createEffect(() => {
		const shown = range();
		const last = loaded().at(-1)?.data;
		if (
			shown &&
			last &&
			next() === undefined &&
			rows().length < last.total &&
			last.thresholds.length === perPage &&
			loadMore({ end: shown.end, loaded: rows().length, batch: perPage })
		) {
			setCount(batches().length + 1);
		}
	});

	const noun = () => (search().archived ? "archived threshold" : "threshold");

	return (
		<main class="page">
			<div class="pagehead">
				<div class="crumb-line">Thresholds</div>
				<Heading level={1} size="xl" class="th-count">
					<Show
						when={total() !== undefined}
						fallback={
							<>
								<Skeleton size="text" class="th-count-skel" />
								<span class="sr-only">Thresholds</span>
							</>
						}
					>
						{total() === 1 ? `1 ${noun()}` : `${total()} ${noun()}s`}
					</Show>
				</Heading>
				<p class="ph-sub">
					Runs declare thresholds, and every threshold that matches a line
					checks it.
				</p>
			</div>
			<Controls
				search={search()}
				names={names()}
				now={now()}
				onSearch={setSearch}
			/>
			<Switch>
				<Match when={first() && rows().length === 0 && !busy()}>
					<NothingFound search={search()} onSearch={setSearch} />
				</Match>
				<Match when={rows().length > 0}>
					<ThresholdsTable
						slug={slug()}
						rows={rows()}
						total={total() ?? rows().length}
						archived={search().archived}
						window={windowPhrase(search().window, now())}
						windowLabel={
							search().window.kind === "custom"
								? "range"
								: windowLabel(search().window)
						}
						onRange={setRange}
						busy={busy()}
						loading={next()?.isFetching ?? false}
						onRetry={
							next()?.isLoadingError ? () => next()?.refetch() : undefined
						}
					/>
				</Match>
				<Match when={batches()[0]?.isLoadingError}>
					<Banner status="error" role="alert" class="load-error">
						<span class="grow">
							Thresholds did not load: the Bencher API did not answer.
						</span>
						<Button size="sm" onClick={() => batches()[0]?.refetch()}>
							Retry
						</Button>
					</Banner>
				</Match>
				<Match when={true}>
					<div class="ui-table-scroll th-skeleton" aria-busy="true">
						<span class="sr-only">Loading thresholds</span>
						<For each={Array.from({ length: SKELETON_ROWS })}>
							{() => <Skeleton size="text" class="th-skeleton-row" />}
						</For>
					</div>
				</Match>
			</Switch>
		</main>
	);
};

export default Thresholds;

// Each batch resolves its rows once, so the rows on screen keep their objects as data arrives.
const resolved = new WeakMap<JsonConsoleThresholds, ThresholdRow[]>();
const rowsOfBatch = (batch: JsonConsoleThresholds) => {
	let rows = resolved.get(batch);
	if (!rows) {
		rows = rowsOf(batch);
		resolved.set(batch, rows);
	}
	return rows;
};

/** The names of the branch, the testbed, and the measure the search filters by. */
const useNames = (
	search: Accessor<ThresholdsSearch>,
): Accessor<FilterNames> => {
	const branch = useName("branches", () => search().branch);
	const testbed = useName("testbeds", () => search().testbed);
	const measure = useName("measures", () => search().measure);
	return () => ({ branch: branch(), testbed: testbed(), measure: measure() });
};

const useName = (resource: Dimension, uuid: Accessor<string | undefined>) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => nameQuery(api, slug(), resource, uuid()));
	return () => result().data;
};

const NothingFound = (props: {
	search: ThresholdsSearch;
	onSearch: (search: ThresholdsSearch) => void;
}) => {
	const filtered = () => filterCount(props.search) > 0;
	return (
		<div class="ui-table-scroll th-empty">
			<Heading level={2} size="lg">
				{filtered()
					? "No thresholds match these filters"
					: props.search.archived
						? "No archived thresholds"
						: "No thresholds yet"}
			</Heading>
			<p>
				{filtered()
					? "Clear the filters to see every threshold."
					: props.search.archived
						? "A threshold is archived with its branch, testbed, or measure."
						: "The first run that declares a threshold adds it here."}
			</p>
			<Show when={filtered()}>
				<Button
					onClick={() =>
						props.onSearch({
							...DEFAULT_SEARCH,
							archived: props.search.archived,
							window: props.search.window,
						})
					}
				>
					Clear filters
				</Button>
			</Show>
		</div>
	);
};
