import { useLocation, useNavigate, useParams } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Skeleton from "@bencherdev/ui/Skeleton";
import {
	For,
	Match,
	Show,
	Switch,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	on,
} from "solid-js";
import type {
	JsonConsoleAlerts,
	JsonConsoleThreshold,
} from "../../types/bencher";
import { ApiError } from "../api";
import { projectPath, reportPath } from "../paths";
import { formatDate, formatWhen } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { FIRST, useAlertBatches } from "../alerts/batches";
import { useFold } from "../alerts/fold";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import LineTable from "../report/LineTable";
import { plotOf } from "../report/lines";
import AlertControls from "../thresholds/AlertControls";
import AlertActions from "../alerts/AlertActions";
import { useRowStatus } from "../alerts/rowStatus";
import { type AlertLine, alertLinesOf } from "../alerts/lines";
import { distinctLines, visibleLines, withKept } from "../alerts/rows";
import { withPending } from "../alerts/status";
import { lineLabel } from "../report/row";
import { Cards, Change } from "../thresholds/Details";
import { archivedBy, dayText, filterSummary } from "../thresholds/model";
import { alertsBatch, alertsQuery, thresholdQuery } from "../thresholds/query";
import { VALUE } from "../thresholds/rows";
import {
	type ThresholdView,
	decodeView,
	encodeView,
	isUuid,
	windowLabel,
} from "../thresholds/search";

const SKELETON_ROWS = 4;

