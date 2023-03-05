import Plot from "./Plot";
import PlotHeader from "./PlotHeader";
import PlotInit from "./PlotInit";
import PlotTab from "./PlotTab";

const PerfPlot = (props) => {
	return (
		<div class="columns">
			<div class="column">
				<nav class="panel">
					<PlotHeader
						user={props.user}
						config={props.config}
						path_params={props.path_params}
						metric_kind={props.metric_kind}
						start_date={props.start_date}
						end_date={props.end_date}
						refresh={props.refresh}
						handleMetricKind={props.handleMetricKind}
						handleStartTime={props.handleStartTime}
						handleEndTime={props.handleEndTime}
					/>
					<div class="panel-block" id="perf_viewport">
						{props.isPlotInit() ? (
							<PlotInit
								metric_kind={props.metric_kind}
								branches={props.branches}
								testbeds={props.testbeds}
								benchmarks={props.benchmarks}
								handleTab={props.handleTab}
							/>
						) : (
							<Plot
								user={props.user}
								config={props.config}
								path_params={props.path_params}
								branches={props.branches}
								testbeds={props.testbeds}
								benchmarks={props.benchmarks}
								perf_data={props.perf_data}
								key={props.key}
								handleKey={props.handleKey}
							/>
						)}
					</div>
					<PlotTab
						project_slug={props.project_slug}
						tab={props.tab}
						branches_tab={props.branches_tab}
						testbeds_tab={props.testbeds_tab}
						benchmarks_tab={props.benchmarks_tab}
						handleTab={props.handleTab}
						handleBranchChecked={props.handleBranchChecked}
						handleTestbedChecked={props.handleTestbedChecked}
						handleBenchmarkChecked={props.handleBenchmarkChecked}
					/>
				</nav>
			</div>
		</div>
	);
};

export default PerfPlot;
