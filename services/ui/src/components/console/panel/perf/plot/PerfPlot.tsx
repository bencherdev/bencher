import { createSignal, For } from "solid-js";
import { PerKind, perfKindCapitalized } from "../../../config/types";
import PlotHeader from "./PlotHeader";
import PlotTab from "./PlotTab";

const perf_kinds = [
  PerKind.LATENCY,
  PerKind.THROUGHPUT,
  PerKind.COMPUTE,
  PerKind.MEMORY,
  PerKind.STORAGE,
];

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
          <p class="panel-tabs">
            <a class="is-active">Branches</a>
            <a>Testbeds</a>
            <a>Benchmarks</a>
          </p>
          <PlotTab />
        </nav>
      </div>
    </div>
  );
};

export default PerfPlot;
