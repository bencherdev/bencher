import { Reports } from "ui";
const queryString = require('query-string');
import * as d3 from "d3";


const pre = document.getElementById("bencher-chart");

const reports_arg = queryString.parse(location.search).reports;
if (reports_arg) {
    // pre.textContent = reports_arg;
    const reports = Reports.from_str(reports_arg);
    // console.log(reports.to_string());
    pre.textContent = reports.to_string();
}
