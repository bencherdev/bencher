import { Reports } from "ui";

const pre = document.getElementById("bencher-chart");
const reports = Reports.new();

const renderLoop = () => {
    pre.textContent = reports.render();
    requestAnimationFrame(renderLoop);
};

requestAnimationFrame(renderLoop);
