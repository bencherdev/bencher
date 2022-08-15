import { For } from "solid-js";
import { PerfTab, PerKind } from "../../../config/types";
import { toCapitalized } from "../../../config/util";

const Plot = (props) => {
  return (
    <div>
      {props.isPlotInit() && (
        <PlotInit
          branches={props.branches}
          testbeds={props.testbeds}
          benchmarks={props.benchmarks}
          handleTab={props.handleTab}
        />
      )}
    </div>
  );
};

const PlotInit = (props) => {
  return (
    <div class="content">
      <ul>
        <li class="checkbox">
          <input
            type="checkbox"
            checked={props.branches().length > 0}
            disabled={true}
          />
          Select at least one{" "}
          <a
            onClick={() => {
              props.handleTab(PerfTab.BRANCHES);
            }}
          >
            Branch
          </a>
        </li>
        <br />
        <li class="checkbox">
          <input
            type="checkbox"
            checked={props.testbeds().length > 0}
            disabled={true}
          />
          Select at least one{" "}
          <a
            onClick={() => {
              props.handleTab(PerfTab.TESTBEDS);
            }}
          >
            Testbed
          </a>
        </li>
        <br />
        <li class="checkbox">
          <input
            type="checkbox"
            checked={props.benchmarks().length > 0}
            disabled={true}
          />
          Select at least one{" "}
          <a
            onClick={() => {
              props.handleTab(PerfTab.BENCHMARKS);
            }}
          >
            Benchmark
          </a>
        </li>
      </ul>
      <br />
    </div>
  );
};

export default Plot;
