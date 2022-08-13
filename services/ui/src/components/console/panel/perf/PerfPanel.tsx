import axios from "axios";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
} from "solid-js";
import { PerKind } from "../../config/types";
import PerfHeader from "./PerfHeader";
import PerfPlot from "./PerfPlot";

const initPerfQuery = () => {
  return {
    branches: [],
    testbeds: [],
    benchmarks: [],
    kind: "latency",
    start_time: null,
    end_time: null,
  };
};

const PerfPanel = (props) => {
  const [branches, setBranches] = createSignal([]);
  const [testbeds, setTestbeds] = createSignal([]);
  const [benchmarks, setBenchmarks] = createSignal([]);
  const [kind, setKind] = createSignal(PerKind.LATENCY);
  const [start_time, setStartTime] = createSignal(null);
  const [end_time, setEndTime] = createSignal(null);

  const perf_query = createMemo(() => {
    return {
      branches: branches(),
      testbeds: testbeds(),
      benchmarks: benchmarks(),
      kind: kind(),
      start_time: start_time(),
      end_time: end_time(),
    };
  });

  const options = (token: string) => {
    return {
      url: props.config?.plot?.url(),
      method: "POST",
      data: perf_query(),
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
    };
  };

  const fetchData = async (refresh) => {
    try {
      const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
      if (typeof token !== "string") {
        return;
      }
      let resp = await axios(options(token));
      const data = resp.data;
      console.log(data);
      return data;
    } catch (error) {
      console.error(error);
    }
  };

  const [refresh, setRefresh] = createSignal(0);
  const perf_query_refresh = createMemo(() => {
    return {
      perf_query: perf_query(),
      refresh: refresh(),
    };
  });
  const [perf_data] = createResource(perf_query_refresh, fetchData);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  return (
    <>
      <PerfHeader
        config={props.config?.header}
        refresh={refresh}
        handleTitle={props.handleTitle}
        handleRefresh={handleRefresh}
      />
      <PerfPlot query={perf_query()} handleKind={setKind} />
    </>
  );
};

export default PerfPanel;
