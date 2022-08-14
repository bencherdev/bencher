import { For } from "solid-js";
import { PerfTab } from "../../../config/types";
import { toCapitalized } from "../../../config/util";

const perf_tabs = [PerfTab.BRANCHES, PerfTab.TESTBEDS, PerfTab.BENCHMARKS];

const PlotTab = (props) => {
  const getTab = () => {
    switch (props.perf_tab()) {
      case PerfTab.BRANCHES:
        return props.branches_tab();
      case PerfTab.TESTBEDS:
        return props.testbeds_tab();
      case PerfTab.BENCHMARKS:
        return props.benchmarks_tab();
      default:
        return [];
    }
  };

  const handleChecked = (index: number) => {
    switch (props.perf_tab()) {
      case PerfTab.BRANCHES:
        return props.handleBranchChecked(index);
      case PerfTab.TESTBEDS:
        return props.handleTestbedChecked(index);
      case PerfTab.BENCHMARKS:
        return props.handleBenchmarkChecked(index);
      default:
        return [];
    }
  };

  return (
    <>
      <p class="panel-tabs">
        <For each={perf_tabs}>
          {(tab) => (
            <a
              class={props.perf_tab() === tab ? "is-active" : ""}
              onClick={() => props.handlePerfTab(tab)}
            >
              {toCapitalized(tab)}
            </a>
          )}
        </For>
      </p>
      <For each={getTab()}>
        {(item, index) => (
          <a class="panel-block" onSelect={(e) => handleChecked(index())}>
            <input
              type="checkbox"
              checked={item.checked}
              onInput={() => handleChecked(index())}
            />
            {item.name}
          </a>
        )}
      </For>
    </>
  );
};

export default PlotTab;
