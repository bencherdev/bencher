import { For } from "solid-js";
import { PerfTab } from "../../../config/types";
import { toCapitalized } from "../../../config/util";

const perf_tabs = [PerfTab.BRANCHES, PerfTab.TESTBEDS, PerfTab.BENCHMARKS];

const PlotTab = (props) => {
  const getTab = () => {
    switch (props.tab()) {
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

  const handleChecked = (index: number, uuid: string) => {
    switch (props.tab()) {
      case PerfTab.BRANCHES:
        return props.handleBranchChecked(index, uuid);
      case PerfTab.TESTBEDS:
        return props.handleTestbedChecked(index, uuid);
      case PerfTab.BENCHMARKS:
        return props.handleBenchmarkChecked(index, uuid);
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
              class={props.tab() === tab ? "is-active" : ""}
              onClick={() => props.handleTab(tab)}
            >
              {toCapitalized(tab)}
            </a>
          )}
        </For>
      </p>
      <For each={getTab()}>
        {(item, index) => (
          // TODO maybe move over to a store and see if this works?
          // https://www.solidjs.com/tutorial/stores_createstore
          <a
            class="panel-block"
            onSelect={(e) => handleChecked(index(), item.uuid)}
          >
            <input
              type="checkbox"
              checked={item.checked}
              onChange={() => handleChecked(index(), item.uuid)}
            />
            {item.name}
          </a>
        )}
      </For>
    </>
  );
};

export default PlotTab;
