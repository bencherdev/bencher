import { Reports } from "ui";
const queryString = require('query-string');


const pre = document.getElementById("bencher-chart");
const reports = Reports.new();

const renderLoop = () => {
    pre.textContent = reports.render();
    requestAnimationFrame(renderLoop);
};

requestAnimationFrame(renderLoop);

const parsed = queryString.parse(location.search);
console.log(parsed);
