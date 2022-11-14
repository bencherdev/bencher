import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { PerKind } from "../../../config/types";

// TODO query the backend based off of metric kind
const getLabel = (kind) => {
  switch (kind) {
    case PerKind.LATENCY:
      return "↑ Nanoseconds";
    case PerKind.THROUGHPUT:
      return "↑ Events per Nanoseconds";
    case PerKind.COMPUTE:
    case PerKind.MEMORY:
    case PerKind.STORAGE:
      return "↑ Average Performance";
    default:
      return "↑ UNITS";
  }
};

const LinePlot = (props) => {
  const plotted = () => {
    const json_perf = props.perf_data();
    if (
      typeof json_perf !== "object" ||
      json_perf === null ||
      !Array.isArray(json_perf.benchmarks)
    ) {
      return;
    }

    const plot_arrays = [];
    const colors = d3.schemeTableau10;
    json_perf.benchmarks.forEach((benchmark, index) => {
      const data = benchmark.data;
      if (!(Array.isArray(data) && props.perf_active[index])) {
        return;
      }

      const line_data = [];
      data.forEach((datum) => {
        const x_value = new Date(datum.start_time);
        x_value.setSeconds(x_value.getSeconds() + datum.iteration);
        const y_value = datum.metrics?.value;
        const xy = [x_value, y_value];
        line_data.push(xy);
      });

      const color = colors[index % 10];
      plot_arrays.push(Plot.line(line_data, { stroke: color }));
    });

    return Plot.plot({
      y: {
        grid: true,
        label: getLabel(json_perf.kind),
      },
      marks: plot_arrays,
      width: props.width(),
    });
  };

  return <>{plotted()}</>;
};

export default LinePlot;
