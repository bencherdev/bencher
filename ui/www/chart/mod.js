import { Reports } from "ui";
import { GroupedBarChart } from "./grouped_bar_chart";
import { LineChart } from "./multi_line_chart";
import * as d3 from "d3";
const queryString = require('query-string');

export function chart(inventory, data) {
  if (inventory === undefined || data === undefined) {
    return;
  }
  console.log(data);
  const d3_chart = document.getElementById("bencher-chart");
  if (false) {
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
  } else {
    const fake_data = `division,date,unemployment
    "Bethesda",2000-01-01,2.6
    "Bethesda",2000-02-01,2.6
    "Bethesda",2000-03-01,2.6
    "Bethesda",2000-04-01,2.6
    "Bethesda",2000-05-01,2.7
    "Bethesda",2000-06-01,2.7
    "Boston-Cambridge-Quincy, MA NECTA Div",2000-01-01,2.7
    "Boston-Cambridge-Quincy, MA NECTA Div",2000-02-01,2.6
    "Boston-Cambridge-Quincy, MA NECTA Div",2000-03-01,2.6
    "Boston-Cambridge-Quincy, MA NECTA Div",2000-04-01,2.5
    "Boston-Cambridge-Quincy, MA NECTA Div",2000-05-01,2.4
    "Brockton-Bridgewater-Easton, MA NECTA Div",2000-01-01,3
    "Brockton-Bridgewater-Easton, MA NECTA Div",2000-02-01,3
    "Brockton-Bridgewater-Easton, MA NECTA Div",2000-03-01,2.9
    "Brockton-Bridgewater-Easton, MA NECTA Div",2000-04-01,2.9
    "Brockton-Bridgewater-Easton, MA NECTA Div",2000-05-01,2.8`;
    const data_array = [
      {date: "2000-01-01", unemployment: 3.0, division: "Bethesda"},
      {date: "2000-02-01", unemployment: 3.0, division: "Bethesda"},
      {date: "2000-03-01", unemployment: 2.9, division: "Bethesda"},
      {date: "2000-04-01", unemployment: 2.9, division: "Bethesda"},
      {date: "2000-05-01", unemployment: 2.8, division: "Bethesda"},
      {date: "2000-01-01", unemployment: 1.0, division: "Brockton"},
      {date: "2000-02-01", unemployment: 2.0, division: "Brockton"},
      {date: "2000-03-01", unemployment: 2.9, division: "Brockton"},
      {date: "2000-04-01", unemployment: 3.9, division: "Brockton"},
      {date: "2000-05-01", unemployment: 2.8, division: "Brockton"},
    ];
    const new_data = [
      {"division":"Bethesda","date":"2000-01-01T00:00:00.000Z","unemployment":2.6},
      {"division":"Bethesda","date":"2000-02-01T00:00:00.000Z","unemployment":2.6},
      {"division":"Bethesda","date":"2000-03-01T00:00:00.000Z","unemployment":2.6},
      {"division":"Bethesda","date":"2000-04-01T00:00:00.000Z","unemployment":2.6},
      {"division":"Bethesda","date":"2000-05-01T00:00:00.000Z","unemployment":2.7},
      {"division":"Bethesda","date":"2000-06-01T00:00:00.000Z","unemployment":2.7},
    ];
    const chart = LineChart(data_array, {
      x: d => d.date,
      y: d => d.unemployment,
      z: d => d.division,
      yLabel: "↑ Unemployment (%)",
      width: 640,
      height: 500,
      color: "steelblue",
      voronoi: false, // if true, show Voronoi overlay
    });
    d3.select(d3_chart).node().appendChild(chart);
  }
}

