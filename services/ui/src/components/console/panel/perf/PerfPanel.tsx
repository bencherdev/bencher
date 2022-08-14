import axios from "axios";
import { useSearchParams } from "solid-app-router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
} from "solid-js";
import { isPerfKind, PerKind } from "../../config/types";
import PerfHeader from "./PerfHeader";
import PerfPlot from "./plot/PerfPlot";

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

const addToAray = (array: any[], add: any) => {
  array.push(add);
  return array;
};
const removeFromAray = (array: any[], remove: any) => {
  const index = array.indexOf(remove);
  if (index > -1) {
    array.splice(index, 1);
  }
  return array;
};

const arrayFromString = (array_str: undefined | string) => {
  if (typeof array_str === "string") {
    const array = array_str.split(",");
    return removeFromAray(array, "");
  }
  return [];
};
const arrayToString = (array: any[]) => array.join();

const dateToDateTime = (date_str: string) => {
  const date_ms = Date.parse(date_str);
  if (date_ms) {
    const date = new Date(date_ms);
    if (date) {
      return date.toISOString();
    }
  }
  return null;
};

const BRANCHES_PARAM = "branches";
const TESTBEDS_PARAM = "testbeds";
const BENCHMARKS_PARAM = "benchmarks";
const KIND_PARAM = "kind";
const START_TIME_PARAM = "start_time";
const END_TIME_PARAM = "end_time";

const DEFAULT_PEF_KIND = PerKind.LATENCY;

const PerfPanel = (props) => {
  const [searchParams, setSearchParams] = useSearchParams();

  if (!Array.isArray(arrayFromString(searchParams[BRANCHES_PARAM]))) {
    setSearchParams({ [BRANCHES_PARAM]: null });
  }
  if (!Array.isArray(arrayFromString(searchParams[TESTBEDS_PARAM]))) {
    setSearchParams({ [TESTBEDS_PARAM]: null });
  }
  if (!Array.isArray(arrayFromString(searchParams[BENCHMARKS_PARAM]))) {
    setSearchParams({ [BENCHMARKS_PARAM]: null });
  }
  if (!isPerfKind(searchParams[KIND_PARAM])) {
    setSearchParams({ [KIND_PARAM]: DEFAULT_PEF_KIND });
  }
  if (!Number.isSafeInteger(searchParams[START_TIME_PARAM])) {
    setSearchParams({ [START_TIME_PARAM]: null });
  }
  if (!Number.isSafeInteger(searchParams[END_TIME_PARAM])) {
    setSearchParams({ [END_TIME_PARAM]: null });
  }

  const branches = createMemo(() =>
    arrayFromString(searchParams[BRANCHES_PARAM])
  );
  const testbeds = createMemo(() =>
    arrayFromString(searchParams[TESTBEDS_PARAM])
  );
  const benchmarks = createMemo(() =>
    arrayFromString(searchParams[BENCHMARKS_PARAM])
  );
  const kind = createMemo(() => {
    if (isPerfKind(searchParams[KIND_PARAM])) {
      return searchParams[KIND_PARAM];
    } else {
      return DEFAULT_PEF_KIND;
    }
  });
  const start_time = createMemo(() => searchParams[START_TIME_PARAM]);
  const end_time = createMemo(() => searchParams[END_TIME_PARAM]);

  const addToArrayParam = (param: string, array: any[], add: string) =>
    setSearchParams({ [param]: arrayToString(addToAray(array, add)) });
  const removeFromArrayParam = (param: string, array: any[], remove: string) =>
    setSearchParams({
      [param]: arrayToString(removeFromAray(array, remove)),
    });

  const addBranch = (branch: string) =>
    addToArrayParam(BRANCHES_PARAM, branches(), branch);
  const removeBranch = (branch: string) =>
    removeFromArrayParam(BRANCHES_PARAM, branches(), branch);
  const handleKind = (kind: PerKind) => setSearchParams({ [KIND_PARAM]: kind });
  const handleStartTime = (date: string) =>
    setSearchParams({ [START_TIME_PARAM]: dateToDateTime(date) });
  const handleEndTime = (date: string) =>
    setSearchParams({ [END_TIME_PARAM]: dateToDateTime(date) });

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
    console.log(perf_query());
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
      <PerfPlot
        query={perf_query()}
        handleKind={handleKind}
        handleStartTime={handleStartTime}
        handleEndTime={handleEndTime}
      />
    </>
  );
};

export default PerfPanel;
