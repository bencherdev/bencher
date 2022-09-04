import axios from "axios";
import { createEffect, createResource, createSignal, For } from "solid-js";
import { PerfTab } from "../../../config/types";
import * as d3 from "d3";
import { getToken } from "../../../../site/util";
import validator from "validator";

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

    const token = getToken();
    if (token && !validator.isJWT(token)) {
      return key_data;
    }

    await Promise.all(
      uuids.map(async (uuid: string) => {
        try {
          let resp = await axios(key_data_options(token, perf_tab, uuid));
          const data = resp.data;
          key_data[uuid] = data;
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
          perf_active={props.perf_active}
          handleKey={props.handleKey}
          handlePerfActive={props.handlePerfActive}
        />
      ) : (
        <MinimizedKey
          perf_data={props.perf_data}
          perf_active={props.perf_active}
          handleKey={props.handleKey}
          handlePerfActive={props.handlePerfActive}
        />
      )}
    </>
  );
};

const ExpandedKey = (props) => {
  return (
    <>
      <MinimizeKeyButton handleKey={props.handleKey} />
      <br />
      <For each={props.perf_data()?.benchmarks}>
        {(
          perf: {
            branch: string;
            testbed: string;
            benchmark: string;
          },
          index
        ) => (
          <>
            {index() !== 0 && <hr class="is-primary" />}
            <div class="content">
              <KeyResource
                icon="fas fa-code-branch"
                name={props.branches()?.[perf.branch]?.name}
              />
              <KeyResource
                icon="fas fa-server"
                name={props.testbeds()?.[perf.testbed]?.name}
              />
              <KeyResource
                icon="fas fa-tachometer-alt"
                name={props.benchmarks()?.[perf.benchmark]?.name}
              />
            </div>
            <KeyButton
              index={index}
              perf_active={props.perf_active}
              handlePerfActive={props.handlePerfActive}
            />
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
      <For each={props.perf_data()?.benchmarks}>
        {(_perf, index) => (
          <KeyButton
            index={index}
            perf_active={props.perf_active}
            handlePerfActive={props.handlePerfActive}
          />
        )}
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
        props.perf_active[props.index()]
          ? `background-color:${color};`
          : `border-color:${color};color:${color};`
      }
      onClick={() => props.handlePerfActive(props.index())}
    >
      {props.index() + 1}
    </button>
  );
};

export default PlotKey;
