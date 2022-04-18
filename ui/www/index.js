import { InventoryData, Reports } from "ui";
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



function BarChart(data, {
    x = (d, i) => i, // given d in data, returns the (ordinal) x-value
    y = d => d, // given d in data, returns the (quantitative) y-value
    title, // given d in data, returns the title text
    marginTop = 20, // the top margin, in pixels
    marginRight = 0, // the right margin, in pixels
    marginBottom = 30, // the bottom margin, in pixels
    marginLeft = 40, // the left margin, in pixels
    width = 640, // the outer width of the chart, in pixels
    height = 400, // the outer height of the chart, in pixels
    xDomain, // an array of (ordinal) x-values
    xRange = [marginLeft, width - marginRight], // [left, right]
    yType = d3.scaleLinear, // y-scale type
    yDomain, // [ymin, ymax]
    yRange = [height - marginBottom, marginTop], // [bottom, top]
    xPadding = 0.1, // amount of x-range to reserve to separate bars
    yFormat, // a format specifier string for the y-axis
    yLabel, // a label for the y-axis
    color = "currentColor" // bar fill color
  } = {}) {
    // Compute values.
    const X = d3.map(data, x);
    const Y = d3.map(data, y);
  
    // Compute default domains, and unique the x-domain.
    if (xDomain === undefined) xDomain = X;
    if (yDomain === undefined) yDomain = [0, d3.max(Y)];
    xDomain = new d3.InternSet(xDomain);
  
    // Omit any data not present in the x-domain.
    const I = d3.range(X.length).filter(i => xDomain.has(X[i]));
  
    // Construct scales, axes, and formats.
    const xScale = d3.scaleBand(xDomain, xRange).padding(xPadding);
    const yScale = yType(yDomain, yRange);
    const xAxis = d3.axisBottom(xScale).tickSizeOuter(0);
    const yAxis = d3.axisLeft(yScale).ticks(height / 40, yFormat);
  
    // Compute titles.
    if (title === undefined) {
      const formatValue = yScale.tickFormat(100, yFormat);
      title = i => `${X[i]}\n${formatValue(Y[i])}`;
    } else {
      const O = d3.map(data, d => d);
      const T = title;
      title = i => T(O[i], i, data);
    }
  
    const svg = d3.create("svg")
        .attr("width", width)
        .attr("height", height)
        .attr("viewBox", [0, 0, width, height])
        .attr("style", "max-width: 100%; height: auto; height: intrinsic;");
  
    svg.append("g")
        .attr("transform", `translate(${marginLeft},0)`)
        .call(yAxis)
        .call(g => g.select(".domain").remove())
        .call(g => g.selectAll(".tick line").clone()
            .attr("x2", width - marginLeft - marginRight)
            .attr("stroke-opacity", 0.1))
        .call(g => g.append("text")
            .attr("x", -marginLeft)
            .attr("y", 10)
            .attr("fill", "currentColor")
            .attr("text-anchor", "start")
            .text(yLabel));
  
    const bar = svg.append("g")
        .attr("fill", color)
      .selectAll("rect")
      .data(I)
      .join("rect")
        .attr("x", i => xScale(X[i]))
        .attr("y", i => yScale(Y[i]))
        .attr("height", i => yScale(0) - yScale(Y[i]))
        .attr("width", xScale.bandwidth());
  
    if (title) bar.append("title")
        .text(title);
  
    svg.append("g")
        .attr("transform", `translate(0,${height - marginBottom})`)
        .call(xAxis);
  
    return svg.node();
}

// Copyright 2021 Observable, Inc.
// Released under the ISC license.
// https://observablehq.com/@d3/grouped-bar-chart
function GroupedBarChart(data, {
  x = (d, i) => i, // given d in data, returns the (ordinal) x-value
  y = d => d, // given d in data, returns the (quantitative) y-value
  z = () => 1, // given d in data, returns the (categorical) z-value
  title, // given d in data, returns the title text
  marginTop = 30, // top margin, in pixels
  marginRight = 0, // right margin, in pixels
  marginBottom = 30, // bottom margin, in pixels
  marginLeft = 40, // left margin, in pixels
  width = 640, // outer width, in pixels
  height = 400, // outer height, in pixels
  xDomain, // array of x-values
  xRange = [marginLeft, width - marginRight], // [xmin, xmax]
  xPadding = 0.1, // amount of x-range to reserve to separate groups
  yType = d3.scaleLinear, // type of y-scale
  yDomain, // [ymin, ymax]
  yRange = [height - marginBottom, marginTop], // [ymin, ymax]
  zDomain, // array of z-values
  zPadding = 0.05, // amount of x-range to reserve to separate bars
  yFormat, // a format specifier string for the y-axis
  yLabel, // a label for the y-axis
  colors = d3.schemeTableau10, // array of colors
} = {}) {
  // Compute values.
  const X = d3.map(data, x);
  const Y = d3.map(data, y);
  const Z = d3.map(data, z);

  // Compute default domains, and unique the x- and z-domains.
  if (xDomain === undefined) xDomain = X;
  if (yDomain === undefined) yDomain = [0, d3.max(Y)];
  if (zDomain === undefined) zDomain = Z;
  xDomain = new d3.InternSet(xDomain);
  zDomain = new d3.InternSet(zDomain);

  // Omit any data not present in both the x- and z-domain.
  const I = d3.range(X.length).filter(i => xDomain.has(X[i]) && zDomain.has(Z[i]));

  // Construct scales, axes, and formats.
  const xScale = d3.scaleBand(xDomain, xRange).paddingInner(xPadding);
  const xzScale = d3.scaleBand(zDomain, [0, xScale.bandwidth()]).padding(zPadding);
  const yScale = yType(yDomain, yRange);
  const zScale = d3.scaleOrdinal(zDomain, colors);
  const xAxis = d3.axisBottom(xScale).tickSizeOuter(0);
  const yAxis = d3.axisLeft(yScale).ticks(height / 60, yFormat);

  // Compute titles.
  if (title === undefined) {
    const formatValue = yScale.tickFormat(100, yFormat);
    title = i => `${X[i]}\n${Z[i]}\n${formatValue(Y[i])}`;
  } else {
    const O = d3.map(data, d => d);
    const T = title;
    title = i => T(O[i], i, data);
  }

  const svg = d3.create("svg")
      .attr("width", width)
      .attr("height", height)
      .attr("viewBox", [0, 0, width, height])
      .attr("style", "max-width: 100%; height: auto; height: intrinsic;");

  svg.append("g")
      .attr("transform", `translate(${marginLeft},0)`)
      .call(yAxis)
      .call(g => g.select(".domain").remove())
      .call(g => g.selectAll(".tick line").clone()
          .attr("x2", width - marginLeft - marginRight)
          .attr("stroke-opacity", 0.1))
      .call(g => g.append("text")
          .attr("x", -marginLeft)
          .attr("y", 10)
          .attr("fill", "currentColor")
          .attr("text-anchor", "start")
          .text(yLabel));

  const bar = svg.append("g")
    .selectAll("rect")
    .data(I)
    .join("rect")
      .attr("x", i => xScale(X[i]) + xzScale(Z[i]))
      .attr("y", i => yScale(Y[i]))
      .attr("width", xzScale.bandwidth())
      .attr("height", i => yScale(0) - yScale(Y[i]))
      .attr("fill", i => zScale(Z[i]));

  if (title) bar.append("title")
      .text(title);

  svg.append("g")
      .attr("transform", `translate(0,${height - marginBottom})`)
      .call(xAxis);

  return Object.assign(svg.node(), {scales: {color: zScale}});
}