import { useLocation } from "@solidjs/router";
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
import type { JsonConsoleRunSummary } from "../../types/bencher";
import ContentSkeleton from "../ContentSkeleton";
import { PROJECTS_PATH } from "../paths";
import { useProject, useProjectNavigate } from "../project";
import { useQueryResult } from "../query";
import Controls, { nameKey } from "../reports/Controls";
import { type Range, hasMore, loadMore } from "../reports/rows";
import { screenBatch } from "../reports/virtual";
import { type RunsBatch, batchQuery } from "../runs/query";

export { prefetchRuns as prefetch } from "../runs/query";
import RunsTable from "../runs/RunsTable";
import {
	type FilterNames,
	type ReportsSearch,
	decodeSearch,
	encodeSearch,
} from "../reports/search";

/** Skeleton rows for a list never fetched, which show only once the wait is noticeable. */
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<RunsBatch>;

const Runs = () => {
	const { api, slug } = useProject();
	const location = useLocation();
	const navigate = useProjectNavigate();
	const search = createMemo(() =>
		decodeSearch(new URLSearchParams(location.search)),
	);
	const setSearch = (next: ReportsSearch) => {
		const query = encodeSearch(next);
		// Router paths are relative to the console's base, and the project root is Runs too.
		navigate(
			`${location.pathname.slice(PROJECTS_PATH.length)}${query ? `?${query}` : ""}`,
			{ scroll: false },
		);
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

	const runs = createMemo(() => {
		const seen = new Set<string>();
		const all: JsonConsoleRunSummary[] = [];
		// A run that arrives between batches shifts them by one.
		for (const batch of loaded()) {
			for (const run of batch.data?.runs ?? []) {
				if (!seen.has(run.uuid)) {
					seen.add(run.uuid);
					all.push(run);
				}
			}
		}
		return all;
	});
	const first = () => loaded()[0]?.data;
	/** The rows on screen are the last search's, until the new one answers. */
	const busy = () => batches()[0]?.isPlaceholderData === true;
	const total = () => first()?.total;

	// A project that never ran gets the setup card, which polls the first batch
	// until a run lands; only the unfiltered list asks whether it ever ran.
	const quiet = () => !busy() && total() === 0 && encodeSearch(search()) === "";
	const ever = useQueryResult(() => ({
		...batchQuery(
			api,
			slug(),
			{ ...search(), window: { kind: "all" } },
			{ page: 1, perPage },
		),
		enabled: quiet(),
	}));
	const deciding = () =>
		quiet() && ever().data === undefined && ever().errorUpdatedAt === 0;
	const empty = () => quiet() && ever().data?.total === 0;
	const [Setup, setSetup] = createSignal<SetupView>();
	createEffect(() => {
		if (quiet() && !Setup()) {
			import("../runs/Setup")
				.then(({ default: view }) => setSetup(() => view))
				.catch(() => {});
		}
	});
	const [now, setNow] = createSignal(Date.now());
	// Every answer, a poll's too, moves the times the rows count from now.
	createComputed(
		on(
			() => batches().map((batch) => batch.dataUpdatedAt),
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
			hasMore(
				{ rows: last.runs, total: last.total, batch: last.batch },
				runs().length,
			) &&
			loadMore({ end: shown.end, loaded: runs().length, batch: perPage })
		) {
			setCount(batches().length + 1);
		}
	});

	return (
		<Show
			when={!empty()}
			fallback={
				<Show when={Setup()} fallback={<ContentSkeleton />}>
					{(View) => {
						const Loaded = View();
						return (
							<Loaded
								poll={() =>
									batchQuery(api, slug(), search(), { page: 1, perPage })
								}
							/>
						);
					}}
				</Show>
			}
		>
			<main class="page reports">
				<div class="pagehead">
					<div class="crumb-line">Runs</div>
					<Heading level={1} size="xl" class="reports-count">
						<Show
							when={total() !== undefined && !deciding()}
							fallback={
								<>
									<Skeleton size="text" class="reports-count-skel" />
									<span class="sr-only">Runs</span>
								</>
							}
						>
							{total() === 1 ? "1 run" : `${total()} runs`}
						</Show>
					</Heading>
				</div>
				<Controls
					search={search()}
					names={names()}
					onSearch={setSearch}
					now={now()}
				/>
				<Switch>
					<Match
						when={first() && runs().length === 0 && !busy() && !deciding()}
					>
						<NothingFound search={search()} onSearch={setSearch} />
					</Match>
					<Match when={runs().length > 0}>
						<RunsTable
							slug={slug()}
							runs={runs()}
							total={total() ?? runs().length}
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
							<span class="grow">Runs did not load</span>
							<Button size="sm" onClick={() => batches()[0]?.refetch()}>
								Retry
							</Button>
						</Banner>
					</Match>
					<Match when={true}>
						<div class="ui-table-scroll reports-skeleton" aria-busy="true">
							<span class="sr-only">Loading runs</span>
							<For each={Array.from({ length: SKELETON_ROWS })}>
								{() => <Skeleton size="text" class="reports-skeleton-row" />}
							</For>
						</div>
					</Match>
				</Switch>
			</main>
		</Show>
	);
};

type SetupView = typeof import("../runs/Setup").default;

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

export default Runs;

const NothingFound = (props: {
	search: ReportsSearch;
	onSearch: (search: ReportsSearch) => void;
}) => {
	const all = () => props.search.window.kind === "all";
	return (
		<div class="ui-table-scroll reports-empty">
			<Heading level={2} size="lg">
				No runs
			</Heading>
			<Show when={!all()}>
				<div class="btnrow">
					<Button
						onClick={() =>
							props.onSearch({ ...props.search, window: { kind: "all" } })
						}
					>
						Show all time
					</Button>
				</div>
			</Show>
		</div>
	);
};
