import { useParams } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import Truncate from "@bencherdev/ui/Truncate";
import {
	type QueryClient,
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import {
	For,
	Match,
	Show,
	Suspense,
	Switch,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	lazy,
	mapArray,
	on,
} from "solid-js";
import type {
	JsonConsoleAlerts,
	JsonConsoleRun,
	JsonConsoleRunLink,
	JsonConsoleRunReport,
} from "../../types/bencher";
import { type Api, ApiError } from "../api";
import Dot from "../dot";
import { projectPath, runPath } from "../paths";
import { formatWhen } from "../plot/format";
import { formatCount } from "../plot/count";
import { useNarrow } from "../plot/narrow";
import { leafOf } from "../leaf";
import { useEntityTitle, useProject } from "../project";
import { useQueryResult } from "../query";
import { absoluteTime, duration } from "../reports/format";
import { type Range, hasMore, loadMore } from "../reports/rows";
import { screenBatch } from "../reports/virtual";
import CopyHash from "../runs/CopyHash";
import { allNames, runHead } from "../runs/format";
import Neighbor from "../runs/Neighbor";
import { isRunId, runBatchQuery } from "../runs/run";
import RunReports from "../runs/RunReports";
import StatusPill from "../runs/StatusPill";
import { landed, runStatus } from "../runs/status";

/** Only a run that raised alerts draws them, so the code that draws a report's lines loads with its first alert. */
const RunAlerts = lazy(() => import("../runs/RunAlerts"));

/** Skeleton rows for a run never fetched, which show only once the wait is noticeable. */
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<JsonConsoleRun>;

/** A run: its facts, then its reports in start order, each opening its own page. */
const Run = () => {
	const { api, slug } = useProject();
	const params = useParams<{ run: string }>();
	const client = useQueryClient();
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
	useEntityTitle("Run", () => {
		const shown = run();
		return shown && `${leafOf(shown.branch.name)} ${runHead(shown)}`;
	});
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

	/** The testbeds of the reports loaded so far, each once. */
	const testbeds = createMemo(() => [
		...new Set(reports().map((report) => report.testbed.name)),
	]);

	const [now, setNow] = createSignal(Date.now());
	// Every answer, a poll's too, moves the times the page counts from now.
	createComputed(
		on(
			() => batches().map((batch) => batch.dataUpdatedAt),
			() => setNow(Date.now()),
		),
	);

	// The alerting lines on screen: once the table shows some, lines that land later wait for the reader.
	const alertKeys = createMemo(() =>
		(run()?.alerting?.groups ?? []).flatMap((group) =>
			group.alerts.flatMap((entry) => entry.line.alert?.uuid ?? []),
		),
	);
	const [shown, setShown] = createSignal<ReadonlySet<string>>(new Set());
	createComputed(
		on([() => params.run, alertKeys], ([current, keys], previous) => {
			if (previous?.[0] !== current || shown().size === 0) {
				setShown(new Set(keys));
			}
		}),
	);
	const held = () => alertKeys().filter((key) => !shown().has(key)).length;

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
	/** Where a step to the run on one side leads, once the run is known. */
	const step = (side: "previous" | "next") => {
		const current = run();
		const link = current?.[side];
		return current && link
			? {
					href: runPath(slug(), link.uuid),
					shown: formatWhen(link.start_time),
					detail: `the run on ${current.branch.name}, ${runHead(link)}`,
				}
			: undefined;
	};
	const intendRun = (link: JsonConsoleRunLink | undefined) => {
		if (link) {
			prefetch(client, api, slug(), link.uuid);
		}
	};
	// The report page's first batch comes with its code, which the move needs anyway.
	const intendReport = (uuid: string) =>
		import("./Report")
			.then(({ prefetch }) => prefetch(client, api, slug(), uuid))
			.catch(() => {});

	return (
		<Show when={!missing()} fallback={<NotFound slug={slug()} />}>
			<main class="page">
				<div class="pagehead rp-head">
					<div class="ph-title">
						<Crumb slug={slug()} />
						<Title run={run()} now={now()} />
					</div>
					<div class="ph-actions rp-actions">
						<Neighbor
							side="previous"
							link={step("previous")}
							onIntent={() => intendRun(run()?.previous)}
						/>
						<Neighbor
							side="next"
							link={step("next")}
							onIntent={() => intendRun(run()?.next)}
						/>
					</div>
					<p class="ph-sub rp-sub ui-parts">
						<Show
							when={run()}
							fallback={<Skeleton size="text" class="rp-sub-skel" />}
						>
							{(run) => (
								<For each={subtitle(run(), testbeds(), narrow(), now())}>
									{(part, index) => (
										<>
											<Show when={index() > 0}>
												<Dot />
											</Show>
											{part.reveal ? (
												<Truncate text={part.text} reveal />
											) : (
												<span data-fixed>{part.text}</span>
											)}
										</>
									)}
								</For>
							)}
						</Show>
					</p>
				</div>
				<section class="dm-section" aria-label="Reports">
					<div class="dm-section-head">
						<h2 class="seclabel">Reports</h2>
						<Show when={run()}>
							{(run) => <span class="count">{formatCount(run().total)}</span>}
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
										now={now()}
										onRange={setRange}
										onIntent={intendReport}
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
				<Show when={run()}>
					{(run) => (
						<Show
							when={
								(run().alerting?.total ?? 0) > 0 ? run().alerting : undefined
							}
						>
							{(alerting) => (
								<section class="dm-section" aria-label="Alerts in this run">
									<div class="dm-section-head">
										<h2 class="seclabel">Alerts in this run</h2>
										<span class="count">{alertCount(alerting())}</span>
										<Show when={held()}>
											{(count) => (
												<span class="rn-held" role="status">
													<span>
														<b>{formatCount(count())}</b> new{" "}
														{count() === 1 ? "line" : "lines"}
													</span>
													<Button
														size="sm"
														onClick={() => setShown(new Set(alertKeys()))}
													>
														Show
													</Button>
												</span>
											)}
										</Show>
									</div>
									<Suspense>
										<RunAlerts
											slug={slug()}
											label={`Alerts in run ${runHead(run())}, by report`}
											alerting={alerting()}
											shown={shown()}
											onReportIntent={intendReport}
										/>
									</Suspense>
								</section>
							)}
						</Show>
					)}
				</Show>
			</main>
		</Show>
	);
};

export default Run;

/** Start a run's first batch, at the size its page asks for, and the code of any alerts it raised, before the reader opens it. */
export const prefetch = (
	client: QueryClient,
	api: Api,
	slug: string,
	version: string,
) =>
	client
		.query(
			runBatchQuery(api, slug, version, { page: 1, perPage: screenBatch() }),
		)
		.then((run) => {
			if ((run.alerting?.total ?? 0) > 0) {
				RunAlerts.preload();
			}
		})
		.catch(() => {});

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
const Title = (props: { run: JsonConsoleRun | undefined; now: number }) => (
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
					<span class="ui-parts">
						<Truncate text={run().branch.name} tail reveal />
						<Dot />
						<span class="rn-titlehash" data-fixed>
							{runHead(run())}
						</span>
					</span>
				)}
			</Show>
		</Heading>
		<Show when={props.run?.hash}>{(hash) => <CopyHash hash={hash()} />}</Show>
		<Show when={(props.run?.alerts.total ?? 0) > 0}>
			<span class="rp-alerts">
				<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
					<circle cx="4" cy="4" r="4" />
				</svg>
				{plural(props.run?.alerts.total ?? 0, "alert")}
			</span>
		</Show>
		<Show
			when={
				props.run && runStatus(props.run, props.now, duration).kind
					? props.run
					: undefined
			}
		>
			{(run) => <StatusPill status={runStatus(run(), props.now, duration)} />}
		</Show>
	</div>
);

