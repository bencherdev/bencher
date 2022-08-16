import axios from "axios";
import { createEffect, createResource, createSignal, For } from "solid-js";
import { PerfTab } from "../../../config/types";
import * as d3 from "d3";

const PlotKey = (props) => {
  const key_data_options = (token: string, perf_tab: PerfTab, uuid: string) => {
    return {
      url: props.config?.key_url(props.path_params(), perf_tab, uuid),
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
    };
  };

  const fetchKeyData = async (perf_tab: PerfTab, uuids: any[]) => {
    const key_data = {};
    console.log(uuids);

    await Promise.all(
      uuids.map(async (uuid: string) => {
        try {
          console.log(uuid);
          const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
          // Don't even send query if there isn't at least one: branch, testbed, and benchmark
          if (typeof token !== "string") {
            return;
          }
          let resp = await axios(key_data_options(token, perf_tab, uuid));
          const data = resp.data;
          key_data[uuid] = data;
          console.log(data);
        } catch (error) {
          console.error(error);
        }
      })
    );

    return key_data;
  };

  const [branches] = createResource(props.branches, async (uuids) => {
    return fetchKeyData(PerfTab.BRANCHES, uuids);
  });
  const [testbeds] = createResource(props.testbeds, async (uuids) => {
    return fetchKeyData(PerfTab.TESTBEDS, uuids);
  });
  const [benchmarks] = createResource(props.benchmarks, async (uuids) => {
    return fetchKeyData(PerfTab.BENCHMARKS, uuids);
  });

  return (
    <>
      {props.key() ? (
        <ExpandedKey
          branches={branches}
          testbeds={testbeds}
          benchmarks={benchmarks}
          perf_data={props.perf_data}
          handleKey={props.handleKey}
        />
      ) : (
        <MinimizedKey perf_data={props.perf_data} handleKey={props.handleKey} />
      )}
    </>
  );
};

const ExpandedKey = (props) => {
  return (
    <>
      <MinimizeKeyButton handleKey={props.handleKey} />
      <br />
      <For each={props.perf_data()?.perf}>
        {(
          perf: {
            branch_uuid: string;
            testbed_uuid: string;
            benchmark_uuid: string;
          },
          index
        ) => (
          <>
            {index() !== 0 && <hr class="is-primary" />}
            <div class="content">
              <KeyResource
                icon="fas fa-code-branch"
                name={props.branches()?.[perf.branch_uuid]?.name}
              />
              <KeyResource
                icon="fas fa-server"
                name={props.testbeds()?.[perf.testbed_uuid]?.name}
              />
              <KeyResource
                icon="fas fa-tachometer-alt"
                name={props.benchmarks()?.[perf.benchmark_uuid]?.name}
              />
            </div>
            <KeyButton index={index} on={index() % 2 === 0} />
          </>
        )}
      </For>
      <br />
      <MinimizeKeyButton handleKey={props.handleKey} />
    </>
  );
};

const MinimizedKey = (props) => {
  return (
    <>
      <MaximizeKeyButton handleKey={props.handleKey} />
      <br />
      <For each={props.perf_data()?.perf}>
        {(_perf, index) => <KeyButton index={index} on={index() % 2 === 0} />}
      </For>
      <br />
      <MaximizeKeyButton handleKey={props.handleKey} />
    </>
  );
};

const MinimizeKeyButton = (props) => {
  return (
    <button
      class="button is-small is-fullwidth is-primary is-inverted"
      onClick={() => props.handleKey(false)}
    >
      <span class="icon">
        <i class="fas fa-less-than" aria-hidden="true" />
      </span>
    </button>
  );
};

const MaximizeKeyButton = (props) => {
  return (
    <button
      class="button is-small is-fullwidth is-primary is-inverted"
      onClick={() => props.handleKey(true)}
    >
      <span class="icon">
        <i class="fas fa-greater-than" aria-hidden="true" />
      </span>
    </button>
  );
};

const KeyResource = (props) => {
  return (
    <nav class="level is-mobile">
      <div class="level-left">
        <div class="level-item">
          <span class="icon">
            <i class={props.icon} aria-hidden="true" />
          </span>
        </div>
        <div class="level-item">{props.name}</div>
      </div>
    </nav>
  );
};

const KeyButton = (props) => {
  const color = d3.schemeTableau10[props.index() % 10];

  return (
    <button
      // On click toggle visibility of perf
      // move button over to being is-outlined
      class="button is-small is-fullwidth"
      style={
        props.on
          ? `background-color:${color};`
          : `border-color:${color};color:${color};`
      }
      // onClick={() => props.handleKey(false)}
    >
      {props.index() + 1}
    </button>
  );
};

export default PlotKey;
