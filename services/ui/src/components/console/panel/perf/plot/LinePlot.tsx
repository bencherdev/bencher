import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { PerKind } from "../../../config/types";

const getDatum = (kind, datum) => {
  switch (kind) {
    case PerKind.LATENCY:
      return datum?.duration;
    case PerKind.THROUGHPUT:
      return 0;
    case PerKind.COMPUTE:
      return 0;
    case PerKind.MEMORY:
      return 0;
    case PerKind.STORAGE:
      return 0;
    default:
      return 0;
  }
};

const getLabel = (kind) => {
  switch (kind) {
    case PerKind.LATENCY:
      return "↑ Nanoseconds";
    case PerKind.THROUGHPUT:
      return "TODO";
    case PerKind.COMPUTE:
      return "TODO";
    case PerKind.MEMORY:
      return "TODO";
    case PerKind.STORAGE:
      return "TODO";
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
        const date_time = new Date(datum.start_time);
        const x_value = date_time.setSeconds(
          date_time.getSeconds() + datum.iteration
        );
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
