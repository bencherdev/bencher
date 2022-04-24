import { panel } from './panel/mod.js';
import { chart } from "./chart/mod.js";
import { get_reports } from "./reports/mod.js"

main();

function main() {
  const reports = get_reports();
  if (reports === undefined) {
    return;
  }

  const latency = reports.latency();
  const inventory = latency.inventory();
  const data = latency.data();

  panel(inventory);
  chart(inventory, data);
}

