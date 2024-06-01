import { type Accessor, Match, Switch, createMemo } from "solid-js";
import { PerfTab } from "../../../../../config/types";
import type {
	JsonBenchmark,
	JsonBranch,
	JsonPlot,
	JsonReport,
	JsonTestbed,
} from "../../../../../types/bencher";
import { DEFAULT_PAGE } from "../../PerfPanel";
import type { TabList } from "./PlotTab";
import ReportsTab from "./ReportsTab";
import DimensionsTab from "./DimensionTab";
import type { FieldHandler } from "../../../../field/Field";
import { BACK_PARAM, encodePath } from "../../../../../util/url";
import type { Theme } from "../../../../navbar/theme/theme";
import PlotsTab from "./PlotsTab";

const Tab = (props: {
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	measures: Accessor<string[]>;
	// Tabs
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	plots_tab: TabList<JsonPlot>;
	loading: Accessor<boolean>;
	tab: Accessor<PerfTab>;
	per_page: Accessor<number>;
	page: Accessor<number>;
	search: Accessor<undefined | string>;
	reports_start_date: Accessor<undefined | string>;
	reports_end_date: Accessor<undefined | string>;
	handlePage: (page: number) => void;
	handleChecked: (index: number, slug?: string) => void;
	handleSearch: FieldHandler;
	handleReportsStartTime: (start_time: string) => void;
	handleReportsEndTime: (end_time: string) => void;
}) => {
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
					!props.loading() &&
					tabList().length === 0 &&
					props.page() !== DEFAULT_PAGE
				}
			>
				<div class="box">
					<div class="columns is-centered">
						<div class="column is-5">
							<BackButton
								tab={props.tab}
								page={props.page}
								handlePage={props.handlePage}
							/>
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
					measures={props.measures}
					tab={props.tab}
					tabList={tabList as Accessor<TabList<JsonReport>>}
					per_page={props.per_page}
					start_date={props.reports_start_date}
					end_date={props.reports_end_date}
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
					tabList={
						tabList as Accessor<
							TabList<JsonBranch | JsonTestbed | JsonBenchmark>
						>
					}
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
					tabList={
						tabList as Accessor<
							TabList<JsonBranch | JsonTestbed | JsonBenchmark>
						>
					}
					per_page={props.per_page}
					search={props.search}
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

const BackButton = (props: {
	tab: Accessor<PerfTab>;
	page: Accessor<number>;
	handlePage: (page: number) => void;
}) => {
	return (
		<button
			class="button is-primary is-fullwidth"
			type="button"
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

export default Tab;
