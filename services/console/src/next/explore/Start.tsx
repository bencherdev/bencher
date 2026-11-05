import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import Truncate from "@bencherdev/ui/Truncate";
import { tailOf } from "@bencherdev/ui/tail";
import {
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import {
	For,
	type JSX,
	Match,
	Show,
	Switch,
	createMemo,
	createSignal,
	createUniqueId,
	on,
} from "solid-js";
import type {
	JsonAlert,
	JsonConsoleRunSummary,
	JsonPlot,
} from "../../types/bencher";
import { projectPath } from "../paths";
import { formatChange, formatWhen } from "../plot/format";
import { formatCount } from "../plot/count";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import type { ExploreQuery } from "../query/query";
import { exploreSearch } from "../query/selection";
import {
	RUN_REPORTS,
	reportStatus,
	runReportsQuery,
	runStatus,
	statusText,
} from "../runs/status";
import StatusPill from "../runs/StatusPill";
import Tag from "../tag";
import { type Option, SearchMenu, choicesOf } from "./Box";
import { plotSearch } from "./pinned";
import { type Dimension, nameQuery } from "./queries";
import {
	type RunReport,
	START,
	alertLine,
	benchmarkOptions,
	reportBenchmarkQuery,
	reportBenchmarksQuery,
	runHref,
	runName,
	runTime,
	startQueries,
} from "./start";
import { rollingName } from "./window";

/** Three short lists blank Explore starts from, asked for in one round; a run opens to its reports in place, and a report to its benchmarks. */
const Start = (props: { onQuery: (query: ExploreQuery) => void }) => {
	const { api, slug } = useProject();
	const alerts = useQueryResult(() => startQueries(api, slug()).alerts);
	const runs = useQueryResult(() => startQueries(api, slug()).runs);
	const plots = useQueryResult(() => startQueries(api, slug()).plots);
	// Every answer, a poll's too, moves the time a running job counts from now.
	const now = createMemo(
		on(
			() => runs().dataUpdatedAt,
			() => Date.now(),
		),
	);
	const explore = () => projectPath(slug(), "explore");
	const [open, setOpen] = createSignal<string>();
	return (
		<section class="ex-startcard" aria-labelledby="ex-start">
			<h2 class="seclabel" id="ex-start">
				Start from
			</h2>
			<div class="ex-cols">
				<List
					id="ex-runs"
					title="Latest runs"
					all={{ href: projectPath(slug(), "runs"), text: "All runs" }}
					result={runs()}
					rows={START.runs}
					empty="No runs"
				>
					{(run: JsonConsoleRunSummary) => (
						<RunRow
							run={run}
							now={now()}
							open={open() === run.uuid}
							onOpen={(next) => setOpen(next ? run.uuid : undefined)}
							onQuery={props.onQuery}
						/>
					)}
				</List>
				<List
					id="ex-alerts"
					title="Alerting now"
					all={{ href: projectPath(slug(), "alerts"), text: "All alerts" }}
					result={alerts()}
					rows={START.alerts}
					empty="No alerts"
				>
					{(alert: JsonAlert) => {
						const delta =
							alert.boundary.baseline === undefined
								? null
								: formatChange(alert.value, alert.boundary.baseline);
						const tags = Object.entries(alert.variant.parameters).map(
							([key, value]) => `${key}=${String(value)}`,
						);
						const { branch, testbed, measure } = alert.threshold;
						return (
							<a
								class="ex-srow"
								href={`${explore()}${exploreSearch([alertLine(alert)])}`}
								// The router hands an anchor's state to the page it opens.
								ref={(link) =>
									link.setAttribute(
										"state",
										JSON.stringify({
											branch: branch.name,
											testbed: testbed.name,
											uuid: alert.uuid,
										}),
									)
								}
							>
								<span class="ex-alertdot" aria-hidden="true" />
								<span class="grow">
									<span class="ex-sline ex-aline">
										<b class="ex-aname">
											<Truncate text={alert.benchmark.name} tail />
											{/* Unseen, it holds the name at least as wide as its tail. */}
											<span class="ex-hold" aria-hidden="true">
												{tailOf(alert.benchmark.name)[1]}
											</span>
										</b>{" "}
										<span class="ex-stags">
											<Truncate class="muted" text={measure.name} tail />{" "}
											<For each={tags}>
												{(tag) => (
													<>
														<Tag class="ex-tag" tag={tag} tail />{" "}
													</>
												)}
											</For>
										</span>
									</span>
									<span class="ex-sub ui-parts">
										<Truncate text={branch.name} tail />
										<span data-fixed>·</span>
										<Truncate text={testbed.name} tail />
										<Show when={delta}>
											<span data-fixed>· {delta}</span>
										</Show>
									</span>
								</span>
							</a>
						);
					}}
				</List>
				<List
					id="ex-plots"
					title="Pinned plots"
					all={{ href: projectPath(slug(), "plots"), text: "All plots" }}
					result={plots()}
					rows={START.plots}
					empty="No pinned plots"
				>
					{(plot: JsonPlot) => (
						<a class="ex-srow" href={`${explore()}${plotSearch(plot)}`}>
							<span class="grow">
								<b>
									<Truncate text={plot.title ?? "Untitled plot"} />
								</b>
								<span class="ex-sub">{rollingName(plot.window)}</span>
							</span>
						</a>
					)}
				</List>
			</div>
		</section>
	);
};

export default Start;

const List = <T,>(props: {
	id: string;
	title: string;
	all: { href: string; text: string };
	result: QueryObserverResult<T[]>;
	rows: number;
	empty: string;
	children: (item: T) => JSX.Element;
}) => (
	<section
		class="ex-list"
		style={{ "--ex-rows": props.rows }}
		aria-labelledby={props.id}
	>
		<div class="ex-sh">
			<h3 id={props.id}>{props.title}</h3>
			<a href={props.all.href}>{props.all.text}</a>
		</div>
		<Switch
			fallback={
				<For each={Array.from({ length: props.rows })}>
					{() => <Skeleton size="text" class="ex-skel" />}
				</For>
			}
		>
			<Match when={props.result.data?.length === 0}>
				<p class="ex-none">{props.empty}</p>
			</Match>
			<Match when={props.result.data}>
				{(data) => <For each={data()}>{props.children}</For>}
			</Match>
			<Match when={props.result.isError}>
				<p class="ex-none">This list did not load</p>
			</Match>
		</Switch>
	</section>
);

/** A run that opens, anywhere on its row, to its reports; its hash opens the run. */
const RunRow = (props: {
	run: JsonConsoleRunSummary;
	now: number;
	open: boolean;
	onOpen: (open: boolean) => void;
	onQuery: (query: ExploreQuery) => void;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	// Started as the pointer or focus arrives, so the click opens a list already read.
	const prefetch = () =>
		client.query(runReportsQuery(api, slug(), props.run.uuid)).catch(() => {});
	const name = () => runName(props.run);
	const time = () => runTime(props.run.end_time - props.run.start_time);
	const status = () => runStatus(props.run, props.now, runTime);
	const sub = () =>
		[
			formatWhen(props.run.start_time),
			props.run.reports > 1 && `${formatCount(props.run.reports)} reports`,
			props.run.testbed_count > 1 &&
				`${formatCount(props.run.testbed_count)} testbeds`,
		].filter((part): part is string => typeof part === "string");
	const panel = createUniqueId();
	return (
		<div>
			<div class="ex-rhead rowlink">
				<span class="grow">
					<span class="ex-sline ui-parts">
						<button
							type="button"
							class="ex-rbtn rowlink-name"
							// The row's other lines sit outside its button, so its name carries them.
							aria-label={[
								props.run.branch.name,
								`run ${name()}`,
								...sub(),
								status().kind ? statusText(status()) : time(),
							].join(", ")}
							aria-expanded={props.open}
							aria-controls={panel}
							onPointerEnter={prefetch}
							onFocus={prefetch}
							onClick={() => props.onOpen(!props.open)}
						>
							<b>
								<Truncate text={props.run.branch.name} tail />
							</b>
						</button>
						<a
							class="ex-rhash"
							href={runHref(slug(), props.run.uuid)}
							data-fixed
						>
							{name()}
						</a>
					</span>
					<span class="ex-sub">{sub().join(" · ")}</span>
				</span>
				<Show
					when={status().kind}
					fallback={<span class="ex-n">{time()}</span>}
				>
					<StatusPill status={status()} />
				</Show>
				<Icon name="chevron-down" class="ex-rcaret" />
			</div>
			<Show when={props.open}>
				<RunReports
					id={panel}
					run={props.run}
					name={name()}
					onQuery={props.onQuery}
				/>
			</Show>
		</div>
	);
};

/** An opened run's first reports in start order, each opening to its benchmarks; its page lists the rest. */
const RunReports = (props: {
	id: string;
	run: JsonConsoleRunSummary;
	name: string;
	onQuery: (query: ExploreQuery) => void;
}) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() =>
		runReportsQuery(api, slug(), props.run.uuid),
	);
	const now = createMemo(
		on(
			() => result().dataUpdatedAt,
			() => Date.now(),
		),
	);
	const [open, setOpen] = createSignal<string>();
	return (
		<section
			id={props.id}
			class="ex-rpick"
			aria-label={`Reports in run ${props.name}`}
		>
			<Switch
				fallback={
					<For
						each={Array.from({
							length: Math.min(props.run.reports, RUN_REPORTS),
						})}
					>
						{() => <Skeleton size="text" class="ex-skel" />}
					</For>
				}
			>
				<Match when={result().data}>
					{(run) => (
						<>
							<For each={run().reports}>
								{(report) => (
									// A runner job's report has no benchmarks to search until its results are in.
									<Show
										when={!report.job}
										fallback={
											<div class="ex-rhead">
												<ReportLine
													order={report.order}
													branch={run().branch.name}
													testbed={report.testbed.name}
												/>
												<span class="spacer" />
												<StatusPill
													status={reportStatus(report, now(), runTime)}
												/>
											</div>
										}
									>
										<ReportRow
											report={{ ...report, branch: run().branch }}
											order={report.order}
											branch={run().branch.name}
											open={open() === report.uuid}
											onOpen={(next) => setOpen(next ? report.uuid : undefined)}
											onQuery={props.onQuery}
										/>
									</Show>
								)}
							</For>
							<Show when={run().total > run().reports.length}>
								<div class="ex-sh">
									<a href={runHref(slug(), props.run.uuid)}>
										All {run().total} reports
									</a>
								</div>
							</Show>
						</>
					)}
				</Match>
				<Match when={result().isError}>
					<p class="ex-none">This list did not load</p>
				</Match>
			</Switch>
		</section>
	);
};

