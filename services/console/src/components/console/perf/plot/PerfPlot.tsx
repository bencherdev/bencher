import {
	type Accessor,
	Match,
	type Resource,
	Show,
	Switch,
	createMemo,
} from "solid-js";
import type { PerfTab } from "../../../../config/types";
import type {
	JsonAuthUser,
	JsonBenchmark,
	JsonBranch,
	JsonPerf,
	JsonPlot,
	JsonProject,
	JsonReport,
	JsonTestbed,
	XAxis,
} from "../../../../types/bencher";
import Plot from "./Plot";
import PlotHeader from "./PlotHeader";
import PlotInit from "./PlotInit";
import PlotTab, { type TabList } from "./tab/PlotTab";
import { themeColor, type Theme } from "../../../navbar/theme/theme";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	project_slug: Accessor<undefined | string>;
	theme: Accessor<Theme>;
	isConsole: boolean;
	isEmbed: boolean;
	isPlotInit: Accessor<boolean>;
	report: Accessor<undefined | string>;
	measures: Accessor<string[]>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	refresh: () => void;
	perfData: Resource<JsonPerf>;
	tab: Accessor<PerfTab>;
	key: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	reports_data: Resource<JsonReport>;
	branches_data: Resource<JsonBranch>;
	testbeds_data: Resource<JsonTestbed>;
	benchmarks_data: Resource<JsonBenchmark>;
	plots_data: Resource<JsonPlot>;
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	plots_tab: TabList<JsonPlot>;
	reports_per_page: Accessor<number>;
	branches_per_page: Accessor<number>;
	testbeds_per_page: Accessor<number>;
	benchmarks_per_page: Accessor<number>;
	plots_per_page: Accessor<number>;
	reports_page: Accessor<number>;
	branches_page: Accessor<number>;
	testbeds_page: Accessor<number>;
	benchmarks_page: Accessor<number>;
	plots_page: Accessor<number>;
	reports_total_count: Accessor<number>;
	branches_total_count: Accessor<number>;
	testbeds_total_count: Accessor<number>;
	benchmarks_total_count: Accessor<number>;
	plots_total_count: Accessor<number>;
	reports_start_date: Accessor<undefined | string>;
	reports_end_date: Accessor<undefined | string>;
	branches_search: Accessor<undefined | string>;
	testbeds_search: Accessor<undefined | string>;
	benchmarks_search: Accessor<undefined | string>;
	plots_search: Accessor<undefined | string>;
	embed_logo: Accessor<boolean>;
	embed_title: Accessor<undefined | string>;
	embed_header: Accessor<boolean>;
	embed_key: Accessor<boolean>;
	handleMeasure: (measure: null | string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleTab: (tab: PerfTab) => void;
	handleKey: (key: boolean) => void;
	handleXAxis: (x_axis: XAxis) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (lower_value: boolean) => void;
	handleUpperValue: (upper_value: boolean) => void;
	handleLowerBoundary: (lower_boundary: boolean) => void;
	handleUpperBoundary: (upper_boundary: boolean) => void;
	handleReportChecked: (index: number) => void;
	handleBranchChecked: (index: undefined | number) => void;
	handleTestbedChecked: (index: undefined | number) => void;
	handleBenchmarkChecked: (index: undefined | number) => void;
	handlePlotChecked: (index: number) => void;
	handleReportsPage: (reports_page: number) => void;
	handleBranchesPage: (branches_page: number) => void;
	handleTestbedsPage: (testbeds_page: number) => void;
	handleBenchmarksPage: (benchmarks_page: number) => void;
	handlePlotsPage: (plots_page: number) => void;
	handleReportsStartTime: (start_time: string) => void;
	handleReportsEndTime: (end_time: string) => void;
	handleBranchesSearch: (branches_search: string) => void;
	handleTestbedsSearch: (testbeds_search: string) => void;
	handleBenchmarksSearch: (benchmarks_search: string) => void;
	handlePlotsSearch: (plots_search: string) => void;
}

