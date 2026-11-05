import { useLocation, useParams } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import {
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
	createMemo,
	createSignal,
	lazy,
	mapArray,
	on,
} from "solid-js";
import type { JsonConsoleReport, JsonConsoleSpec } from "../../types/bencher";
import type { Settable } from "../alerts/status";
import { useFold } from "../alerts/fold";
import { ApiError } from "../api";
import { alertPath, projectPath, reportPath, runPath } from "../paths";
import { useNarrow } from "../plot/narrow";
import { leafOf } from "../leaf";
import { useEntityTitle, useProject, useProjectNavigate } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import Controls from "../report/Controls";
import LineTable from "../report/LineTable";
import {
	type GroupRow,
	type ReportLine,
	groupsOf,
	lineAt,
	linesOf,
	plotOf,
} from "../report/lines";
import {
	type ReportBatch,
	batchQuery,
	isReportId,
	prefetchReport,
	screenBatch,
} from "../report/query";

export { prefetchLinkedReport as prefetch } from "../report/query";
import { slotsOf } from "../report/slots";
import { declaresThresholdV0 } from "../report/snippetV0";
import ThresholdSheet from "../report/ThresholdSheet";
import {
	type ReportView,
	parseReportView,
	reportViewSearch,
	windowDays,
} from "../report/view";
import { duration, shortHash } from "../reports/format";
import Neighbor, { onMoved } from "../runs/Neighbor";
import { reportStatus } from "../runs/status";
import StatusPill from "../runs/StatusPill";
import { formatCount } from "../plot/count";

const SKELETON_ROWS = 8;

