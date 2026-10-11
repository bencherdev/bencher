import { useLocation, useNavigate, useParams } from "@solidjs/router";
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
import type {
	JsonConsoleAlerts,
	JsonConsoleThreshold,
} from "../../types/bencher";
import { ApiError } from "../api";
import { projectPath, reportPath } from "../paths";
import { formatDate, formatWhen } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import LineTable from "../report/LineTable";
import { plotOf } from "../report/lines";
import AlertControls from "../thresholds/AlertControls";
import { type AlertLine, alertLinesOf } from "../thresholds/alerts";
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
	windowPhrase,
} from "../thresholds/search";

const SKELETON_ROWS = 4;

type Batch = QueryObserverResult<JsonConsoleAlerts>;

/** A threshold: what it applies to, its model and their history, how to change it, and the alerts it raised. */
const Threshold = () => {
	const { api, slug } = useProject();
	const params = useParams<{ threshold: string }>();
	const location = useLocation();
	const navigate = useNavigate();
	const narrow = useNarrow();
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
	const alertsKey = createMemo(
		() => `${params.threshold}?${encodeView(view())}`,
	);
	// Later batches belong to the view that asked for them, so a new view drops them before they could ask again.
	const [more, setMore] = createSignal({ key: "", count: 1 });
	createComputed(
		on(alertsKey, (key) => setMore({ key, count: 1 }), { defer: true }),
	);
	const queries = createMemo(
		mapArray(
			() =>
				Array.from(
					{ length: more().key === alertsKey() ? more().count : 1 },
					(_, index) => index + 1,
				),
			(page) =>
				useQueryResult(() => ({
					...alertsQuery(api, slug(), params.threshold, view(), {
						page,
						perPage,
					}),
					enabled: valid(),
				})),
		),
	);
	const batches = () => queries().map((batch) => batch());
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
	const next = () => batches()[loaded().length];
	const first = () => loaded()[0]?.data;
	const busy = () => batches()[0]?.isPlaceholderData === true;
	const lines = createMemo(() =>
		loaded().flatMap((batch) => (batch.data ? linesOfBatch(batch.data) : [])),
	);
	const nearEnd = () => {
		const last = loaded().at(-1)?.data;
		if (
			last &&
			next() === undefined &&
			lines().length < last.total &&
			last.groups.reduce((sum, group) => sum + group.alerts.length, 0) ===
				perPage
		) {
			setMore({ key: alertsKey(), count: batches().length + 1 });
		}
	};
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
		if (lines().length > 0 && !explorer()) {
			import("../thresholds/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
	});
	const explore = () => {
		const search = explorer();
		return search && lines().length > 0
			? `${projectPath(slug(), "explore")}${search(lines(), view().window)}`
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
	const windowText = () => windowPhrase(view().window, now());
	const history = () =>
		view().window.kind === "custom" ? "range" : windowLabel(view().window);

	return (
		<Show
			when={!missing()}
			fallback={
				<NotFound
					slug={slug()}
					project={bootstrap().data?.project.name ?? slug()}
				/>
			}
		>
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
								<Cards threshold={threshold()} now={now()} />
								<Change
									slug={slug()}
									threshold={threshold()}
									host={
										document.getElementById("console")?.dataset.apiUrl ?? ""
									}
								/>
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
												{(counts) => (
													<>
														{counts().active} active ·{" "}
														{counts().dismissed + counts().silenced} dismissed{" "}
														{windowText()}
													</>
												)}
											</Show>
										</span>
									</div>
									<AlertControls view={view()} now={now()} onView={setView} />
									<Alerts
										slug={slug()}
										threshold={threshold()}
										view={view()}
										lines={lines()}
										total={first()?.total}
										narrow={narrow()}
										history={history()}
										expanded={expanded()}
										onExpand={expand}
										onNearEnd={nearEnd}
										busy={busy()}
										loading={next()?.isFetching ?? false}
										failed={
											first() === undefined &&
											batches()[0]?.isLoadingError === true
										}
										onRetry={(batch) =>
											(batch === "first" ? batches()[0] : next())?.refetch()
										}
										nextFailed={next()?.isLoadingError === true}
									/>
								</section>
							</>
						)}
					</Match>
					<Match when={failure()}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">
								This threshold did not load: the Bencher API did not answer.
							</span>
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
	narrow: boolean;
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
}) => (
	<Switch>
		<Match when={props.total === 0 && !props.busy}>
			<div class="th-empty">
				<Heading level={3} size="base">
					{props.view.status === "active"
						? "No active alerts"
						: "No alerts in this window"}
				</Heading>
				<p>
					{props.view.status === "active"
						? "Nothing this threshold raised is active. All lists the dismissed ones."
						: "Widen the window to see older alerts."}
				</p>
			</div>
		</Match>
		<Match when={props.lines.length > 0}>
			<LineTable
				label="Alerts this threshold raised"
				slots={props.lines}
				rows={props.total ?? props.lines.length}
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
		</Match>
		<Match when={props.failed}>
			<Banner status="error" role="alert" class="load-error">
				<span class="grow">
					These alerts did not load: the Bencher API did not answer.
				</span>
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

/** The report an alert came from, and its status once it is no longer active. */
const ReportCell = (props: { slug: string; line: AlertLine }) => {
	const narrow = useNarrow();
	const when = () => formatWhen(props.line.report.start);
	return (
		<span class="th-report">
			<a
				href={reportPath(props.slug, props.line.report.uuid)}
				aria-label={`Open the report from ${when()}${props.line.report.hash ? `, ${props.line.report.hash}` : ""}`}
			>
				{narrow()
					? formatDate(props.line.report.start)
					: [when(), props.line.report.hash].filter(Boolean).join(" · ")}
			</a>
			<Show when={props.line.status !== "active"}>
				<span class="th-quiet"> · {props.line.status}</span>
			</Show>
		</span>
	);
};

const NotFound = (props: { slug: string; project: string }) => (
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
		<section class="rp-missing">
			<Heading level={2} size="lg">
				{props.project} has no threshold here
			</Heading>
			<p>It may have been deleted, or the link may be mistyped.</p>
			<a
				class="ui-button"
				data-variant="primary"
				data-size="md"
				href={projectPath(props.slug, "thresholds")}
			>
				Open Thresholds
			</a>
		</section>
	</main>
);
