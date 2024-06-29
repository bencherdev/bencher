import { type Accessor, For, createMemo, type Resource } from "solid-js";
import { PerfTab } from "../../../../../config/types";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonPlot,
	JsonReport,
	JsonTestbed,
} from "../../../../../types/bencher";
import Pagination, { PaginationSize } from "../../../../site/Pagination";
import Tab from "./Tab";
import type { FieldHandler } from "../../../../field/Field";
import type { Theme } from "../../../../navbar/theme/theme";

export type TabList<T> = TabElement<T>[];

export interface TabElement<T> {
	resource: T;
	checked: boolean;
}

const perf_tabs = [
	PerfTab.REPORTS,
	PerfTab.BRANCHES,
	PerfTab.TESTBEDS,
	PerfTab.BENCHMARKS,
	PerfTab.PLOTS,
];

export interface Props {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	tab: Accessor<PerfTab>;
	handleTab: (tab: PerfTab) => void;
	// Tab UUID
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	measures: Accessor<string[]>;
	// Tab data
	reports_data: Resource<JsonReport>;
	branches_data: Resource<JsonBranch>;
	testbeds_data: Resource<JsonTestbed>;
	benchmarks_data: Resource<JsonBenchmark>;
	plots_data: Resource<JsonPlot>;
	// Tabs
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	plots_tab: TabList<JsonPlot>;
	// Per page
	reports_per_page: Accessor<number>;
	branches_per_page: Accessor<number>;
	testbeds_per_page: Accessor<number>;
	benchmarks_per_page: Accessor<number>;
	plots_per_page: Accessor<number>;
	// Page
	reports_page: Accessor<number>;
	branches_page: Accessor<number>;
	testbeds_page: Accessor<number>;
	benchmarks_page: Accessor<number>;
	plots_page: Accessor<number>;
	// Total Count
	reports_total_count: Accessor<number>;
	branches_total_count: Accessor<number>;
	testbeds_total_count: Accessor<number>;
	benchmarks_total_count: Accessor<number>;
	plots_total_count: Accessor<number>;
	// Search
	reports_start_date: Accessor<undefined | string>;
	reports_end_date: Accessor<undefined | string>;
	branches_search: Accessor<undefined | string>;
	testbeds_search: Accessor<undefined | string>;
	benchmarks_search: Accessor<undefined | string>;
	plots_search: Accessor<undefined | string>;
	// Handle checked
	handleReportChecked: (
		index: number,
		measure_uuid: undefined | string,
	) => void;
	handleBranchChecked: (index: undefined | number) => void;
	handleTestbedChecked: (index: undefined | number) => void;
	handleBenchmarkChecked: (index: undefined | number) => void;
	handlePlotChecked: (index: number) => void;
	// Handle page
	handleReportsPage: (reports_page: number) => void;
	handleBranchesPage: (branches_page: number) => void;
	handleTestbedsPage: (testbeds_page: number) => void;
	handleBenchmarksPage: (benchmarks_page: number) => void;
	handlePlotsPage: (plots_page: number) => void;
	// Handle search
	handleReportsStartTime: (start_time: string) => void;
	handleReportsEndTime: (end_time: string) => void;
	handleBranchesSearch: (branches_search: string) => void;
	handleTestbedsSearch: (testbeds_search: string) => void;
	handleBenchmarksSearch: (benchmarks_search: string) => void;
	handlePlotsSearch: (plots_search: string) => void;
}

