import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { PerKind } from "../../../config/types";

const getDatum = (kind, datum) => {
  switch (kind) {
    case PerKind.LATENCY:
      return datum?.duration;
    case PerKind.THROUGHPUT:
      return datum?.event / datum?.unit_time;
    case PerKind.COMPUTE:
    case PerKind.MEMORY:
    case PerKind.STORAGE:
      return datum?.avg;
    default:
      return 0;
  }
};

const getLabel = (kind) => {
  switch (kind) {
    case PerKind.LATENCY:
      return "↑ Nanoseconds";
    case PerKind.THROUGHPUT:
      return "↑ Events per Nanoseconds";
    case PerKind.COMPUTE:
    case PerKind.MEMORY:
    case PerKind.STORAGE:
      return "↑ Average performance";
    default:
      return "↑ UNITS";
  }
};

const LinePlot = (props) => {
  const plotted = () => {
    const perf_data = props.perf_data();
    if (
      typeof perf_data !== "object" ||
      perf_data === null ||
      !Array.isArray(perf_data.perf)
    ) {
      return;
    }

    const plot_arrays = [];
    const colors = d3.schemeTableau10;
    perf_data.perf.forEach((perf, index) => {
      const data = perf.data;
      if (!(Array.isArray(data) && props.perf_active[index])) {
        return;
      }

      const line_data = [];
      data.forEach((datum) => {
        const x_value = new Date(datum.start_time);
        x_value.setSeconds(x_value.getSeconds() + datum.iteration);
        const y_value = getDatum(perf_data.kind, datum.datum);
        const xy = [x_value, y_value];
        line_data.push(xy);
      });

      const color = colors[index % 10];
      plot_arrays.push(Plot.line(line_data, { stroke: color }));
    });

    return Plot.plot({
      y: {
        grid: true,
        label: getLabel(perf_data.kind),
      },
      marks: plot_arrays,
    });
  };

  return <div>{plotted()}</div>;
};

export default LinePlot;