/** A threshold: what it applies to, its model and their history, how to change it, and the alerts it raised. */
const Threshold = () => {
	const { api, slug, host } = useProject();
	const params = useParams<{ threshold: string }>();
	const location = useLocation();
	const navigate = useNavigate();
	const fold = useFold();
	const bootstrap = useQueryResult(() => consoleProjectQuery(api, slug()));
	const view = createMemo(() =>
		decodeView(new URLSearchParams(location.search)),
	);
	const setView = (next: ThresholdView) => {
		const query = encodeView(next);
		// Router paths are relative to the console's base.
		navigate(
			`/${slug()}/thresholds/${params.threshold}${query ? `?${query}` : ""}`,
			{ scroll: false },
		);
	};
	const valid = () => isUuid(params.threshold);
	const threshold = useQueryResult(() => ({
		...thresholdQuery(api, slug(), params.threshold),
		enabled: valid(),
	}));
	const data = () => threshold().data;

	const perPage = alertsBatch();
	const { batches, loaded, next, first, busy, loading, nearEnd, reread } =
		useAlertBatches({
			view: () => `${params.threshold}?${encodeView(view())}`,
			query: (start) => ({
				...alertsQuery(api, slug(), params.threshold, view(), {
					...start,
					perPage,
				}),
				enabled: valid(),
			}),
			alerts: (alerts) => alerts,
			perPage,
			inView: (): number => rows.inView(listed(), view().status),
			keysInView: (alerts) =>
				rows
					.linesInView(linesOfBatch(alerts), view().status)
					.map(({ key }) => key),
			inFlight: (): number => rows.inFlight(),
		});
	const listed = createMemo(() =>
		distinctLines(
			loaded().map((batch) => (batch.data ? linesOfBatch(batch.data) : [])),
		),
	);
	let table: HTMLElement | undefined;
	let status: HTMLElement | undefined;
	const rows = useRowStatus({
		cached: () => ["console", "threshold", slug(), params.threshold, "alerts"],
		reread,
		first: () =>
			alertsQuery(api, slug(), params.threshold, view(), {
				...FIRST,
				perPage,
			}).queryKey,
		filters: () =>
			`${params.threshold}?${encodeView({ ...view(), status: "active" })}`,
		shown: () => lines(),
		table: () => table,
		empty: () => status?.querySelector<HTMLElement>("input:checked"),
		keep: (from, to) => {
			const kept = resolved.get(from);
			if (kept) {
				resolved.set(to, kept);
			}
		},
	});
	const lines = createMemo(() =>
		visibleLines(
			withKept(listed(), rows.kept()),
			view().status,
			rows.statusOf,
			rows.keptHere,
		),
	);
	const canEdit = () => bootstrap().data?.permissions.edit === true;
	const [now, setNow] = createSignal(Date.now());
	createComputed(
		on(
			() => batches()[0]?.dataUpdatedAt,
			() => setNow(Date.now()),
		),
	);

	const [expanded, setExpanded] = createSignal<ReadonlySet<string>>(new Set());
	createComputed(
		on(
			() => params.threshold,
			() => setExpanded(new Set<string>()),
			{ defer: true },
		),
	);
	const expand = (line: { key: string }, open: boolean) => {
		const keys = new Set(expanded());
		if (open) {
			keys.add(line.key);
		} else {
			keys.delete(line.key);
		}
		setExpanded(keys);
	};

	// Explore's query codec loads once there are lines to open, not with the page.
	const [explorer, setExplorer] =
		createSignal<typeof import("../thresholds/explore")["exploreSearchOf"]>();
	createEffect(() => {
		if (listed().length > 0 && !explorer()) {
			import("../thresholds/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
	});
	const explore = () => {
		const search = explorer();
		return search && listed().length > 0
			? `${projectPath(slug(), "explore")}${search(listed(), view().window)}`
			: undefined;
	};

	const failure = () => {
		const error = threshold().error;
		return data() || !error
			? undefined
			: error instanceof ApiError
				? error.kind
				: "network";
	};
	const missing = () => !valid() || failure() === "not_found";
	const history = () =>
		view().window.kind === "custom" ? "range" : windowLabel(view().window);

	return (
		<Show when={!missing()} fallback={<NotFound slug={slug()} />}>
			<main class="page">
				<div class="pagehead th-head">
					<div class="ph-title">
						<div class="crumb-line">
							<a href={projectPath(slug(), "thresholds")}>Thresholds</a>
							<span aria-hidden="true">/</span>
						</div>
						<Title threshold={data()} />
					</div>
					<div class="ph-actions">
						<Show
							when={explore()}
							fallback={
								<Button size="sm" disabled>
									Open in Explore
								</Button>
							}
						>
							{(href) => (
								<a
									class="ui-button"
									data-variant="secondary"
									data-size="sm"
									href={href()}
									aria-label="Open in Explore: the lines this threshold alerted on"
								>
									Open in Explore
								</a>
							)}
						</Show>
					</div>
					<p class="ph-sub">
						<Show
							when={data()}
							fallback={<Skeleton size="text" class="th-sub-skel" />}
						>
							{(threshold) => (
								<>Declared {dayText(threshold().created, now())}</>
							)}
						</Show>
					</p>
				</div>
				<Switch>
					<Match when={data()}>
						{(threshold) => (
							<>
								<Cards slug={slug()} threshold={threshold()} now={now()} />
								<Change slug={slug()} threshold={threshold()} host={host} />
								<section class="th-alerts" aria-labelledby="th-alerts">
									<div class="th-alerts-head">
										<h2 class="seclabel" id="th-alerts">
											Alerts raised
										</h2>
										<span class="th-quiet th-summary">
											<Show
												when={first()?.counts}
												fallback={
													<Skeleton size="text" class="th-summary-skel" />
												}
											>
												{(counts) => {
													const shown = () =>
														withPending(counts(), [rows.pending()]);
													return (
														<>
															{shown().active} active ·{" "}
															{shown().dismissed + shown().silenced} dismissed
														</>
													);
												}}
											</Show>
										</span>
									</div>
									<AlertControls
										view={view()}
										now={now()}
										onView={setView}
										statusRef={(element) => {
											status = element;
										}}
									/>
									<Show when={rows.notice()}>
										{(notice) => (
											<Banner status="error" role="alert" class="load-error">
												<span class="grow">
													{notice() === "partial"
														? "Some alerts did not change"
														: "The alert did not change"}
												</span>
											</Banner>
										)}
									</Show>
									<Alerts
										slug={slug()}
										threshold={threshold()}
										view={view()}
										lines={lines()}
										total={first()?.total}
										rows={Math.max(
											lines().length,
											(first()?.total ?? 0) + lines().length - listed().length,
										)}
										narrow={fold()}
										tableRef={(element) => {
											table = element;
										}}
										history={history()}
										expanded={expanded()}
										onExpand={expand}
										onNearEnd={nearEnd}
										busy={busy()}
										loading={loading()}
										failed={
											first() === undefined &&
											batches()[0]?.isLoadingError === true
										}
										onRetry={(batch) =>
											(batch === "first" ? batches()[0] : next())?.refetch()
										}
										nextFailed={next()?.isLoadingError === true}
										statusOf={rows.statusOf}
										changed={rows.changed}
										onChange={canEdit() ? rows.change : undefined}
									/>
								</section>
							</>
						)}
					</Match>
					<Match when={failure()}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">This threshold did not load</span>
							<Button size="sm" onClick={() => threshold().refetch()}>
								Retry
							</Button>
						</Banner>
					</Match>
					<Match when={true}>
						<Skeleton size="card" />
					</Match>
				</Switch>
			</main>
		</Show>
	);
};

export default Threshold;

// Each batch resolves its lines once, so the rows on screen keep their objects as data arrives.
const resolved = new WeakMap<JsonConsoleAlerts, AlertLine[]>();
const linesOfBatch = (batch: JsonConsoleAlerts) => {
	let lines = resolved.get(batch);
	if (!lines) {
		lines = alertLinesOf(batch);
		resolved.set(batch, lines);
	}
	return lines;
};

/** What the threshold applies to, and its state; before it arrives, a placeholder of the same line box. */
const Title = (props: { threshold: JsonConsoleThreshold | undefined }) => (
	<div class="th-titlerow">
		<Heading level={1} size="xl">
			<Show
				when={props.threshold}
				fallback={
					<>
						<Skeleton size="text" class="th-title-skel" />
						<span class="sr-only">Loading the threshold</span>
					</>
				}
			>
				{(threshold) => (
					<>
						{[
							threshold().branch.name,
							threshold().testbed.name,
							threshold().measure.name,
							threshold().metric ?? VALUE,
						].join(" · ")}
					</>
				)}
			</Show>
		</Heading>
		<Show when={props.threshold}>
			{(threshold) => (
				<>
					<span class="th-pill">{filterSummary(threshold().parameters)}</span>
					<span
						class="th-pill"
						classList={{
							"th-pill-ok": !archivedBy(threshold()) && !!threshold().model,
						}}
					>
						{archivedBy(threshold())
							? "archived"
							: threshold().model
								? "active"
								: "no model"}
					</span>
				</>
			)}
		</Show>
	</div>
);

const Alerts = (props: {
	slug: string;
	threshold: JsonConsoleThreshold;
	view: ThresholdView;
	lines: AlertLine[];
	total: number | undefined;
	/** The rows the list counts: the view's total and the rows kept or let go on this page. */
	rows: number;
	narrow: boolean;
	tableRef: (element: HTMLElement) => void;
	history: string;
	expanded: ReadonlySet<string>;
	onExpand: (line: { key: string }, open: boolean) => void;
	onNearEnd: () => void;
	busy: boolean;
	loading: boolean;
	/** The first batch failed; nothing is on screen. */
	failed: boolean;
	/** The batch after the loaded ones failed. */
	nextFailed: boolean;
	onRetry: (batch: "first" | "next") => void;
	statusOf: (line: AlertLine) => AlertLine["status"];
	changed: (line: AlertLine) => boolean;
	/** Without it the reader cannot change an alert's status. */
	onChange?:
		| ((status: "active" | "dismissed", line: AlertLine) => void)
		| undefined;
}) => (
	<Switch>
		<Match
			when={
				props.total !== undefined && props.lines.length === 0 && !props.busy
			}
		>
			<div class="th-empty">
				<Heading level={3} size="base">
					No alerts
				</Heading>
			</div>
		</Match>
		<Match when={props.lines.length > 0}>
			<div class="lr-alerts" ref={props.tableRef}>
				<LineTable
					label="Alerts this threshold raised"
					slots={props.lines}
					rows={props.rows}
					narrow={props.narrow}
					metrics={false}
					history={`History, ${props.history}`}
					expanded={props.expanded}
					onExpand={props.onExpand}
					aside={{
						label: "Report",
						column: "th-c-report",
						cell: (line) => (
							<ReportCell slug={props.slug} line={line as AlertLine} />
						),
					}}
					dimmed={(line) => props.statusOf(line as AlertLine) !== "active"}
					actions={{
						cell: (line) => {
							const alert = line as AlertLine;
							const change = props.onChange;
							return (
								<AlertActions
									name={
										lineLabel(
											{
												benchmark: line.benchmark.name,
												parameters: line.parameters,
												measure: line.measure.name,
												metric: line.metric,
											},
											false,
										).name
									}
									status={props.statusOf(alert)}
									justNow={props.changed(alert)}
									modified={alert.modified}
									onDismiss={change && (() => change("dismissed", alert))}
									onReactivate={change && (() => change("active", alert))}
								/>
							);
						},
						// An editor's rows never read their status here, so a change keeps their controls and the focus.
						narrow: (line) =>
							props.onChange !== undefined ||
							props.statusOf(line as AlertLine) !== "active",
					}}
					plot={(line) => ({
						data: plotOf(
							line,
							props.threshold.branch.name,
							props.threshold.testbed.name,
						),
						note: `${props.history}, ending at its report`,
						reportHref: (uuid) => reportPath(props.slug, uuid),
					})}
					onNearEnd={props.onNearEnd}
					busy={props.busy}
					loading={props.loading}
					onRetry={props.nextFailed ? () => props.onRetry("next") : undefined}
				/>
			</div>
		</Match>
		<Match when={props.failed}>
			<Banner status="error" role="alert" class="load-error">
				<span class="grow">These alerts did not load</span>
				<Button size="sm" onClick={() => props.onRetry("first")}>
					Retry
				</Button>
			</Banner>
		</Match>
		<Match when={true}>
			<div class="lr-table th-alerts-skeleton" aria-busy="true">
				<span class="sr-only">Loading the alerts</span>
				<For each={Array.from({ length: SKELETON_ROWS })}>
					{() => <Skeleton size="text" class="rp-skeleton-row" />}
				</For>
			</div>
		</Match>
	</Switch>
);

/** The report an alert came from. */
const ReportCell = (props: { slug: string; line: AlertLine }) => {
	const narrow = useNarrow();
	const when = () =>
		narrow()
			? formatDate(props.line.report.start)
			: formatWhen(props.line.report.start);
	const hash = () => (narrow() ? undefined : props.line.report.hash);
	return (
		<span class="th-report">
			<a
				href={reportPath(props.slug, props.line.report.uuid)}
				aria-label={`${[when(), hash()].filter(Boolean).join(" · ")}, open the report`}
			>
				{when()}
				<Show when={hash()}>
					{(shown) => (
						<>
							{" "}
							<span class="th-report-hash">· {shown()}</span>
						</>
					)}
				</Show>
			</a>
		</span>
	);
};

const NotFound = (props: { slug: string }) => (
	<main class="page">
		<div class="pagehead">
			<div class="ph-title">
				<div class="crumb-line">
					<a href={projectPath(props.slug, "thresholds")}>Thresholds</a>
					<span aria-hidden="true">/</span>
				</div>
				<Heading level={1} size="xl">
					Threshold not found
				</Heading>
			</div>
		</div>
	</main>
);