const PlotTab = (props: Props) => {
	const loading = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_data.loading;
			case PerfTab.BRANCHES:
				return props.branches_data.loading;
			case PerfTab.TESTBEDS:
				return props.testbeds_data.loading;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_data.loading;
			case PerfTab.PLOTS:
				return props.plots_data.loading;
			default:
				return false;
		}
	});

	const page = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_page();
			case PerfTab.BRANCHES:
				return props.branches_page();
			case PerfTab.TESTBEDS:
				return props.testbeds_page();
			case PerfTab.BENCHMARKS:
				return props.benchmarks_page();
			case PerfTab.PLOTS:
				return props.plots_page();
			default:
				return 1;
		}
	});

	const handlePage = (page: number) => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.handleReportsPage(page);
			case PerfTab.BRANCHES:
				return props.handleBranchesPage(page);
			case PerfTab.TESTBEDS:
				return props.handleTestbedsPage(page);
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarksPage(page);
			case PerfTab.PLOTS:
				return props.handlePlotsPage(page);
			default:
				return console.error("No handle for tab", props.tab(), page);
		}
	};

	const totalCount = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_total_count();
			case PerfTab.BRANCHES:
				return props.branches_total_count();
			case PerfTab.TESTBEDS:
				return props.testbeds_total_count();
			case PerfTab.BENCHMARKS:
				return props.benchmarks_total_count();
			case PerfTab.PLOTS:
				return props.plots_total_count();
			default:
				return 0;
		}
	});

	const handleChecked = (index?: number, uuid?: string) => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.handleReportChecked(index, uuid);
			case PerfTab.BRANCHES:
				return props.handleBranchChecked(index);
			case PerfTab.TESTBEDS:
				return props.handleTestbedChecked(index);
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarkChecked(index);
			case PerfTab.PLOTS:
				return props.handlePlotChecked(index);
			default:
				return [];
		}
	};

	// Pagination
	const reportsLength = createMemo(() => props.reports_tab.length);
	const branchesLength = createMemo(() => props.branches_tab.length);
	const testbedsLength = createMemo(() => props.testbeds_tab.length);
	const benchmarksLength = createMemo(() => props.benchmarks_tab.length);
	const plotsLength = createMemo(() => props.plots_tab.length);
	const tabLength = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return reportsLength();
			case PerfTab.BRANCHES:
				return branchesLength();
			case PerfTab.TESTBEDS:
				return testbedsLength();
			case PerfTab.BENCHMARKS:
				return benchmarksLength();
			case PerfTab.PLOTS:
				return plotsLength();
			default:
				return 0;
		}
	});

	const perPage = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_per_page();
			case PerfTab.BRANCHES:
				return props.branches_per_page();
			case PerfTab.TESTBEDS:
				return props.testbeds_per_page();
			case PerfTab.BENCHMARKS:
				return props.benchmarks_per_page();
			case PerfTab.PLOTS:
				return props.plots_per_page();
			default:
				return 8;
		}
	});

	const search = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.branches_search();
			case PerfTab.TESTBEDS:
				return props.testbeds_search();
			case PerfTab.BENCHMARKS:
				return props.benchmarks_search();
			case PerfTab.PLOTS:
				return props.plots_search();
			default:
				return undefined;
		}
	});

	const handleSearch = (_key: string, search: string, _valid: boolean) => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.handleBranchesSearch(search);
			case PerfTab.TESTBEDS:
				return props.handleTestbedsSearch(search);
			case PerfTab.BENCHMARKS:
				return props.handleBenchmarksSearch(search);
			case PerfTab.PLOTS:
				return props.handlePlotsSearch(search);
			default:
				return console.error("No handle for tab", props.tab(), search);
		}
	};

	return (
		<>
			<div class="panel-tabs">
				<nav class="level">
					<div class="level-item">
						<div class="level is-mobile">
							<div class="level-item">
								<TabName
									tab={props.tab}
									perfTab={PerfTab.REPORTS}
									handleTab={props.handleTab}
								/>
							</div>
							<div class="level-item">
								{/* biome-ignore lint/a11y/useValidAnchor: <explanation> */}
								<a style="pointer-events: none; cursor: default; color: #fdb07e;">
									||
								</a>
							</div>
						</div>
					</div>
					<div class="level-item">
						<div class="level is-mobile">
							<div class="level-item">
								<TabName
									tab={props.tab}
									perfTab={PerfTab.BRANCHES}
									handleTab={props.handleTab}
								/>
							</div>
							<div class="level-item">
								<TabName
									tab={props.tab}
									perfTab={PerfTab.TESTBEDS}
									handleTab={props.handleTab}
								/>
							</div>
							<div class="level-item">
								<TabName
									tab={props.tab}
									perfTab={PerfTab.BENCHMARKS}
									handleTab={props.handleTab}
								/>
							</div>
						</div>
					</div>
					<div class="level-item">
						<div class="level is-mobile">
							<div class="level-item">
								{/* biome-ignore lint/a11y/useValidAnchor: <explanation> */}
								<a style="pointer-events: none; cursor: default; color: #fdb07e;">
									||
								</a>
							</div>
							<div class="level-item">
								<TabName
									tab={props.tab}
									perfTab={PerfTab.PLOTS}
									handleTab={props.handleTab}
								/>
							</div>
						</div>
					</div>
				</nav>
			</div>
			<Tab
				project_slug={props.project_slug}
				theme={props.theme}
				isConsole={props.isConsole}
				branches={props.branches}
				testbeds={props.testbeds}
				benchmarks={props.benchmarks}
				measures={props.measures}
				reports_tab={props.reports_tab}
				branches_tab={props.branches_tab}
				testbeds_tab={props.testbeds_tab}
				benchmarks_tab={props.benchmarks_tab}
				plots_tab={props.plots_tab}
				loading={loading}
				tab={props.tab}
				per_page={perPage}
				page={page}
				search={search}
				reports_start_date={props.reports_start_date}
				reports_end_date={props.reports_end_date}
				handlePage={handlePage}
				handleChecked={handleChecked}
				handleSearch={handleSearch as FieldHandler}
				handleReportsStartTime={props.handleReportsStartTime}
				handleReportsEndTime={props.handleReportsEndTime}
			/>
			<div class="panel-block">
				<div class="container">
					<div class="columns is-centered">
						<div class="column is-11">
							<br />
							<Pagination
								size={PaginationSize.SMALL}
								data_len={tabLength}
								per_page={perPage}
								page={page}
								total_count={totalCount}
								handlePage={handlePage}
							/>
							<br />
						</div>
					</div>
				</div>
			</div>
		</>
	);
};

const TabName = (props: {
	tab: Accessor<PerfTab>;
	perfTab: PerfTab;
	handleTab: (tab: PerfTab) => void;
}) => {
	return (
		// biome-ignore lint/a11y/useValidAnchor: <explanation>
		<a
			class={props.tab() === props.perfTab ? "is-active" : ""}
			title={`View ${formatTab(props.perfTab)}`}
			onMouseDown={() => props.handleTab(props.perfTab)}
		>
			{formatTab(props.perfTab)}
		</a>
	);
};

const formatTab = (tab: PerfTab) => {
	switch (tab) {
		case PerfTab.REPORTS:
			return "Reports";
		case PerfTab.BRANCHES:
			return "Branches";
		case PerfTab.TESTBEDS:
			return "Testbeds";
		case PerfTab.BENCHMARKS:
			return "Benchmarks";
		case PerfTab.PLOTS:
			return "Pinned";
	}
};

export default PlotTab;
