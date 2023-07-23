import { Link } from "solid-app-router";
import { For, Match, Switch } from "solid-js";
import { PerfTab } from "../../../config/types";
import { date_time_fmt, toCapitalized } from "../../../config/util";
import { DEFAULT_PAGE } from "../PerfPanel";
import Pagination, { PaginationSize } from "../../../../site/Pagination";

const perf_tabs = [
	PerfTab.REPORTS,
	PerfTab.BRANCHES,
	PerfTab.TESTBEDS,
	PerfTab.BENCHMARKS,
];

const PlotTab = (props) => {
	const getTab = () => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_tab;
			case PerfTab.BRANCHES:
				return props.branches_tab;
			case PerfTab.TESTBEDS:
				return props.testbeds_tab;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_tab;
			default:
				return [];
		}
	};

	const getPage = () => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_page;
			case PerfTab.BRANCHES:
				return props.branches_page;
			case PerfTab.TESTBEDS:
				return props.testbeds_page;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_page;
			default:
				return 1;
		}
	};

	const getHandlePage = () => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.handleReportsPage;
			case PerfTab.BRANCHES:
				return props.handleBranchesPage;
			case PerfTab.TESTBEDS:
				return props.handleTestbedsPage;
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarksPage;
			default:
				return (page: number) =>
					console.error("No handle for tab", props.tab(), page);
		}
	};

	const handleChecked = (index: number, slug: void | string) => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.handleReportChecked(index, slug);
			case PerfTab.BRANCHES:
				return props.handleBranchChecked(index);
			case PerfTab.TESTBEDS:
				return props.handleTestbedChecked(index);
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarkChecked(index);
			default:
				return [];
		}
	};

	return (
		<>
			<div class="panel-tabs">
				<For each={perf_tabs}>
					{(tab, index) => (
						<>
							<a
								class={props.tab() === tab ? "is-active" : ""}
								title={`View ${toCapitalized(tab)}`}
								onClick={() => props.handleTab(tab)}
							>
								{toCapitalized(tab)}
							</a>
							{index() === 0 && (
								<a style="pointer-events:none;color:#fdb07e;">||</a>
							)}
						</>
					)}
				</For>
			</div>
			<Tab
				project_slug={props.project_slug}
				is_console={props.is_console}
				tab={props.tab}
				getTab={getTab}
				getPage={getPage}
				getHandlePage={getHandlePage}
				handleChecked={handleChecked}
			/>
			<div class="panel-block">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-11">
							<br />
							<Pagination
								size={PaginationSize.SMALL}
								data_len={getTab()?.length}
								per_page={props.per_page()}
								page={getPage()()}
								handlePage={getHandlePage()}
							/>
							<br />
						</div>
					</div>
				</div>
			</div>
		</>
	);
};

const Tab = (props: {
	project_slug: string;
	is_console: boolean;
	tab: () => PerfTab;
	getTab: () => any[];
	getPage: () => number;
	getHandlePage: () => (page: number) => void;
	handleChecked: (index: number, slug: void | string) => void;
}) => {
	return (
		<Switch
			fallback={
				<div class="box">
					<p>No {props.tab()} found</p>
				</div>
			}
		>
			<Match
				when={
					props.is_console &&
					props.getTab().length === 0 &&
					props.getPage() === DEFAULT_PAGE
				}
			>
				<div class="box">
					<AddButton project_slug={props.project_slug} tab={props.tab} />
				</div>
			</Match>
			<Match
				when={props.getTab().length === 0 && props.getPage() !== DEFAULT_PAGE}
			>
				<div class="box">
					<BackButton
						tab={props.tab}
						page={props.getPage()}
						handlePage={props.getHandlePage()}
					/>
				</div>
			</Match>
			<Match
				when={props.tab() === PerfTab.REPORTS && props.getTab().length > 0}
			>
				<For each={props.getTab()}>
					{(report, index) => (
						<For each={report.resource?.results?.[0]}>
							{(result, _index) => (
								<a
									class="panel-block"
									title={`View Report from ${date_time_fmt(
										report.resource?.start_time,
									)}`}
									onClick={(_e) =>
										// Send the Metric Kind slug instead of the Report UUID
										props.handleChecked(index(), result.metric_kind?.slug)
									}
								>
									<div class="columns is-vcentered is-mobile">
										<div class="column is-narrow">
											<input type="radio" checked={report.checked} />
										</div>
										<div class="column">
											<small style="overflow-wrap:anywhere;">
												{date_time_fmt(report.resource?.start_time)}
											</small>
											<ReportDimension
												icon="fas fa-shapes"
												name={result.metric_kind?.name}
											/>
											<ReportDimension
												icon="fas fa-code-branch"
												name={report.resource?.branch?.name}
											/>
											<ReportDimension
												icon="fas fa-server"
												name={report.resource?.testbed?.name}
											/>
										</div>
									</div>
								</a>
							)}
						</For>
					)}
				</For>
			</Match>
			<Match
				when={props.tab() !== PerfTab.REPORTS && props.getTab().length > 0}
			>
				<For each={props.getTab()}>
					{(dimension, index) => (
						<a
							class="panel-block"
							title={`${dimension.checked ? "Remove" : "Add"} ${
								dimension.resource?.name
							}`}
							onClick={(_e) => props.handleChecked(index())}
						>
							<div class="columns is-vcentered is-mobile">
								<div class="column is-narrow">
									<input type="checkbox" checked={dimension.checked} />
								</div>
								<div class="column">
									<small style="overflow-wrap:anywhere;">
										{dimension.resource?.name}
									</small>
								</div>
							</div>
						</a>
					)}
				</For>
			</Match>
		</Switch>
	);
};

const AddButton = (props) => {
	const getHref = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
			case PerfTab.TESTBEDS:
				return `/console/projects/${props.project_slug}/${props.tab()}/add`;
			case PerfTab.REPORTS:
			case PerfTab.BENCHMARKS:
				return "/docs/how-to/track-benchmarks";
			default:
				return "#";
		}
	};

	const getText = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return "Add a Branch";
			case PerfTab.TESTBEDS:
				return "Add a Testbed";
			case PerfTab.REPORTS:
			case PerfTab.BENCHMARKS:
				return "Track Your Benchmarks";
			default:
				return "Unknown Tab";
		}
	};

	return (
		<Link
			class="button is-primary is-fullwidth"
			title={getText()}
			href={getHref()}
		>
			{getText()}
		</Link>
	);
};

const BackButton = (props) => {
	return (
		<button
			class="button is-primary is-fullwidth"
			title="Go back to the previous page"
			onClick={(e) => {
				e.preventDefault();
				props.handlePage(props.page() - 1);
			}}
		>
			That's all the {props.tab()}. Go back.
		</button>
	);
};

const ReportDimension = (props: { icon: string; name: string }) => {
	return (
		<div>
			<span class="icon">
				<i class={props.icon} aria-hidden="true" />
			</span>
			<small style="overflow-wrap:anywhere;">{props.name}</small>
		</div>
	);
};

export default PlotTab;
