import * as Plot from "@observablehq/plot";
import * as d3 from "d3";
import { createSignal } from "solid-js";
import { PerKind } from "../../../config/types";

// TODO query the backend based off of metric kind
const getLabel = (kind) => {
  switch (kind) {
    default:
      return "↑ UNITS";
  }
};

const LinePlot = (props) => {
  const [max_units, setMaxUnits] = createSignal(1);

  const handleMaxUnits = (value: number) => {
    setMaxUnits(Math.max(max_units(), Math.round(value).toString().length));
  };

  const plotted = () => {
    const json_perf = props.perf_data();
    if (
      typeof json_perf !== "object" ||
      json_perf === null ||
      !Array.isArray(json_perf.results)
    ) {
      return;
    }

    const plot_arrays = [];
    const colors = d3.schemeTableau10;
    json_perf.results.forEach((result, index) => {
      const perf_metrics = result.metrics;
      if (!(Array.isArray(perf_metrics) && props.perf_active[index])) {
        return;
      }

      const line_data = [];
      perf_metrics.forEach((perf_metric) => {
        const x_value = new Date(perf_metric.start_time);
        x_value.setSeconds(x_value.getSeconds() + perf_metric.iteration);
        const y_value = perf_metric.metric?.value;
        handleMaxUnits(y_value);
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
      nice: true,
      // https://github.com/observablehq/plot/blob/main/README.md#layout-options
      // For simplicity’s sake and for consistent layout across plots, margins are not automatically sized to make room for tick labels; instead, shorten your tick labels or increase the margins as needed.
      marginLeft: max_units() * 10,
    });
  };

  return <>{plotted()}</>;
};

export default LinePlot;
