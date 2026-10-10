import { useLocation, useNavigate, useParams } from "@solidjs/router";
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
	Switch,
	createComputed,
	createMemo,
	createSignal,
	mapArray,
	on,
} from "solid-js";
import type { JsonConsoleReport } from "../../types/bencher";
import { ApiError } from "../api";
import { projectPath, reportPath } from "../paths";
import { formatWhen } from "../plot/format";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { consoleProjectQuery } from "../queries";
import { useQueryResult } from "../query";
import Controls from "../report/Controls";
import LineTable from "../report/LineTable";
import {
	type GroupRow,
	type ReportLine,
	groupsOf,
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
import { slotsOf } from "../report/slots";
import ThresholdSheet from "../report/ThresholdSheet";
import {
	type ReportView,
	parseReportView,
	reportViewSearch,
	windowDays,
} from "../report/view";
import { absoluteTime, duration, shortHash } from "../reports/format";

const SKELETON_ROWS = 8;

/** A report as one row per line, each with its history over a window ending at the report. */
const Report = () => {
	const { api, slug } = useProject();
	const params = useParams<{ report: string }>();
	const location = useLocation();
	const navigate = useNavigate();
	const client = useQueryClient();
	const narrow = useNarrow();
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

	const perPage = screenBatch();
	const linesKey = createMemo(
		() => `${params.report}${query({ ...view(), expanded: [] })}`,
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
	/** The rows on screen are the last view's, until the new one answers. */
	const busy = () => batches()[0]?.isPlaceholderData === true;

	const lines = createMemo(() =>
		loaded().flatMap((batch) =>
			batch.data ? linesOfBatch(batch.data.report) : [],
		),
	);
	const groups = createMemo<GroupRow[]>(() => {
		const batch = first();
		return batch ? groupsOfBatch(batch) : [];
	});
	const slots = createMemo(() => slotsOf(groups(), lines()));
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
	// Explore's query codec loads with the first selection, not with the page.
	const [explorer, setExplorer] =
		createSignal<typeof import("../report/explore")["exploreSearchOf"]>();
	const select = (line: ReportLine, selected: boolean) => {
		if (!explorer()) {
			import("../report/explore")
				.then(({ exploreSearchOf }) => setExplorer(() => exploreSearchOf))
				.catch(() => {});
		}
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
	const explore = () => {
		const current = report();
		const search = explorer();
		return (
			current &&
			search &&
			`${projectPath(slug(), "explore")}${search(
				[...selection().values()],
				{ uuid: current.uuid, end: current.window.end_time },
				windowDays(view().window),
			)}`
		);
	};
	const intent = (uuid: string) =>
		prefetchReport(client, api, slug(), uuid, view(), perPage);

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
				<div class="pagehead rp-head">
					<div class="ph-title">
						<Crumb slug={slug()} />
						<Title report={report()} />
					</div>
					<div class="ph-actions rp-actions">
						<Neighbor
							side="previous"
							report={report()}
							href={(uuid) => reportPath(slug(), uuid) + query(view())}
							onIntent={intent}
						/>
						<Neighbor
							side="next"
							report={report()}
							href={(uuid) => reportPath(slug(), uuid) + query(view())}
							onIntent={intent}
						/>
					</div>
					<p class="ph-sub rp-sub">
						<Show
							when={report()}
							fallback={<Skeleton size="text" class="rp-sub-skel" />}
						>
							{(report) => (
								<>
									{subtitle(
										report(),
										hash(),
										view().search ? undefined : report().total,
										narrow(),
									)}
								</>
							)}
						</Show>
					</p>
					<Show when={bootstrap().data?.permissions.edit === false}>
						<p class="ro-note">
							<Icon name="read-only" />
							Read only. Ask a project Maintainer for access.
						</p>
					</Show>
				</div>
				<Controls view={view()} narrow={narrow()} onView={setView} />
				<Selection
					count={selection().size}
					narrow={narrow()}
					explore={explore()}
					onClear={() => setSelection(new Map())}
				/>
				<Switch>
					<Match when={report()?.total === 0 && !busy()}>
						<div class="card soft rp-empty">
							<Heading level={2} size="lg">
								{view().search
									? `No lines match "${view().search}"`
									: "This report has no lines"}
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
								narrow={narrow()}
								metrics={report().counts.metrics > 1}
								history={`History, ${windowLabel()}`}
								expanded={expanded()}
								onExpand={expand}
								selected={selectedKeys()}
								onSelect={select}
								onNoThreshold={setSheet}
								plot={(line) => ({
									data: plotOf(
										line,
										report().branch.name,
										report().testbed.name,
									),
									note: `${windowLabel()}, ending at this report`,
									reportHref: (uuid) => reportPath(slug(), uuid),
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
							<span class="grow">
								This report did not load: the Bencher API did not answer.
							</span>
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
							host={document.getElementById("console")?.dataset.apiUrl ?? ""}
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

const Crumb = (props: { slug: string }) => (
	<div class="crumb-line">
		<a href={projectPath(props.slug, "reports")}>Reports</a>
		<span aria-hidden="true">/</span>
	</div>
);

/** The branch and testbed, and the report's alerts; before they arrive, a placeholder of the same line box. */
const Title = (props: { report: JsonConsoleReport | undefined }) => (
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
	</div>
);

/** When, how, and what the run reported; `lines` is left out while a filter narrows them. */
const subtitle = (
	report: JsonConsoleReport,
	hash: string | undefined,
	lines: number | undefined,
	narrow: boolean,
) => {
	const total = lines === undefined ? [] : [plural(lines, "line")];
	if (narrow) {
		return [formatWhen(report.start_time), report.adapter, hash, ...total]
			.filter(Boolean)
			.join(" · ");
	}
	const counts = report.counts;
	return [
		absoluteTime(report.start_time),
		report.adapter,
		hash,
		[
			plural(counts.benchmarks, "benchmark"),
			plural(counts.variants, "variant"),
			plural(counts.measures, "measure"),
			...(counts.metrics > 1 ? [plural(counts.metrics, "metric")] : []),
			...total,
		].join(", "),
		`took ${duration(report.end_time - report.start_time)}`,
	]
		.filter(Boolean)
		.join(" · ");
};

const plural = (count: number, noun: string) =>
	`${count} ${count === 1 ? noun : `${noun}s`}`;

/** A link to the branch's report on one side, or that side's control, disabled, while there is none. */
const Neighbor = (props: {
	side: "previous" | "next";
	report: JsonConsoleReport | undefined;
	href: (uuid: string) => string;
	onIntent: (uuid: string) => void;
}) => {
	const text = () => (props.side === "previous" ? "Previous" : "Next");
	const chevron = () => (
		<svg class="rp-chevron" viewBox="0 0 24 24" aria-hidden="true">
			<path d={props.side === "previous" ? "M15 5l-7 7 7 7" : "M9 5l7 7-7 7"} />
		</svg>
	);
	return (
		<Show
			when={props.report?.[props.side]}
			fallback={
				<Button size="sm" class="rp-neighbor" disabled>
					{text()}
				</Button>
			}
		>
			{(link) => {
				const when = () => formatWhen(link().start_time);
				const intent = () => props.onIntent(link().uuid);
				return (
					<a
						class="ui-button rp-neighbor"
						data-variant="secondary"
						data-size="sm"
						href={props.href(link().uuid)}
						aria-label={`${text()} report on ${props.report?.branch.name}, ${props.report?.testbed.name}: ${when()}, ${shortHash(link().hash) ?? link().adapter}`}
						onPointerEnter={intent}
						onFocus={intent}
						onTouchStart={intent}
					>
						<Show when={props.side === "previous"}>{chevron()}</Show>
						{text()}
						<span class="muted">{when()}</span>
						<Show when={props.side === "next"}>{chevron()}</Show>
					</a>
				);
			}}
		</Show>
	);
};

const Selection = (props: {
	count: number;
	narrow: boolean;
	explore: string | undefined;
	onClear: () => void;
}) => (
	<section class="rp-selbar" aria-label="Selected lines">
		<span class="rp-selcount" aria-live="polite">
			{props.count === 0
				? "Select lines to open them together in Explore"
				: props.count === 1
					? "1 line selected"
					: `${props.count} lines selected`}
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
					aria-label={`Open ${plural(props.count, "line")} in Explore`}
				>
					{props.narrow
						? `Open ${props.count}`
						: `Open ${props.count} in Explore`}
				</a>
			)}
		</Show>
	</section>
);

const NotFound = (props: { slug: string; project: string }) => (
	<main class="page">
		<div class="pagehead">
			<div class="ph-title">
				<Crumb slug={props.slug} />
				<Heading level={1} size="xl">
					Report not found
				</Heading>
			</div>
		</div>
		<section class="card soft rp-missing">
			<Heading level={2} size="lg">
				{props.project} has no report here
			</Heading>
			<p>It may have been deleted, or the link may be mistyped.</p>
			<a
				class="ui-button"
				data-variant="primary"
				data-size="md"
				href={projectPath(props.slug, "reports")}
			>
				Open Reports
			</a>
		</section>
	</main>
);
