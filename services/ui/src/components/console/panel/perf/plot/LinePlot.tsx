import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { PerKind } from "../../../config/types";

const getPerf = (kind, perf) => {
  switch (kind) {
    case PerKind.LATENCY:
      return perf?.latency?.duration;
    case PerKind.THROUGHPUT:
      return perf?.throughput?.event / perf?.throughput?.unit_time;
    case PerKind.COMPUTE:
      return perf?.compute?.duration;
    case PerKind.MEMORY:
      return perf?.memory?.duration;
    case PerKind.STORAGE:
      return perf?.storage?.duration;
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
    const json_perf = props.perf_data();
    if (
      typeof json_perf !== "object" ||
      json_perf === null ||
      !Array.isArray(json_perf.data)
    ) {
      return;
    }

    const plot_arrays = [];
    const colors = d3.schemeTableau10;
    json_perf.data.forEach((datum, index) => {
      const perfs = datum.perfs;
      if (!(Array.isArray(perfs) && props.perf_active[index])) {
        return;
      }

      const line_data = [];
      perfs.forEach((perf) => {
        const x_value = new Date(perf.start_time);
        x_value.setSeconds(x_value.getSeconds() + perf.iteration);
        const y_value = getPerf(json_perf.kind, perf);
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
    });
  };

  return <div>{plotted()}</div>;
};

export default LinePlot;