/** A report as one row per line, each with its history over a window ending at the report. */
const Report = () => {
	const { api, slug, host } = useProject();
	const params = useParams<{ report: string }>();
	const location = useLocation();
	const navigate = useProjectNavigate();
	const client = useQueryClient();
	const narrow = useNarrow();
	const fold = useFold();
	const view = createMemo(() =>
		parseReportView(new URLSearchParams(location.search)),
	);
	const query = (next: ReportView) => {
		const search = reportViewSearch(next);
		return search ? `?${search}` : "";
	};
	const setView = (next: ReportView, replace = false) =>
		// Router paths are relative to the console's base.
		navigate(`/${slug()}/reports/${params.report}${query(next)}`, {
			scroll: false,
			replace,
		});
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	// The statuses its row plots set, which outlive a row folded and opened again.
	const kept = new Map<string, Settable>();
	const canChange = () => bootstrap().data?.permissions.edit === true;
	const version = () => bootstrap().data?.project.bmf_version;

	const perPage = screenBatch();
	const linesKey = createMemo(
		() =>
			`${params.report}${query({ ...view(), expanded: [], iteration: undefined })}`,
	);
	// Later batches belong to the view that asked for them, so a new view drops them before they could ask again.
	const [more, setMore] = createSignal({ key: "", count: 1 });
	createComputed(
		on(linesKey, (key) => setMore({ key, count: 1 }), { defer: true }),
	);
	const queries = createMemo(
		mapArray(
			() =>
				Array.from(
					{ length: more().key === linesKey() ? more().count : 1 },
					(_, index) => index + 1,
				),
			(page) =>
				useQueryResult(() => ({
					...batchQuery(api, slug(), params.report, view(), {
						page,
						perPage,
					}),
					enabled: isReportId(params.report),
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
	const first = () => loaded()[0]?.data;
	const report = () => first()?.report;
	useEntityTitle("Report", () => {
		const shown = report();
		return (
			shown &&
			`${leafOf(shown.branch.name)} ${shortHash(shown.version.hash) ?? `v${shown.version.number}`}`
		);
	});
	/** The rows on screen are the last view's, until the new one answers. */
	const busy = () => batches()[0]?.isPlaceholderData === true;
	const [now, setNow] = createSignal(Date.now());
	// Every answer, a poll's too, moves the time a running job counts from now.
	createComputed(
		on(
			() => batches()[0]?.dataUpdatedAt,
			() => setNow(Date.now()),
		),
	);

	const lines = createMemo(() =>
		loaded().flatMap((batch) =>
			batch.data ? linesOfBatch(batch.data.report) : [],
		),
	);
	// Each row keeps its object while only the batches change, so its open plot stays.
	const rows = createMemo(
		mapArray(lines, (line) =>
			createMemo(() => lineAt(line, report()?.uuid ?? "", view().iteration)),
		),
	);
	const shown = () => rows().map((row) => row());
	const groups = createMemo<GroupRow[]>(() => {
		const batch = first();
		return batch ? groupsOfBatch(batch) : [];
	});
	const slots = createMemo(() => slotsOf(groups(), shown()));
	const nearEnd = () => {
		const last = loaded().at(-1)?.data?.report;
		if (
			last &&
			next() === undefined &&
			lines().length < (report()?.total ?? 0) &&
			last.lines.length === perPage
		) {
			setMore({ key: linesKey(), count: batches().length + 1 });
		}
	};

	const expanded = createMemo(() => new Set(view().expanded));
	const expand = (line: ReportLine, open: boolean) =>
		setView(
			{
				...view(),
				expanded: open
					? [...view().expanded, line.key]
					: view().expanded.filter((key) => key !== line.key),
			},
			true,
		);
	const [selection, setSelection] = createSignal<
		ReadonlyMap<string, ReportLine>
	>(new Map());
	// Explore's query codec loads with the first selection or once the lines want their links, not with the page.
	const [explorer, setExplorer] =
		createSignal<typeof import("../report/explore")["exploreSearchOf"]>();
	const loadExplorer = () => {
		if (!explorer()) {
			import("../report/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
	};
	const select = (line: ReportLine, selected: boolean) => {
		loadExplorer();
		const chosen = new Map(selection());
		if (selected) {
			chosen.set(line.key, line);
		} else {
			chosen.delete(line.key);
		}
		setSelection(chosen);
	};
	const selectedKeys = createMemo(() => new Set(selection().keys()));
	const [sheet, setSheet] = createSignal<ReportLine>();
	// Another report starts with nothing chosen.
	createComputed(
		on(
			() => params.report,
			() => {
				setSelection(new Map());
				setSheet();
			},
			{ defer: true },
		),
	);

	// Data already seen keeps the page up; only a report never loaded shows a failure.
	const failure = () => {
		const error = batches()[0]?.error;
		return report() || !error
			? undefined
			: error instanceof ApiError
				? error.kind
				: "network";
	};
	const missing = () => !isReportId(params.report) || failure() === "not_found";
	const windowLabel = () => {
		const window = view().window;
		return typeof window === "number" ? `${windowDays(window)}d` : window;
	};
	const hash = () => shortHash(report()?.version.hash);
	/** Explore's link for some of the report's lines, once the codec has arrived. */
	const explore = () => {
		const current = report();
		const search = explorer();
		return current && search
			? (lines: readonly ReportLine[]) =>
					`${projectPath(slug(), "explore")}${search(
						lines,
						{ uuid: current.uuid, end: current.window.end_time },
						windowDays(view().window),
					)}`
			: undefined;
	};
	const step = (side: "previous" | "next") => {
		const current = report();
		const link = current?.run?.[side];
		if (!(current?.run && link)) {
			return undefined;
		}
		const shown = narrow()
			? `#${link.order}`
			: `#${link.order} · ${link.testbed}`;
		return {
			href: reportPath(slug(), link.uuid) + query(view()),
			shown,
			detail: `report ${formatCount(link.order)} of ${formatCount(current.run.total)}${narrow() ? `, ${link.testbed}` : ""}`,
		};
	};
	const intent = (side: "previous" | "next") => {
		const link = report()?.run?.[side];
		if (link) {
			prefetchReport(client, api, slug(), link.uuid, view(), perPage);
		}
	};
	// The run page's first batch comes with its code, which the move needs anyway, and the code
	// that draws its alerts when this report raised some.
	const intendRun = (version: string) => {
		if ((report()?.counts.alerts.total ?? 0) > 0) {
			import("../runs/RunAlerts").catch(() => {});
		}
		import("./Run")
			.then(({ prefetch }) => prefetch(client, api, slug(), version))
			.catch(() => {});
	};

	return (
		<Show when={!missing()} fallback={<NotFound slug={slug()} />}>
			<main class="page">
				<div class="pagehead rp-head">
					<div class="ph-title">
						<Crumb
							slug={slug()}
							report={report()}
							readOnly={bootstrap().data?.permissions.edit === false}
							onRunIntent={intendRun}
						/>
						<Title report={report()} now={now()} />
					</div>
					<div class="ph-actions rp-actions">
						<Neighbor
							side="previous"
							link={step("previous")}
							onIntent={() => intent("previous")}
						/>
						<Neighbor
							side="next"
							link={step("next")}
							onIntent={() => intent("next")}
						/>
					</div>
					<p class="ph-sub rp-sub">
						<Show
							when={report()}
							fallback={<Skeleton size="text" class="rp-sub-skel" />}
						>
							{(report) => (
								<For
									each={subtitle(
										report(),
										// The run's crumb names the hash when there is one.
										report().run ? undefined : hash(),
										view().search ? undefined : report().total,
										narrow(),
									)}
								>
									{(part, index) => (
										<>
											{index() > 0 && " · "}
											{part === SPEC ? (
												<Spec spec={report().spec} narrow={narrow()} />
											) : (
												part
											)}
										</>
									)}
								</For>
							)}
						</Show>
					</p>
				</div>
				<Controls
					view={view()}
					iterations={report()?.counts.iterations ?? 0}
					narrow={narrow()}
					onView={setView}
				/>
				<Selection
					count={selection().size}
					narrow={narrow()}
					explore={explore()?.([...selection().values()])}
					onClear={() => setSelection(new Map())}
				/>
				<Switch>
					{/* Until a runner job's report is finished, its heading says why it has no lines. */}
					<Match when={report()?.total === 0 && report()?.job !== undefined}>
						{null}
					</Match>
					<Match when={report()?.total === 0 && !busy()}>
						<div class="card soft rp-empty">
							<Heading level={2} size="lg">
								No lines
							</Heading>
							<Show when={view().search}>
								<Button onClick={() => setView({ ...view(), search: "" })}>
									Clear the filter
								</Button>
							</Show>
						</div>
					</Match>
					<Match when={report()}>
						{(report) => (
							<LineTable
								label={`Lines in report ${hash() ?? report().uuid}, grouped by ${first()?.group}`}
								slots={slots()}
								rows={groups().length + report().total}
								narrow={fold()}
								metrics={report().counts.metrics > 1}
								history={`History, ${windowLabel()}`}
								expanded={expanded()}
								onExpand={expand}
								selected={selectedKeys()}
								onSelect={select}
								onNoThreshold={setSheet}
								declares={version() === 0 ? declaresThresholdV0 : undefined}
								explore={explore()}
								onExploreIntent={loadExplorer}
								href={(line) =>
									line.alert && alertPath(slug(), line.alert.uuid)
								}
								plot={(line) => ({
									data: plotOf(
										line,
										report().branch.name,
										report().testbed.name,
									),
									note: `${windowLabel()}, ending at this report`,
									reportHref: (uuid) => reportPath(slug(), uuid),
									iteration: line.chosen,
									kept,
									canChange,
								})}
								onNearEnd={nearEnd}
								busy={busy()}
								loading={next()?.isFetching ?? false}
								onRetry={
									next()?.isLoadingError ? () => next()?.refetch() : undefined
								}
							/>
						)}
					</Match>
					<Match when={failure()}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">This report did not load</span>
							<Button size="sm" onClick={() => batches()[0]?.refetch()}>
								Retry
							</Button>
						</Banner>
					</Match>
					<Match when={true}>
						<div class="lr-table rp-skeleton" aria-busy="true">
							<span class="sr-only">Loading the report's lines</span>
							<For each={Array.from({ length: SKELETON_ROWS })}>
								{() => <Skeleton size="text" class="rp-skeleton-row" />}
							</For>
						</div>
					</Match>
				</Switch>
				<Show when={report()}>
					{(report) => (
						<ThresholdSheet
							line={sheet()}
							lines={lines()}
							report={report().uuid}
							branch={report().branch.name}
							testbed={report().testbed.name}
							adapter={report().adapter}
							version={version()}
							host={host}
							onClose={() => setSheet()}
						/>
					)}
				</Show>
			</main>
		</Show>
	);
};

export default Report;

type Batch = QueryObserverResult<ReportBatch>;

// Each batch resolves its lines and headers once, so the rows on screen keep their objects as data arrives.
const resolved = new WeakMap<JsonConsoleReport, ReportLine[]>();
const linesOfBatch = (report: JsonConsoleReport) => {
	let lines = resolved.get(report);
	if (!lines) {
		lines = linesOf(report);
		resolved.set(report, lines);
	}
	return lines;
};
const headers = new WeakMap<JsonConsoleReport, GroupRow[]>();
const groupsOfBatch = ({ report, group }: ReportBatch) => {
	let groups = headers.get(report);
	if (!groups) {
		groups = groupsOf(report, group);
		headers.set(report, groups);
	}
	return groups;
};

/** The way back to Runs and to the report's run, and whether the reader can only read, where its arrival takes no width from the title. */
const Crumb = (props: {
	slug: string;
	report?: JsonConsoleReport | undefined;
	readOnly?: boolean;
	onRunIntent?: (version: string) => void;
}) => (
	<div class="crumb-line">
		<a href={projectPath(props.slug, "runs")}>Runs</a>
		<span aria-hidden="true">/</span>
		<Show when={props.report?.run && props.report}>
			{(report) => {
				const version = () => report().run?.uuid ?? "";
				const intent = () => props.onRunIntent?.(version());
				return (
					<>
						<a
							class="as-typed"
							href={runPath(props.slug, version())}
							onPointerMove={onMoved(intent)}
							onFocus={intent}
							onTouchStart={intent}
						>
							{report().branch.name} <span aria-hidden="true">·</span>{" "}
							<span class="rn-crumbhash">
								{shortHash(report().version.hash) ??
									`v${report().version.number}`}
							</span>
						</a>
						<span aria-hidden="true">/</span>
					</>
				);
			}}
		</Show>
		<Show when={props.readOnly}>
			<span class="rp-ro">
				<Icon name="read-only" />
				Read only
			</span>
		</Show>
	</div>
);

/**
 * The branch and testbed, and the report's alerts; before they arrive, a
 * placeholder of the same line box, so what arrives later moves nothing.
 */
const Title = (props: {
	report: JsonConsoleReport | undefined;
	now: number;
}) => (
	<div class="rp-titlerow">
		<Heading level={1} size="xl">
			<Show
				when={props.report}
				fallback={
					<>
						<Skeleton size="text" class="rp-title-skel" />
						<span class="sr-only">Loading the report</span>
					</>
				}
			>
				{(report) => (
					<>
						{report().branch.name} · {report().testbed.name}
					</>
				)}
			</Show>
		</Heading>
		<Show when={(props.report?.counts.alerts.total ?? 0) > 0}>
			<span class="rp-alerts">
				<svg class="lr-alertdot" viewBox="0 0 8 8" aria-hidden="true">
					<circle cx="4" cy="4" r="4" />
				</svg>
				{plural(props.report?.counts.alerts.total ?? 0, "alert")}
			</span>
		</Show>
		<Show when={props.report?.job && props.report}>
			{(report) => (
				<StatusPill status={reportStatus(report(), props.now, duration)} />
			)}
		</Show>
	</div>
);

/** Where the sub line names the spec a runner report ran on. */
const SPEC = Symbol("spec");

/** The spec's tooltip loads only for a runner report; until it arrives its name holds the same box. */
const SpecTip = lazy(() => import("../report/SpecTip"));

/** A runner report's spec by name, its OS, architecture, CPU, and memory in a tooltip; a phone, which shows no tooltip, reads its name. */
const Spec = (props: {
	spec: JsonConsoleSpec | undefined;
	narrow: boolean;
}) => (
	<Show when={props.spec}>
		{(spec) => (
			<Show when={!props.narrow} fallback={spec().name}>
				<Suspense
					fallback={
						<button type="button" class="rn-specname">
							{spec().name}
						</button>
					}
				>
					<SpecTip spec={spec()} />
				</Suspense>
			</Show>
		)}
	</Show>
);

/** Its place in its run, when, how, and what the report holds; `lines` is left out while a filter narrows them. */
const subtitle = (
	report: JsonConsoleReport,
	hash: string | undefined,
	lines: number | undefined,
	narrow: boolean,
) => {
	// A runner job's report holds nothing until its results are in, and its status carries its time.
	const job = report.job !== undefined;
	const total = lines === undefined || job ? [] : [plural(lines, "line")];
	const run = report.run;
	// A report alone in its run has no place to tell.
	const place =
		run?.order && run.total > 1
			? `#${formatCount(run.order)} of ${formatCount(run.total)}`
			: undefined;
	const spec = report.spec && SPEC;
	if (narrow) {
		return [
			place,
			startTime(report.start_time, true),
			report.adapter,
			spec,
			hash,
			...total,
		].filter((part): part is string | typeof SPEC => part !== undefined);
	}
	const counts = report.counts;
	return [
		place,
		startTime(report.start_time, false),
		report.adapter,
		spec,
		hash,
		job
			? undefined
			: [
					plural(counts.benchmarks, "benchmark"),
					plural(counts.variants, "variant"),
					plural(counts.measures, "measure"),
					...(counts.metrics > 1 ? [plural(counts.metrics, "metric")] : []),
					...total,
				].join(", "),
		job ? undefined : `took ${duration(report.end_time - report.start_time)}`,
	].filter((part): part is string | typeof SPEC => part !== undefined);
};

const plural = (count: number, noun: string) =>
	`${formatCount(count)} ${count === 1 ? noun : `${noun}s`}`;

/** When the report started, to the second, as its run's reports start close together; a narrow page leaves out the year. */
const startTime = (time: number, narrow: boolean) =>
	new Intl.DateTimeFormat("en-US", {
		month: "short",
		day: "numeric",
		...(narrow ? {} : { year: "numeric" as const }),
		hour: "2-digit",
		minute: "2-digit",
		second: "2-digit",
		hourCycle: "h23",
	}).format(time);

const Selection = (props: {
	count: number;
	narrow: boolean;
	explore: string | undefined;
	onClear: () => void;
}) => (
	<section class="rp-selbar" aria-label="Selected lines">
		<span class="rp-selcount" aria-live="polite">
			{props.count === 1
				? "1 line selected"
				: `${formatCount(props.count)} lines selected`}
		</span>
		<Show when={props.count > 0}>
			<button
				type="button"
				class="lnk rp-clear"
				onClick={() => props.onClear()}
			>
				Clear
			</button>
		</Show>
		<span class="spacer" />
		<Show
			when={props.count > 0 && props.explore}
			fallback={
				<Button size="sm" variant="primary" disabled>
					Open in Explore
				</Button>
			}
		>
			{(href) => (
				<a
					class="ui-button"
					data-variant="primary"
					data-size="sm"
					href={href()}
					aria-label={`Open ${formatCount(props.count)} in Explore`}
				>
					{props.narrow
						? `Open ${formatCount(props.count)}`
						: `Open ${formatCount(props.count)} in Explore`}
				</a>
			)}
		</Show>
	</section>
);

const NotFound = (props: { slug: string }) => (
	<main class="page">
		<div class="pagehead">
			<div class="ph-title">
				<Crumb slug={props.slug} />
				<Heading level={1} size="xl">
					Report not found
				</Heading>
			</div>
		</div>
	</main>
);