/** A report by its place in the run, its branch, and its testbed, which share the line fairly. */
const ReportLine = (props: {
	order: number;
	branch: string;
	testbed: string;
}) => (
	<span class="ex-sline ui-parts">
		<span class="mono muted" data-fixed>
			{props.order}
		</span>
		<b>
			<Truncate text={props.branch} tail />
		</b>
		<span class="muted" data-fixed>
			·
		</span>
		<Truncate class="muted" text={props.testbed} tail />
	</span>
);

/** A report of an opened run, by its place in the run and its testbed, that opens to search the benchmarks it ran. */
const ReportRow = (props: {
	report: RunReport;
	order: number;
	branch: string;
	open: boolean;
	onOpen: (open: boolean) => void;
	onQuery: (query: ExploreQuery) => void;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	// Started as the pointer or focus arrives, so the click opens a list already read.
	const prefetch = () =>
		client
			.query(reportBenchmarksQuery(api, slug(), props.report.uuid))
			.catch(() => {});
	const panel = createUniqueId();
	let toggle: HTMLButtonElement | undefined;
	return (
		<div>
			<div class="ex-rhead rowlink">
				<button
					ref={toggle}
					type="button"
					class="ex-rbtn rowlink-name"
					aria-expanded={props.open}
					aria-controls={panel}
					onPointerEnter={prefetch}
					onFocus={prefetch}
					onClick={() => props.onOpen(!props.open)}
				>
					<ReportLine
						order={props.order}
						branch={props.branch}
						testbed={props.report.testbed.name}
					/>
				</button>
				<span class="spacer" />
				<Icon name="chevron-down" class="ex-rcaret" />
			</div>
			<Show when={props.open}>
				<Picker
					id={panel}
					report={props.report}
					branch={props.branch}
					name={`report ${props.order} on ${props.report.testbed.name}`}
					anchor={toggle}
					onClose={() => props.onOpen(false)}
					onQuery={props.onQuery}
				/>
			</Show>
		</div>
	);
};

/** The benchmarks a report ran, searched as the Benchmarks box searches; a pick draws it as the report ran it. */
const Picker = (props: {
	id: string;
	report: RunReport;
	branch: string;
	/** The report, as a reader hears it. */
	name: string;
	anchor: HTMLElement | undefined;
	onClose: () => void;
	onQuery: (query: ExploreQuery) => void;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const result = useQueryResult(() =>
		reportBenchmarksQuery(api, slug(), props.report.uuid),
	);
	// The boxes name what the pick fills before the plot answers.
	const seed = (
		dimension: Dimension,
		uuid: string,
		name: string,
		units?: string,
	) =>
		client.setQueryData(nameQuery(api, slug(), dimension, uuid).queryKey, {
			name,
			...(units === undefined ? {} : { units }),
		});
	const pick = (option: Option) => {
		const benchmark = result().data?.find(({ uuid }) => uuid === option.value);
		if (!benchmark) {
			return;
		}
		const { branch, testbed } = props.report;
		seed("branches", branch.uuid, props.branch);
		seed("testbeds", testbed.uuid, testbed.name);
		seed("benchmarks", benchmark.uuid, benchmark.name);
		for (const measure of benchmark.measures) {
			seed("measures", measure.uuid, measure.name, measure.units);
		}
		props.onQuery(reportBenchmarkQuery(props.report, benchmark, Date.now()));
	};
	return (
		<div id={props.id} class="ex-rpick">
			<SearchMenu
				label={`Benchmarks in ${props.name}`}
				search="Search benchmarks"
				searchName={`Search benchmarks in ${props.name}`}
				anchor={props.anchor}
				options={(text) => () =>
					choicesOf(benchmarkOptions(result().data ?? [], text()))
				}
				empty={
					result().data
						? "No benchmarks"
						: result().isError
							? "This list did not load"
							: ""
				}
				onClose={props.onClose}
				onPick={pick}
			/>
		</div>
	);
};
