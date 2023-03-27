import { createSignal, Show } from "solid-js";
import { XAxis } from "../../../config/types";
import Plot from "./Plot";
import PlotHeader from "./PlotHeader";
import PlotInit from "./PlotInit";
import PlotTab from "./PlotTab";

const PerfPlot = (props) => {
	const [x_axis, set_x_axis] = createSignal(XAxis.DATE_TIME);

	const handle_x_axis = () => {
		switch (x_axis()) {
			case XAxis.DATE_TIME:
				set_x_axis(XAxis.VERSION);
				break;
			case XAxis.VERSION:
				set_x_axis(XAxis.DATE_TIME);
				break;
		}
	};

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
						handle_x_axis={handle_x_axis}
					/>
					<div class="panel-block">
						<Show
							when={props.isPlotInit()}
							fallback={
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
									x_axis={x_axis}
								/>
							}
						>
							<PlotInit
								metric_kind={props.metric_kind}
								branches={props.branches}
								testbeds={props.testbeds}
								benchmarks={props.benchmarks}
								handleTab={props.handleTab}
							/>
						</Show>
					</div>
					<PlotTab
						project_slug={props.project_slug}
						is_console={props.is_console}
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
