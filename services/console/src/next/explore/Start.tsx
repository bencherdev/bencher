import Skeleton from "@bencherdev/ui/Skeleton";
import type { QueryObserverResult } from "@tanstack/solid-query";
import { For, type JSX, Match, Switch } from "solid-js";
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
import { exploreSearch } from "../query/selection";
import { plotSearch } from "./pinned";
import { START, startQueries } from "./queries";
import { alertLine } from "./start";
import { rollingName } from "./window";

/** Three short lists blank Explore starts from in one click, asked for in one round. */
const Start = () => {
	const { api, slug } = useProject();
	const alerts = useQueryResult(() => startQueries(api, slug()).alerts);
	const reports = useQueryResult(() => startQueries(api, slug()).reports);
	const plots = useQueryResult(() => startQueries(api, slug()).plots);
	const explore = () => projectPath(slug(), "explore");
	return (
		<section class="ex-startcard" aria-labelledby="ex-start">
			<h2 class="seclabel" id="ex-start">
				Or start from
			</h2>
			<div class="ex-cols">
				<List
					id="ex-alerts"
					title="Alerting now"
					all={{ href: projectPath(slug(), "alerts"), text: "All alerts" }}
					result={alerts()}
					rows={START.alerts}
					empty="No alerts are active."
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
										}),
									)
								}
								aria-label={`${[alert.benchmark.name, ...tags, measure.name].join(" ")} on ${branch.name}, ${testbed.name}${delta ? `, ${delta.text}` : ""}: open in Explore`}
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
				<List
					id="ex-reports"
					title="Latest reports"
					all={{ href: projectPath(slug(), "reports"), text: "All reports" }}
					result={reports()}
					rows={START.reports}
					empty="No reports yet."
				>
					{(report: JsonReport) => (
						<a class="ex-srow" href={reportPath(slug(), report.uuid)}>
							<span class="grow">
								<span class="ex-sline">
									<b>{report.branch.name}</b>
									<span class="muted">· {report.testbed.name}</span>
								</span>
								<span class="ex-sub">
									{formatWhen(Date.parse(report.start_time))}
								</span>
							</span>
						</a>
					)}
				</List>
				<List
					id="ex-plots"
					title="Pinned plots"
					all={{ href: projectPath(slug(), "plots"), text: "All plots" }}
					result={plots()}
					rows={START.plots}
					empty="No plots are pinned yet."
				>
					{(plot: JsonPlot) => (
						<a class="ex-srow" href={`${explore()}${plotSearch(plot)}`}>
							<span class="grow">
								<b class="ex-ellip">{plot.title ?? "Untitled plot"}</b>
								<span class="ex-sub">{rollingName(plot.window)} rolling</span>
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
				<p class="ex-none">This list did not load.</p>
			</Match>
		</Switch>
	</section>
);
