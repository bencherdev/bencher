import { type Accessor, For, createMemo } from "solid-js";
import { PerfTab } from "../../../../../config/types";
import { toCapitalized } from "../../../../../config/util";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonReport,
	JsonTestbed,
} from "../../../../../types/bencher";
import Pagination, { PaginationSize } from "../../../../site/Pagination";
import Tab from "./Tab";
import type { FieldHandler } from "../../../../field/Field";

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
];

export interface Props {
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	measures: Accessor<string[]>;
	tab: Accessor<PerfTab>;
	handleTab: (tab: PerfTab) => void;
	// Tabs
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	// Per page
	reports_per_page: Accessor<number>;
	branches_per_page: Accessor<number>;
	testbeds_per_page: Accessor<number>;
	benchmarks_per_page: Accessor<number>;
	// Page
	reports_page: Accessor<number>;
	branches_page: Accessor<number>;
	testbeds_page: Accessor<number>;
	benchmarks_page: Accessor<number>;
	// Search
	reports_start_date: Accessor<undefined | string>;
	reports_end_date: Accessor<undefined | string>;
	branches_search: Accessor<undefined | string>;
	testbeds_search: Accessor<undefined | string>;
	benchmarks_search: Accessor<undefined | string>;
	// Handle checked
	handleReportChecked: (
		index: number,
		measure_uuid: undefined | string,
	) => void;
	handleBranchChecked: (index: number) => void;
	handleTestbedChecked: (index: number) => void;
	handleBenchmarkChecked: (index: number) => void;
	// Handle page
	handleReportsPage: (reports_page: number) => void;
	handleBranchesPage: (branches_page: number) => void;
	handleTestbedsPage: (testbeds_page: number) => void;
	handleBenchmarksPage: (benchmarks_page: number) => void;
	// Handle search
	handleReportsStartTime: (start_time: string) => void;
	handleReportsEndTime: (end_time: string) => void;
	handleBranchesSearch: (branches_search: string) => void;
	handleTestbedsSearch: (testbeds_search: string) => void;
	handleBenchmarksSearch: (benchmarks_search: string) => void;
}

const PlotTab = (props: Props) => {
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
			default:
				return console.error("No handle for tab", props.tab(), page);
		}
	};

	const handleChecked = (index: number, uuid?: string) => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.handleReportChecked(index, uuid);
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

	// Pagination
	const reportsLength = createMemo(() => props.reports_tab.length);
	const branchesLength = createMemo(() => props.branches_tab.length);
	const testbedsLength = createMemo(() => props.testbeds_tab.length);
	const benchmarksLength = createMemo(() => props.benchmarks_tab.length);
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
			default:
				return "";
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
			default:
				return console.error("No handle for tab", props.tab(), search);
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
								// biome-ignore lint/a11y/useValidAnchor: stateful anchor
								onClick={() => props.handleTab(tab)}
							>
								{toCapitalized(tab)}
							</a>
							{index() === 0 && (
								// biome-ignore lint/a11y/useValidAnchor: disabled anchor
								<a style="pointer-events: none; cursor: default; color: #fdb07e;">
									||
								</a>
							)}
						</>
					)}
				</For>
			</div>
			<Tab
				project_slug={props.project_slug}
				isConsole={props.isConsole}
				measures={props.measures}
				reports_tab={props.reports_tab}
				branches_tab={props.branches_tab}
				testbeds_tab={props.testbeds_tab}
				benchmarks_tab={props.benchmarks_tab}
				tab={props.tab}
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

export default PlotTab;
