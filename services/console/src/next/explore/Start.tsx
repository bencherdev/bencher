import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
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
	createSignal,
	createUniqueId,
} from "solid-js";
import {
	BoundaryLimit,
	type JsonAlert,
	type JsonPlot,
	type JsonReport,
} from "../../types/bencher";
import { projectPath, reportPath } from "../paths";
import { formatDelta, formatWhen } from "../plot/format";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import type { ExploreQuery } from "../query/query";
import { exploreSearch } from "../query/selection";
import { type Option, SearchMenu } from "./Box";
import { plotSearch } from "./pinned";
import {
	type Dimension,
	OPTIONS,
	START,
	nameQuery,
	startQueries,
} from "./queries";
import {
	alertLine,
	benchmarkOptions,
	reportBenchmarkQuery,
	reportBenchmarksQuery,
} from "./start";
import { rollingName } from "./window";

/** Three short lists blank Explore starts from, asked for in one round; a report opens to its benchmarks in place. */
const Start = (props: { onQuery: (query: ExploreQuery) => void }) => {
	const { api, slug } = useProject();
	const alerts = useQueryResult(() => startQueries(api, slug()).alerts);
	const reports = useQueryResult(() => startQueries(api, slug()).reports);
	const plots = useQueryResult(() => startQueries(api, slug()).plots);
	const explore = () => projectPath(slug(), "explore");
	const [open, setOpen] = createSignal<string>();
	return (
		<section class="ex-startcard" aria-labelledby="ex-start">
			<h2 class="seclabel" id="ex-start">
				Start from
			</h2>
			<div class="ex-cols">
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
								<b class="ex-ellip">{plot.title ?? "Untitled plot"}</b>
								<span class="ex-sub">{rollingName(plot.window)}</span>
							</span>
						</a>
					)}
				</List>
				<List
					id="ex-reports"
					title="Latest reports"
					all={{ href: projectPath(slug(), "runs"), text: "All runs" }}
					result={reports()}
					rows={START.reports}
					empty="No reports"
				>
					{(report: JsonReport) => (
						<ReportRow
							report={report}
							open={open() === report.uuid}
							onOpen={(next) => setOpen(next ? report.uuid : undefined)}
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
								: formatDelta(
										alert.value,
										alert.boundary.baseline,
										alert.limit === BoundaryLimit.Lower ? "lower" : "upper",
									);
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
											alert: `${branch.name} · ${testbed.name}`,
											uuid: alert.uuid,
										}),
									)
								}
							>
								<span class="ex-alertdot" aria-hidden="true" />
								<span class="grow">
									<span class="ex-sline">
										<b>{alert.benchmark.name}</b>
										<For each={tags}>
											{(tag) => <span class="ex-tag">{tag}</span>}
										</For>
										<span class="muted">{measure.name}</span>
									</span>
									<span class="ex-sub">
										{branch.name} · {testbed.name}
										{delta ? ` · ${delta.text}` : ""}
									</span>
								</span>
							</a>
						);
					}}
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

/** A report that opens, anywhere on its row, to search the benchmarks it ran; its hash opens the report. */
const ReportRow = (props: {
	report: JsonReport;
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
	const hash = () => props.report.branch.head.version?.hash?.slice(0, 7);
	const panel = createUniqueId();
	let toggle: HTMLButtonElement | undefined;
	return (
		<div>
			<div class="ex-rhead">
				<button
					ref={toggle}
					type="button"
					class="ex-rbtn"
					aria-expanded={props.open}
					aria-controls={panel}
					onPointerEnter={prefetch}
					onFocus={prefetch}
					onClick={() => props.onOpen(!props.open)}
				>
					<span class="grow">
						<span class="ex-sline">
							<b>{props.report.branch.name}</b>
							<span class="muted">· {props.report.testbed.name}</span>
						</span>
						<span class="ex-sub">
							{formatWhen(Date.parse(props.report.start_time))}
						</span>
					</span>
				</button>
				<a class="ex-rhash" href={reportPath(slug(), props.report.uuid)}>
					{hash() ?? "report"}
				</a>
				<Icon name="chevron-down" class="ex-rcaret" />
			</div>
			<Show when={props.open}>
				<Picker
					id={panel}
					report={props.report}
					name={
						hash()
							? `report ${hash()}`
							: `the report on ${props.report.branch.name}`
					}
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
	report: JsonReport;
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
		seed("branches", branch.uuid, branch.name);
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
					benchmarkOptions(result().data ?? [], text(), OPTIONS)
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
