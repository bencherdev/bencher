import { createEffect, createSignal } from "solid-js";
import LinePlot from "./LinePlot";
import PlotKey from "./PlotKey";

const Plot = (props) => {
  return (
    <div class="container">
      <div class="columns is-reverse-mobile is-vcentered">
        <div class="column is-narrow">
          <PlotKey
            config={props.config}
            path_params={props.path_params}
            branches={props.branches}
            testbeds={props.testbeds}
            benchmarks={props.benchmarks}
            perf_data={props.perf_data}
            key={props.key}
            handleKey={props.handleKey}
          />
        </div>
        <div
          class={`column is-narrow ${
            props.key() ? "is-three-quarters" : "is-11"
          }`}
        >
          <LinePlot perf_data={props.perf_data} />
        </div>
      </div>
    </div>
  );
};

export default Plot;
