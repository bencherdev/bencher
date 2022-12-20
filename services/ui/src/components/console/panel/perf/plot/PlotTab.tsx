import { Link } from "solid-app-router";
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
      {getTab().length === 0 ? (
        <div class="box">
          <AddButton project_slug={props.project_slug} tab={props.tab} />
        </div>
      ) : (
        <For each={getTab()}>
          {(item, index) => (
            // TODO maybe move over to a store and see if this works?
            // https://www.solidjs.com/tutorial/stores_createstore
            <a
              class="panel-block"
              style="overflow-wrap:break-word;"
              onClick={(e) => handleChecked(index(), item.uuid)}
            >
              <input type="checkbox" checked={item.checked} />
              {item.name}
            </a>
          )}
        </For>
      )}
    </>
  );
};

const AddButton = (props) => {
  const getHref = () => {
    switch (props.tab()) {
      case PerfTab.BRANCHES:
      case PerfTab.TESTBEDS:
        return `/console/projects/${props.project_slug()}/${props.tab()}/add`;
      case PerfTab.BENCHMARKS:
        return "/docs/how-to/quick-start";
      default:
        return "#";
    }
  };

  const getText = () => {
    switch (props.tab()) {
      case PerfTab.BRANCHES:
        return "Create a Branch";
      case PerfTab.TESTBEDS:
        return "Create a Testbed";
      case PerfTab.BENCHMARKS:
        return "Run a Report";
      default:
        return "Ello Poppet";
    }
  };

  return (
    <Link class="button is-primary is-fullwidth" href={getHref()}>
      {getText()}
    </Link>
  );
};

export default PlotTab;
