import { createResource } from "solid-js";
import { createStore } from "solid-js/store";
import LinePlot from "./LinePlot";
import PlotKey from "./PlotKey";

const Plot = (props) => {
  const [perf_active, setPerfActive] = createStore([]);

  const [_perf_active] = createResource(props.perf_data, (json_perf) => {
    const active = [];
    json_perf?.benchmarks?.forEach(() => {
      active.push(true);
    });
    setPerfActive(active);
    return active;
  });

  const handlePerfActive = (index: number) => {
    const active = [...perf_active];
    active[index] = !active[index];
    setPerfActive(active);
  };

  return (
    <div class="container">
      <div
        class={`columns is-reverse-mobile ${props.key() ? "" : "is-vcentered"}`}
      >
        <div class="column is-narrow">
          <PlotKey
            config={props.config}
            path_params={props.path_params}
            branches={props.branches}
            testbeds={props.testbeds}
            benchmarks={props.benchmarks}
            perf_data={props.perf_data}
            key={props.key}
            perf_active={perf_active}
            handleKey={props.handleKey}
            handlePerfActive={handlePerfActive}
          />
        </div>
        <div
          class={`column is-narrow ${
            props.key() ? "is-three-quarters" : "is-11"
          }`}
        >
          <nav class="level">
            <LinePlot perf_data={props.perf_data} perf_active={perf_active} />
          </nav>
        </div>
      </div>
    </div>
  );
};

export default Plot;