const PerfPlot = (props: Props) => {
	const theme = createMemo(() => themeColor(props.theme()));

	return (
		<div class="columns">
			<div class="column">
				<nav class={`panel ${theme()}`}>
					<PlotHeader
						apiUrl={props.apiUrl}
						user={props.user}
						project={props.project}
						project_slug={props.project_slug}
						theme={props.theme}
						isConsole={props.isConsole}
						isEmbed={props.isEmbed}
						isPlotInit={props.isPlotInit}
						measures={props.measures}
						start_date={props.start_date}
						end_date={props.end_date}
						refresh={props.refresh}
						x_axis={props.x_axis}
						clear={props.clear}
						lower_value={props.lower_value}
						upper_value={props.upper_value}
						lower_boundary={props.lower_boundary}
						upper_boundary={props.upper_boundary}
						embed_logo={props.embed_logo}
						embed_title={props.embed_title}
						embed_header={props.embed_header}
						handleMeasure={props.handleMeasure}
						handleStartTime={props.handleStartTime}
						handleEndTime={props.handleEndTime}
						handleXAxis={props.handleXAxis}
						handleClear={props.handleClear}
						handleLowerValue={props.handleLowerValue}
						handleUpperValue={props.handleUpperValue}
						handleLowerBoundary={props.handleLowerBoundary}
						handleUpperBoundary={props.handleUpperBoundary}
					/>
					<div class="panel-block">
						<Switch
							fallback={
								<Plot
									theme={props.theme}
									isConsole={props.isConsole}
									isEmbed={props.isEmbed}
									x_axis={props.x_axis}
									lower_value={props.lower_value}
									upper_value={props.upper_value}
									lower_boundary={props.lower_boundary}
									upper_boundary={props.upper_boundary}
									perfData={props.perfData}
									key={props.key}
									embed_key={props.embed_key}
									handleKey={props.handleKey}
								/>
							}
						>
							<Match when={props.perfData.loading}>
								<progress
									class="progress is-primary"
									style="margin-top: 8rem; margin-bottom: 16rem;"
									max="100"
								/>
							</Match>
							<Match when={props.isPlotInit()}>
								<PlotInit
									measures={props.measures}
									branches={props.branches}
									testbeds={props.testbeds}
									benchmarks={props.benchmarks}
									handleTab={props.handleTab}
								/>
							</Match>
						</Switch>
					</div>
					<Show when={!props.isEmbed}>
						<PlotTab
							project_slug={props.project_slug}
							theme={props.theme}
							isConsole={props.isConsole}
							report={props.report}
							branches={props.branches}
							testbeds={props.testbeds}
							benchmarks={props.benchmarks}
							measures={props.measures}
							tab={props.tab}
							reports_data={props.reports_data}
							branches_data={props.branches_data}
							testbeds_data={props.testbeds_data}
							benchmarks_data={props.benchmarks_data}
							plots_data={props.plots_data}
							reports_tab={props.reports_tab}
							branches_tab={props.branches_tab}
							testbeds_tab={props.testbeds_tab}
							benchmarks_tab={props.benchmarks_tab}
							plots_tab={props.plots_tab}
							reports_per_page={props.reports_per_page}
							branches_per_page={props.branches_per_page}
							testbeds_per_page={props.testbeds_per_page}
							benchmarks_per_page={props.benchmarks_per_page}
							plots_per_page={props.plots_per_page}
							reports_page={props.reports_page}
							branches_page={props.branches_page}
							testbeds_page={props.testbeds_page}
							benchmarks_page={props.benchmarks_page}
							plots_page={props.plots_page}
							reports_total_count={props.reports_total_count}
							branches_total_count={props.branches_total_count}
							testbeds_total_count={props.testbeds_total_count}
							benchmarks_total_count={props.benchmarks_total_count}
							plots_total_count={props.plots_total_count}
							reports_start_date={props.reports_start_date}
							reports_end_date={props.reports_end_date}
							branches_search={props.branches_search}
							testbeds_search={props.testbeds_search}
							benchmarks_search={props.benchmarks_search}
							plots_search={props.plots_search}
							handleTab={props.handleTab}
							handleReportChecked={props.handleReportChecked}
							handleBranchChecked={props.handleBranchChecked}
							handleTestbedChecked={props.handleTestbedChecked}
							handleBenchmarkChecked={props.handleBenchmarkChecked}
							handlePlotChecked={props.handlePlotChecked}
							handleReportsPage={props.handleReportsPage}
							handleBranchesPage={props.handleBranchesPage}
							handleTestbedsPage={props.handleTestbedsPage}
							handleBenchmarksPage={props.handleBenchmarksPage}
							handlePlotsPage={props.handlePlotsPage}
							handleReportsStartTime={props.handleReportsStartTime}
							handleReportsEndTime={props.handleReportsEndTime}
							handleBranchesSearch={props.handleBranchesSearch}
							handleTestbedsSearch={props.handleTestbedsSearch}
							handleBenchmarksSearch={props.handleBenchmarksSearch}
							handlePlotsSearch={props.handlePlotsSearch}
						/>
					</Show>
				</nav>
			</div>
		</div>
	);
};

export default PerfPlot;
