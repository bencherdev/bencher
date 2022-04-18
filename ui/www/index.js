import { Reports } from "ui";
const queryString = require('query-string');
import * as d3 from "d3";


const d3_chart = document.getElementById("d3-chart");

const reports_arg = queryString.parse(location.search).reports;
var data;
var chart;
if (false && reports_arg) {
  const reports = Reports.from_str(reports_arg);
  const latency = reports.latency("tests::benchmark_a");
  data = JSON.parse(latency);

  chart = BarChart(data, {
    x: d => d.date_time,
    y: d => d.duration,
    yLabel: "Date and Time",
    yLabel: "Time (ns)",
    width: 640,
    height: 500,
    color: "steelblue"
  });
} else {
  data = [
    {"state":"AL","age":"<10","population":598478},
    {"state":"AK","age":"<10","population":106741},
    {"state":"AZ","age":"<10","population":892083},
    {"state":"AR","age":"<10","population":392177},
    {"state":"AL","age":"10-19","population":638789},
    {"state":"AK","age":"10-19","population":99926},
    {"state":"AZ","age":"10-19","population":912735},
    {"state":"AR","age":"10-19","population":397185},
    {"state":"AL","age":"20-29","population":661666},
    {"state":"AK","age":"20-29","population":120674},
    {"state":"AZ","age":"20-29","population":939804},
    {"state":"AR","age":"20-29","population":399698},
    {"state":"AL","age":"30-39","population":603013},
    {"state":"AK","age":"30-39","population":102008},
    {"state":"AZ","age":"30-39","population":857054},
    {"state":"AR","age":"30-39","population":372998},
    {"state":"AL","age":"40-49","population":625599},
    {"state":"AK","age":"40-49","population":91539},
    {"state":"AZ","age":"40-49","population":833290},
    {"state":"AR","age":"40-49","population":370157},
    {"state":"AL","age":"50-59","population":673864},
    {"state":"AK","age":"50-59","population":104569},
    {"state":"AZ","age":"50-59","population":834858},
    {"state":"AR","age":"50-59","population":395070},
    {"state":"AL","age":"60-69","population":548376},
    {"state":"AK","age":"60-69","population":70473},
    {"state":"AZ","age":"60-69","population":737884},
    {"state":"AR","age":"60-69","population":329734},
    {"state":"AL","age":"70-79","population":316598},
    {"state":"AK","age":"70-79","population":28422},
    {"state":"AZ","age":"70-79","population":466153},
    {"state":"AR","age":"70-79","population":197985},
    {"state":"AL","age":"≥80","population":174781},
    {"state":"AK","age":"≥80","population":12503},
    {"state":"AZ","age":"≥80","population":254716},
    {"state":"AR","age":"≥80","population":113468},
  ];

  var ages = ["<10","10-19","20-29","30-39","40-49","50-59","60-69","70-79","≥80"];

  chart = GroupedBarChart(data, {
    x: d => d.state,
    y: d => d.population / 1e6,
    z: d => d.age,
    xDomain: d3.groupSort(data, D => d3.sum(D, d => -d.population), d => d.state).slice(0, 6), // top 6
    yLabel: "↑ Population (millions)",
    zDomain: ages,
    colors: d3.schemeSpectral[ages.length],
    width: 640,
    height: 500
  })
}


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