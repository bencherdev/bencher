import { createSignal, For } from "solid-js";
import { PerKind } from "../../../config/types";
import PlotHeader from "./PlotHeader";
import PlotTab from "./PlotTab";

const PerfPlot = (props) => {
  return (
    <div class="columns">
      <div class="column">
        <nav class="panel">
          <PlotHeader
            query={props.query}
            start_date={props.start_date}
            end_date={props.end_date}
            handleKind={props.handleKind}
            handleStartTime={props.handleStartTime}
            handleEndTime={props.handleEndTime}
          />
          <div class="panel-block">
            <p>TODO PLOT HERE</p>
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
