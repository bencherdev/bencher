import { InventoryData, Reports } from "ui";
import { GroupedBarChart } from "./bar_chart";
const queryString = require('query-string');
import * as d3 from "d3";


const d3_chart = document.getElementById("d3-chart");

const reports_arg = queryString.parse(location.search).reports;
var inventory;
var data;
var chart;
if (true && reports_arg) {
  const reports = Reports.from_str(reports_arg);
  const inv_data = reports.latency();
  inventory = JSON.parse(inv_data.inventory());
  data = JSON.parse(inv_data.data());

  // chart = BarChart(data, {
  //   x: d => d.date_time,
  //   y: d => d.duration,
  //   xLabel: "Date and Time",
  //   yLabel: "Time (ns)",
  //   width: 640,
  //   height: 500,
  //   color: "steelblue"
  // });
} else {
  data = [
    {"date_time":"AL","name":"<10","duration":598478},
    {"date_time":"AK","name":"<10","duration":106741},
    {"date_time":"AZ","name":"<10","duration":892083},
    {"date_time":"AR","name":"<10","duration":392177},
    {"date_time":"AL","name":"10-19","duration":638789},
    {"date_time":"AK","name":"10-19","duration":99926},
    {"date_time":"AZ","name":"10-19","duration":912735},
    {"date_time":"AR","name":"10-19","duration":397185},
    {"date_time":"AL","name":"20-29","duration":661666},
    {"date_time":"AK","name":"20-29","duration":120674},
    {"date_time":"AZ","name":"20-29","duration":939804},
    {"date_time":"AR","name":"20-29","duration":399698},
    {"date_time":"AL","name":"30-39","duration":603013},
    {"date_time":"AK","name":"30-39","duration":102008},
    {"date_time":"AZ","name":"30-39","duration":857054},
    {"date_time":"AR","name":"30-39","duration":372998},
    {"date_time":"AL","name":"40-49","duration":625599},
    {"date_time":"AK","name":"40-49","duration":91539},
    {"date_time":"AZ","name":"40-49","duration":833290},
    {"date_time":"AR","name":"40-49","duration":370157},
    {"date_time":"AL","name":"50-59","duration":673864},
    {"date_time":"AK","name":"50-59","duration":104569},
    {"date_time":"AZ","name":"50-59","duration":834858},
    {"date_time":"AR","name":"50-59","duration":395070},
    {"date_time":"AL","name":"60-69","duration":548376},
    {"date_time":"AK","name":"60-69","duration":70473},
    {"date_time":"AZ","name":"60-69","duration":737884},
    {"date_time":"AR","name":"60-69","duration":329734},
    {"date_time":"AL","name":"70-79","duration":316598},
    {"date_time":"AK","name":"70-79","duration":28422},
    {"date_time":"AZ","name":"70-79","duration":466153},
    {"date_time":"AR","name":"70-79","duration":197985},
    {"date_time":"AL","name":"≥80","duration":174781},
    {"date_time":"AK","name":"≥80","duration":12503},
    {"date_time":"AZ","name":"≥80","duration":254716},
    {"date_time":"AR","name":"≥80","duration":113468},
  ];

  var inventory = ["<10","10-19","20-29","30-39","40-49","50-59","60-69","70-79","≥80"];

  // chart = GroupedBarChart(data, {
  //   x: d => d.date_time,
  //   y: d => d.duration,
  //   z: d => d.name,
  //   xLabel: "Date and Time",
  //   yLabel: "Time (ns)",
  //   zDomain: inventory,
  //   colors: d3.schemeSpectral[inventory.length],
  //   width: 640,
  //   height: 500
  // })
}

chart = GroupedBarChart(data, {
  x: d => d.date_time,
  y: d => d.duration,
  z: d => d.name,
  xLabel: "Date and Time",
  yLabel: "Time (ns)",
  zDomain: inventory,
  colors: d3.schemeSpectral[inventory.length],
  width: 640,
  height: 500
})


d3.select(d3_chart).node().appendChild(chart);


