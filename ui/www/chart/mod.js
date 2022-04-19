import { Reports } from "ui";
import { GroupedBarChart } from "./grouped_bar_chart";
import * as d3 from "d3";
const queryString = require('query-string');

export function chart() {
  var inventory = [];
  var data = [];

  const reports_arg = queryString.parse(location.search).reports;
  if (reports_arg) {
    const reports = Reports.from_str(reports_arg);
    const inv_data = reports.latency();
    inventory = JSON.parse(inv_data.inventory());
    data = JSON.parse(inv_data.data());
  }

  const d3_chart = document.getElementById("bencher-chart");
  const chart = GroupedBarChart(data, {
    x: d => d.date_time,
    y: d => d.duration,
    z: d => d.name,
    xLabel: "Date and Time",
    yLabel: "Time (ns)",
    zDomain: inventory,
    colors: d3.schemeTableau10,
    width: 640,
    height: 500
  })
  d3.select(d3_chart).node().appendChild(chart);
}

