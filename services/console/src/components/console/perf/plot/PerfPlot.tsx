import { type Accessor, type Resource, Show } from "solid-js";
import { type PerfRange, type PerfTab } from "../../../../config/types";
import type {
	JsonAuthUser,
	JsonBenchmark,
	JsonBranch,
	JsonPerf,
	JsonProject,
	JsonReport,
	JsonTestbed,
} from "../../../../types/bencher";
import Plot from "./Plot";
import PlotHeader from "./PlotHeader";
import PlotInit from "./PlotInit";
import PlotTab, { type TabList } from "./PlotTab";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Resource<JsonProject>;
	project_slug: Accessor<undefined | string>;
	isConsole: boolean;
	isEmbed: boolean;
	isPlotInit: Accessor<boolean>;
	report: Accessor<undefined | string>;
	metric_kinds: Accessor<string[]>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	start_date: Accessor<undefined | string>;
	end_date: Accessor<undefined | string>;
	refresh: () => void;
	perfData: Accessor<JsonPerf>;
	tab: Accessor<PerfTab>;
	key: Accessor<boolean>;
	range: Accessor<PerfRange>;
	clear: Accessor<boolean>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	reports_tab: TabList<JsonReport>;
	branches_tab: TabList<JsonBranch>;
	testbeds_tab: TabList<JsonTestbed>;
	benchmarks_tab: TabList<JsonBenchmark>;
	reports_per_page: Accessor<number>;
	branches_per_page: Accessor<number>;
	testbeds_per_page: Accessor<number>;
	benchmarks_per_page: Accessor<number>;
	reports_page: Accessor<number>;
	branches_page: Accessor<number>;
	testbeds_page: Accessor<number>;
	benchmarks_page: Accessor<number>;
	handleMetricKind: (metric_kind: null | string) => void;
	handleStartTime: (start_time: string) => void;
	handleEndTime: (end_time: string) => void;
	handleTab: (tab: PerfTab) => void;
	handleKey: (key: boolean) => void;
	handleRange: (range: PerfRange) => void;
	handleClear: (clear: boolean) => void;
	handleLowerValue: (lower_value: boolean) => void;
	handleUpperValue: (upper_value: boolean) => void;
	handleLowerBoundary: (lower_boundary: boolean) => void;
	handleUpperBoundary: (upper_boundary: boolean) => void;
	handleReportChecked: (
		index: number,
		metric_kind_uuid: undefined | string,
	) => void;
	handleBranchChecked: (index: number) => void;
	handleTestbedChecked: (index: number) => void;
	handleBenchmarkChecked: (index: number) => void;
	handleReportsPage: (reports_page: number) => void;
	handleBranchesPage: (branches_page: number) => void;
	handleTestbedsPage: (testbeds_page: number) => void;
	handleBenchmarksPage: (benchmarks_page: number) => void;
}

const PerfPlot = (props: Props) => {
	return (
		<div class="columns">
			<div class="column">
				<nav class="panel">
					<PlotHeader
						apiUrl={props.apiUrl}
						user={props.user}
						project={props.project}
						project_slug={props.project_slug}
						isConsole={props.isConsole}
						isEmbed={props.isEmbed}
						isPlotInit={props.isPlotInit}
						metric_kinds={props.metric_kinds}
						start_date={props.start_date}
						end_date={props.end_date}
						refresh={props.refresh}
						range={props.range}
						clear={props.clear}
						lower_value={props.lower_value}
						upper_value={props.upper_value}
						lower_boundary={props.lower_boundary}
						upper_boundary={props.upper_boundary}
						handleMetricKind={props.handleMetricKind}
						handleStartTime={props.handleStartTime}
						handleEndTime={props.handleEndTime}
						handleRange={props.handleRange}
						handleClear={props.handleClear}
						handleLowerValue={props.handleLowerValue}
						handleUpperValue={props.handleUpperValue}
						handleLowerBoundary={props.handleLowerBoundary}
						handleUpperBoundary={props.handleUpperBoundary}
					/>
					<div class="panel-block">
						<Show
							when={props.isPlotInit()}
							fallback={
								<Plot
									range={props.range}
									lower_value={props.lower_value}
									upper_value={props.upper_value}
									lower_boundary={props.lower_boundary}
									upper_boundary={props.upper_boundary}
									perfData={props.perfData}
									key={props.key}
									handleKey={props.handleKey}
								/>
							}
						>
							<PlotInit
								metric_kinds={props.metric_kinds}
								branches={props.branches}
								testbeds={props.testbeds}
								benchmarks={props.benchmarks}
								handleTab={props.handleTab}
							/>
						</Show>
					</div>
					<Show when={props.isEmbed !== true}>
						<PlotTab
							project_slug={props.project_slug}
							isConsole={props.isConsole}
							metric_kinds={props.metric_kinds}
							tab={props.tab}
							reports_tab={props.reports_tab}
							branches_tab={props.branches_tab}
							testbeds_tab={props.testbeds_tab}
							benchmarks_tab={props.benchmarks_tab}
							reports_per_page={props.reports_per_page}
							branches_per_page={props.branches_per_page}
							testbeds_per_page={props.testbeds_per_page}
							benchmarks_per_page={props.benchmarks_per_page}
							reports_page={props.reports_page}
							branches_page={props.branches_page}
							testbeds_page={props.testbeds_page}
							benchmarks_page={props.benchmarks_page}
							handleTab={props.handleTab}
							handleReportChecked={props.handleReportChecked}
							handleBranchChecked={props.handleBranchChecked}
							handleTestbedChecked={props.handleTestbedChecked}
							handleBenchmarkChecked={props.handleBenchmarkChecked}
							handleReportsPage={props.handleReportsPage}
							handleBranchesPage={props.handleBranchesPage}
							handleTestbedsPage={props.handleTestbedsPage}
							handleBenchmarksPage={props.handleBenchmarksPage}
						/>
					</Show>
				</nav>
			</div>
		</div>
	);
};

export default PerfPlot;
