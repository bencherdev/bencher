import { panel } from './panel/mod.js';
import { chart } from "./chart/mod.js";
import { get_reports } from "./reports/mod.js"

main();

function main() {
  const reports = get_reports();

  if (reports === undefined) {
    return;
  }

  panel();
  chart(reports);
}

