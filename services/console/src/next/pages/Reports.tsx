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
import type { JsonReport } from "../../types/bencher";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import Controls from "../reports/Controls";
import {
	type ReportsBatch,
	batchQuery,
	nameKey,
	screenBatch,
} from "../reports/query";

export { prefetchReports as prefetch } from "../reports/query";
import ReportsTable from "../reports/ReportsTable";
import { type Range, hasMore, loadMore } from "../reports/rows";
import {
	DEFAULT_SEARCH,
	type FilterNames,
	type ReportsSearch,
	decodeSearch,
	encodeSearch,
	filterCount,
	nothingFound,
	windowPhrase,
} from "../reports/search";

/** Skeleton rows for a list never fetched, which show only once the wait is noticeable. */
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<ReportsBatch>;

const Reports = () => {
	const { api, slug } = useProject();
	const location = useLocation();
	const navigate = useNavigate();
	const search = createMemo(() =>
		decodeSearch(new URLSearchParams(location.search)),
	);
	const setSearch = (next: ReportsSearch) => {
		const query = encodeSearch(next);
		// Router paths are relative to the console's base.
		navigate(`/${slug()}/reports${query ? `?${query}` : ""}`, {
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
					batchQuery(api, slug(), search(), { page, perPage }),
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

	const reports = createMemo(() => {
		const seen = new Set<string>();
		const all: JsonReport[] = [];
		// A report that arrives between batches shifts them by one.
		for (const batch of loaded()) {
			for (const report of batch.data?.reports ?? []) {
				if (!seen.has(report.uuid)) {
					seen.add(report.uuid);
					all.push(report);
				}
			}
		}
		return all;
	});
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
			hasMore(last, reports().length) &&
			loadMore({ end: shown.end, loaded: reports().length, batch: perPage })
		) {
			setCount(batches().length + 1);
		}
	});

	return (
		<main class="page reports">
			<div class="pagehead">
				<div class="crumb-line">Reports</div>
				<Heading level={1} size="xl" class="reports-count">
					<Show
						when={total() !== undefined}
						fallback={
							<>
								<Skeleton size="text" class="reports-count-skel" />
								<span class="sr-only">Reports</span>
							</>
						}
					>
						{total() === 1 ? "1 report" : `${total()} reports`}
					</Show>
				</Heading>
				<p class="ph-sub">
					{windowPhrase(search().window, now())}
					<span class="dotsep" aria-hidden="true">
						·
					</span>
					newest first
				</p>
			</div>
			<Controls
				search={search()}
				names={names()}
				onSearch={setSearch}
				now={now()}
			/>
			<Switch>
				<Match when={first() && reports().length === 0 && !busy()}>
					<NothingFound
						search={search()}
						names={names()}
						now={now()}
						onSearch={setSearch}
					/>
				</Match>
				<Match when={reports().length > 0}>
					<ReportsTable
						slug={slug()}
						reports={reports()}
						total={total() ?? reports().length}
						now={now()}
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
							Reports did not load: the Bencher API did not answer.
						</span>
						<Button size="sm" onClick={() => batches()[0]?.refetch()}>
							Retry
						</Button>
					</Banner>
				</Match>
				<Match when={true}>
					<div class="ui-table-scroll reports-skeleton" aria-busy="true">
						<span class="sr-only">Loading reports</span>
						<For each={Array.from({ length: SKELETON_ROWS })}>
							{() => <Skeleton size="text" class="reports-skeleton-row" />}
						</For>
					</div>
				</Match>
			</Switch>
		</main>
	);
};

/** The names of the branch and the testbed the search filters by, by slug. */
const useNames = (search: Accessor<ReportsSearch>): Accessor<FilterNames> => {
	const branch = useName("branches", () => search().branch);
	const testbed = useName("testbeds", () => search().testbed);
	return () => ({ branch: branch(), testbed: testbed() });
};

const useName = (
	resource: "branches" | "testbeds",
	value: Accessor<string | undefined>,
) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => ({
		queryKey: nameKey(resource, slug(), value()),
		queryFn: async ({ signal }: { signal: AbortSignal }) =>
			(
				await api.get<{ name: string }>(
					`/v0/projects/${encodeURIComponent(slug())}/${resource}/${encodeURIComponent(value() ?? "")}`,
					signal,
				)
			).data.name,
		enabled: value() !== undefined,
	}));
	return () => result().data;
};

export default Reports;

const NothingFound = (props: {
	search: ReportsSearch;
	names: FilterNames;
	now: number;
	onSearch: (search: ReportsSearch) => void;
}) => {
	const filtered = () => filterCount(props.search) > 0;
	const all = () => props.search.window.kind === "all";
	return (
		<div class="ui-table-scroll reports-empty">
			<Heading level={2} size="lg">
				{filtered()
					? "No reports match these filters"
					: all()
						? "No reports yet"
						: "No reports in this window"}
			</Heading>
			<p>{nothingFound(props.search, props.now, undefined, props.names)}</p>
			<div class="btnrow">
				<Show when={filtered()}>
					<Button
						onClick={() =>
							props.onSearch({
								window: props.search.window,
								alerts: DEFAULT_SEARCH.alerts,
							})
						}
					>
						Clear filters
					</Button>
				</Show>
				<Show when={!all()}>
					<Button
						onClick={() =>
							props.onSearch({ ...props.search, window: { kind: "all" } })
						}
					>
						Show all time
					</Button>
				</Show>
			</div>
		</div>
	);
};