/** When, which version, its counts so far, its testbeds, and how long it took once its jobs are done; only the testbeds give way, to a reveal. */
const subtitle = (
	run: JsonConsoleRun,
	testbeds: readonly string[],
	narrow: boolean,
	now: number,
) => {
	// The heading's status carries a run's time until its jobs are done.
	const took = runStatus(run, now, duration).kind
		? undefined
		: `took ${duration(run.end_time - run.start_time)}`;
	const lines = landed(run.total, run.jobs)
		? plural(run.lines, "line")
		: undefined;
	const named = {
		text:
			testbeds.length > 0
				? `${plural(run.testbeds, "testbed")}: ${allNames(testbeds, run.testbeds)}`
				: plural(run.testbeds, "testbed"),
		reveal: true,
	};
	const fixed = (text: string | undefined) =>
		text === undefined ? [] : [{ text, reveal: false }];
	return narrow
		? [
				...fixed(formatWhen(run.start_time)),
				...fixed(`v${run.number}`),
				...fixed(plural(run.total, "report")),
				...fixed(lines),
				...fixed(took),
				named,
			]
		: [
				...fixed(absoluteTime(run.start_time)),
				...fixed(`version ${run.number}`),
				...fixed(plural(run.total, "report")),
				named,
				...fixed(lines),
				...fixed(took),
			];
};

const plural = (count: number, noun: string) =>
	`${formatCount(count)} ${count === 1 ? noun : `${noun}s`}`;

/** How many alerts the run raised, and how many the section draws when that is fewer. */
const alertCount = (alerting: JsonConsoleAlerts) => {
	const drawn = alerting.groups.reduce(
		(sum, group) => sum + group.alerts.length,
		0,
	);
	return drawn < alerting.total
		? `${formatCount(drawn)} of ${formatCount(alerting.total)}`
		: formatCount(alerting.total);
};

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
