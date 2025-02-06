import {
	type Accessor,
	Match,
	type Resource,
	Switch,
	createMemo,
} from "solid-js";
import { PerfTab } from "../../../../../config/types";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonPlot,
	JsonReport,
	JsonTestbed,
} from "../../../../../types/bencher";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import type { FieldHandler } from "../../../../field/Field";
import type { Theme } from "../../../../navbar/theme/theme";
import { DEFAULT_PAGE } from "../../PerfPanel";
import DimensionsTab from "./DimensionTab";
import type { TabList } from "./PlotTab";
import PlotsTab from "./PlotsTab";
import ReportsTab from "./ReportsTab";

const Tab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	// Tab UUIDs
	report: Accessor<undefined | string>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	measures: Accessor<string[]>;
	// Selected
	branches_selected: Resource<JsonBranch[]>;
	testbeds_selected: Resource<JsonTestbed[]>;
	benchmarks_selected: Resource<JsonBenchmark[]>;
	// Tabs
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	plots_tab: TabList<JsonPlot>;
	selected: Accessor<number>;
	loading: Accessor<boolean>;
	tab: Accessor<PerfTab>;
	per_page: Accessor<number>;
	page: Accessor<number>;
	search: Accessor<undefined | string>;
	width: Accessor<number>;
	reports_start_date: Accessor<undefined | string>;
	reports_end_date: Accessor<undefined | string>;
	handleBranchSelected: (uuid: string) => void;
	handleTestbedSelected: (uuid: string) => void;
	handleBenchmarkSelected: (uuid: string) => void;
	handlePage: (page: number) => void;
	handleChecked: (index?: number, slug?: string) => void;
	handleSearch: FieldHandler;
	handleReportsStartTime: (start_time: string) => void;
	handleReportsEndTime: (end_time: string) => void;
}) => {
	const tabUuids = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.branches();
			case PerfTab.TESTBEDS:
				return props.testbeds();
			case PerfTab.BENCHMARKS:
				return props.benchmarks();
			case PerfTab.REPORTS:
			case PerfTab.PLOTS:
				return [];
		}
	});

	const selected = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				return props.branches_selected();
			case PerfTab.TESTBEDS:
				return props.testbeds_selected();
			case PerfTab.BENCHMARKS:
				return props.benchmarks_selected();
			default:
				return [];
		}
	});

	const handleSelected = (uuid: string) => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
				props.handleBranchSelected(uuid);
				break;
			case PerfTab.TESTBEDS:
				props.handleTestbedSelected(uuid);
				break;
			case PerfTab.BENCHMARKS:
				props.handleBenchmarkSelected(uuid);
				break;
		}
	};

	const tabList = createMemo(() => {
		switch (props.tab()) {
			case PerfTab.REPORTS:
				return props.reports_tab;
			case PerfTab.BRANCHES:
				return props.branches_tab;
			case PerfTab.TESTBEDS:
				return props.testbeds_tab;
			case PerfTab.BENCHMARKS:
				return props.benchmarks_tab;
			case PerfTab.PLOTS:
				return props.plots_tab;
			default:
				return [];
		}
	});

	return (
		<Switch
			fallback={
				<div class="box">
					<p>🐰 No {props.tab()} found</p>
				</div>
			}
		>
			<Match
				when={
					props.isConsole &&
					!props.loading() &&
					tabList().length === 0 &&
					props.page() === DEFAULT_PAGE &&
					!props.search() &&
					!props.reports_start_date() &&
					!props.reports_end_date()
				}
			>
				<div class="box">
					<div class="columns is-centered">
						<div class="column is-5">
							<AddButton project_slug={props.project_slug} tab={props.tab} />
						</div>
					</div>
				</div>
			</Match>
			<Match
				when={
					props.tab() === PerfTab.REPORTS &&
					(props.loading() ||
						typeof props.reports_start_date() === "string" ||
						typeof props.reports_end_date() === "string" ||
						tabList().length > 0)
				}
			>
				<ReportsTab
					project_slug={props.project_slug}
					theme={props.theme}
					isConsole={props.isConsole}
					loading={props.loading}
					report={props.report}
					measures={props.measures}
					tab={props.tab}
					tabList={tabList as Accessor<TabList<JsonReport>>}
					page={props.page}
					per_page={props.per_page}
					start_date={props.reports_start_date}
					end_date={props.reports_end_date}
					width={props.width}
					handleChecked={props.handleChecked}
					handleStartTime={props.handleReportsStartTime}
					handleEndTime={props.handleReportsEndTime}
				/>
			</Match>
			<Match
				when={
					props.tab() === PerfTab.PLOTS &&
					(props.loading() ||
						typeof props.search() === "string" ||
						tabList().length > 0)
				}
			>
				<PlotsTab
					project_slug={props.project_slug}
					theme={props.theme}
					isConsole={props.isConsole}
					loading={props.loading}
					tab={props.tab}
					tabList={tabList as Accessor<TabList<JsonPlot>>}
					per_page={props.per_page}
					search={props.search}
					handleChecked={props.handleChecked}
					handleSearch={props.handleSearch}
				/>
			</Match>
			<Match
				when={
					props.tab() !== PerfTab.REPORTS &&
					(props.loading() ||
						typeof props.search() === "string" ||
						tabList().length > 0)
				}
			>
				<DimensionsTab
					project_slug={props.project_slug}
					theme={props.theme}
					isConsole={props.isConsole}
					loading={props.loading}
					tab={props.tab}
					tabUuids={tabUuids}
					selected={selected}
					tabList={
						tabList as Accessor<
							TabList<JsonBranch | JsonTestbed | JsonBenchmark>
						>
					}
					per_page={props.per_page}
					search={props.search}
					handleSelected={handleSelected}
					handleChecked={props.handleChecked}
					handleSearch={props.handleSearch}
				/>
			</Match>
		</Switch>
	);
};

const AddButton = (props: {
	project_slug: Accessor<undefined | string>;
	tab: Accessor<PerfTab>;
}) => {
	const getHref = () => {
		switch (props.tab()) {
			case PerfTab.BRANCHES:
			case PerfTab.TESTBEDS:
				return `/console/projects/${props.project_slug()}/${props.tab()}/add?${BACK_PARAM}=${encodePath()}`;
			case PerfTab.REPORTS:
			case PerfTab.BENCHMARKS:
				return `https://bencher.dev/docs/how-to/track-benchmarks?${BACK_PARAM}=${encodePath()}`;
			case PerfTab.PLOTS:
				return `/console/projects/${props.project_slug()}/${props.tab()}?${BACK_PARAM}=${encodePath()}`;
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
			case PerfTab.PLOTS:
				return "Pin a Plot";
			default:
				return "Unknown Tab";
		}
	};

	return (
		<a
			class="button is-primary is-fullwidth"
			title={getText()}
			href={getHref()}
		>
			{getText()}
		</a>
	);
};

export default Tab;
