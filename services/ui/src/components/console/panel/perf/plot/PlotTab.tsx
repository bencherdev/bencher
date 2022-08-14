import { For } from "solid-js";
import { PerfTab } from "../../../config/types";
import { toCapitalized } from "../../../config/util";

const perf_tabs = [PerfTab.BRANCHES, PerfTab.TESTBEDS, PerfTab.BENCHMARKS];

const PlotTab = (props) => {
  return (
    <>
      <p class="panel-tabs">
        <For each={perf_tabs}>
          {(tab) => (
            <a
              class={props.perf_tab() === tab ? "is-active" : ""}
              onClick={(e) => props.handlePerfTab(tab)}
            >
              {toCapitalized(tab)}
            </a>
          )}
        </For>
      </p>
      <For each={perf_tabs}>
        {(item) => (
          <a class="panel-block">
            <input type="checkbox" />
            {toCapitalized(item)}
          </a>
        )}
      </For>
    </>
  );
};

export default PlotTab;
