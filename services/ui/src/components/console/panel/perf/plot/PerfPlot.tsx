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
            kind={props.kind}
            start_date={props.start_date}
            end_date={props.end_date}
            handleKind={props.handleKind}
            handleStartTime={props.handleStartTime}
            handleEndTime={props.handleEndTime}
          />
          <div class="panel-block">
            {props.isPlotInit() ? (
              <PlotInit
                branches={props.branches}
                testbeds={props.testbeds}
                benchmarks={props.benchmarks}
                handleTab={props.handleTab}
              />
            ) : (
              <Plot
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
