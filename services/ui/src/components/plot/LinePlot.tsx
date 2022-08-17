import { createSignal, createEffect, createResource } from "solid-js";
import * as d3 from "d3";

const [metaMetrics, setMetaMetrics] = createSignal([]);

import * as Plot from "@observablehq/plot";
import axios from "axios";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

export function LinePlot() {
  const [metricsId, setMetricsId] = createSignal();
  const [plot] = createResource(metricsId, fetchPlot);

  createEffect(() => {
    // TODO this should actually updated via a prop for the plot parameters
    setMetricsId("");
  });

  return <div>{plot()}</div>;
}

const fetchPlot = async (metrics_id) => {
  try {
    let meta_metrics_array = await axios(options);
    let data_arrays = intoDataArrays(meta_metrics_array?.data);
    let plot_marks = intoPlotMarks(data_arrays);
    let plot = Plot.plot({
      y: {
        grid: true,
      },
      marks: plot_marks,
    });
    return plot;
  } catch (error) {
    console.error(error);
  }
};

const options = {
  url: `${BENCHER_API_URL}/v0/metrics`,
  method: "get",
  headers: {
    "Content-Type": "application/json",
    // Only use with explicit CORS
    // Authorization: `Bearer ${window.localStorage.authToken}`
  },
};

const intoDataArrays = (meta_metrics_array) => {
  var data_arrays = {};

  for (let i = 0; i < meta_metrics_array.length; i++) {
    let meta_metrics_data = meta_metrics_array[i];

    let date_time = new Date(meta_metrics_data.date_time);
    let metrics_obj = meta_metrics_data?.metrics;

    for (const [key, value] of Object.entries(metrics_obj)) {
      let latency = value?.latency?.duration;
      let nanos = latency?.secs * 1_000_000_000 + latency?.nanos;
      let xy = [date_time, nanos];
      if (data_arrays[key] === undefined) {
        data_arrays[key] = [xy];
      } else {
        data_arrays[key].push(xy);
      }
    }
  }

  return data_arrays;
};

const intoPlotMarks = (data_arrays) => {
  let plot_arrays = [];

  let colors = d3.schemeTableau10;
  let index = 0;
  for (const [key, value] of Object.entries(data_arrays)) {
    let color = colors[index % 10];
    plot_arrays.push(Plot.line(value, { stroke: color }));
    index++;
  }

  return plot_arrays;
};
