import { createEffect, createSignal } from "solid-js";
import PlotKey from "./PlotKey";

const Plot = (props) => {
  createEffect(() => {
    console.log(props.perf_data());
  });
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
        <div class={`column ${props.key() ? "is-three-quarters" : "is-full"}`}>
          <div class="box">
            <div class="content">Plot</div>
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
            <br />
          </div>
        </div>
      </div>
    </div>
  );
};

export default Plot;
