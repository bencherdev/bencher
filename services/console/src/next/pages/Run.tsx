import { useParams } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import type { QueryObserverResult } from "@tanstack/solid-query";
import {
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
import type { JsonConsoleRun, JsonConsoleRunReport } from "../../types/bencher";
import { ApiError } from "../api";
import { projectPath } from "../paths";
import { formatWhen } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { absoluteTime, duration } from "../reports/format";
import { type Range, hasMore, loadMore } from "../reports/rows";
import { screenBatch } from "../reports/virtual";
import { runHead } from "../runs/format";
import { isRunId, runBatchQuery } from "../runs/run";
import RunReports from "../runs/RunReports";

/** Skeleton rows for a run never fetched, which show only once the wait is noticeable. */
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<JsonConsoleRun>;

/** A run: its facts, then its reports in start order, each opening its own page. */
const Run = () => {
	const { api, slug } = useProject();
	const params = useParams<{ run: string }>();
	const narrow = useNarrow();

	const perPage = screenBatch();
	const [count, setCount] = createSignal(1);
	createComputed(
		on(
			() => params.run,
			() => setCount(1),
			{ defer: true },
		),
	);
	const queries = createMemo(
		mapArray(
			() => Array.from({ length: count() }, (_, index) => index + 1),
			(page) =>
				useQueryResult(() => ({
					...runBatchQuery(api, slug(), params.run, { page, perPage }),
					enabled: isRunId(params.run),
				})),
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
	const run = () => loaded()[0]?.data;
	const reports = createMemo(() => {
		const seen = new Set<string>();
		const all: JsonConsoleRunReport[] = [];
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

	const [range, setRange] = createSignal<Range>();
	createEffect(() => {
		const shown = range();
		const last = loaded().at(-1)?.data;
		if (
			shown &&
			last &&
			next() === undefined &&
			hasMore(
				{ rows: last.reports, total: last.total, batch: { perPage } },
				reports().length,
			) &&
			loadMore({ end: shown.end, loaded: reports().length, batch: perPage })
		) {
			setCount(batches().length + 1);
		}
	});

	// Data already seen keeps the page up; only a run never loaded shows a failure.
	const failure = () => {
		const error = batches()[0]?.error;
		return run() || !error
			? undefined
			: error instanceof ApiError
				? error.kind
				: "network";
	};
	const missing = () => !isRunId(params.run) || failure() === "not_found";

	return (
		<Show when={!missing()} fallback={<NotFound slug={slug()} />}>
			<main class="page">
				<div class="pagehead rp-head">
					<div class="ph-title">
						<Crumb slug={slug()} />
						<Title run={run()} />
					</div>
					<p class="ph-sub rp-sub">
						<Show
							when={run()}
							fallback={<Skeleton size="text" class="rp-sub-skel" />}
						>
							{(run) => subtitle(run(), narrow())}
						</Show>
					</p>
				</div>
				<section class="dm-section" aria-label="Reports">
					<div class="dm-section-head">
						<h2 class="seclabel">Reports</h2>
						<Show when={run()}>
							{(run) => <span class="count">{run().total}</span>}
						</Show>
					</div>
					<Switch>
						<Match when={run()}>
							{(run) => (
								<div class="ui-table-scroll">
									<RunReports
										slug={slug()}
										reports={reports()}
										total={run().total}
										onRange={setRange}
										loading={next()?.isFetching ?? false}
										onRetry={
											next()?.isLoadingError
												? () => next()?.refetch()
												: undefined
										}
									/>
								</div>
							)}
						</Match>
						<Match when={failure()}>
							<Banner status="error" role="alert" class="load-error">
								<span class="grow">This run did not load</span>
								<Button size="sm" onClick={() => batches()[0]?.refetch()}>
									Retry
								</Button>
							</Banner>
						</Match>
						<Match when={true}>
							<div class="ui-table-scroll reports-skeleton" aria-busy="true">
								<span class="sr-only">Loading the run's reports</span>
								<For each={Array.from({ length: SKELETON_ROWS })}>
									{() => <Skeleton size="text" class="reports-skeleton-row" />}
								</For>
							</div>
						</Match>
					</Switch>
				</section>
			</main>
		</Show>
	);
};

export default Run;

/** The way back to Runs. */
const Crumb = (props: { slug: string }) => (
	<div class="crumb-line">
		<a href={projectPath(props.slug, "runs")}>Runs</a>
		<span aria-hidden="true">/</span>
	</div>
);

/**
 * The branch and the run's head, and its alerts; before they arrive, a
 * placeholder of the same line box, so what arrives later moves nothing.
 */
const Title = (props: { run: JsonConsoleRun | undefined }) => (
	<div class="rp-titlerow">
		<Heading level={1} size="xl">
			<Show
				when={props.run}
				fallback={
					<>
						<Skeleton size="text" class="rp-title-skel" />
						<span class="sr-only">Loading the run</span>
					</>
				}
			>
				{(run) => (
					<>
						{run().branch.name} <span aria-hidden="true">·</span>{" "}
						<span class="rn-titlehash">{runHead(run())}</span>
					</>
				)}
			</Show>
		</Heading>
		<Show when={(props.run?.alerts.total ?? 0) > 0}>
			<span class="rp-alerts">
				<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
					<circle cx="4" cy="4" r="4" />
				</svg>
				{plural(props.run?.alerts.total ?? 0, "alert")}
			</span>
		</Show>
	</div>
);

/** When, which version, its counts, and how long it took. */
const subtitle = (run: JsonConsoleRun, narrow: boolean) => {
	const took = `took ${duration(run.end_time - run.start_time)}`;
	const lines = plural(run.lines, "line");
	if (narrow) {
		return [
			formatWhen(run.start_time),
			`v${run.number}`,
			plural(run.total, "report"),
			lines,
			took,
		].join(" · ");
	}
	return [
		absoluteTime(run.start_time),
		`version ${run.number}`,
		`${plural(run.total, "report")}, ${plural(run.testbeds, "testbed")}`,
		lines,
		took,
	].join(" · ");
};

const plural = (count: number, noun: string) =>
	`${count} ${count === 1 ? noun : `${noun}s`}`;

const NotFound = (props: { slug: string }) => (
	<main class="page">
		<div class="pagehead">
			<div class="ph-title">
				<Crumb slug={props.slug} />
				<Heading level={1} size="xl">
					Run not found
				</Heading>
			</div>
		</div>
	</main>
);
