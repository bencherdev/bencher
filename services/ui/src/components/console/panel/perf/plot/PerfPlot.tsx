import { createSignal, For } from "solid-js";
import { PerKind, perfKindCapitalized } from "../../../config/types";
import PlotHeader from "./PlotHeader";

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
          <a class="panel-block is-active">
            <input type="checkbox" />A
          </a>
          <a class="panel-block">
            <input type="checkbox" />B
          </a>
          <a class="panel-block">
            <input type="checkbox" />C
          </a>
        </nav>
      </div>
    </div>
  );
};

export default PerfPlot;
