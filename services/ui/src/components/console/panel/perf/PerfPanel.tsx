import axios from "axios";
import { useSearchParams } from "solid-app-router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
} from "solid-js";
import { isPerfKind, isPerfTab, PerfTab, PerKind } from "../../config/types";
import PerfHeader from "./PerfHeader";
import PerfPlot from "./plot/PerfPlot";

const BRANCHES_PARAM = "branches";
const TESTBEDS_PARAM = "testbeds";
const BENCHMARKS_PARAM = "benchmarks";
const KIND_PARAM = "kind";
const START_TIME_PARAM = "start_time";
const END_TIME_PARAM = "end_time";

const DEFAULT_PEF_KIND = PerKind.LATENCY;
const DEFAULT_PERF_TAB = PerfTab.BRANCHES;

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

const dateToISO = (date_str: undefined | string) => {
  if (typeof date_str === "string") {
    const date_ms = Date.parse(date_str);
    if (date_ms) {
      const date = new Date(date_ms);
      if (date) {
        return date.toISOString();
      }
    }
  }
  return null;
};
const ISOToDate = (iso_str: undefined | string) => {
  if (typeof iso_str === "string") {
    return iso_str.split("T")?.[0];
  }
  return null;
};

const PerfPanel = (props) => {
  const [searchParams, setSearchParams] = useSearchParams();

  // Sanitize all query params
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
  if (!dateToISO(searchParams[START_TIME_PARAM])) {
    setSearchParams({ [START_TIME_PARAM]: null });
  }
  if (!dateToISO(searchParams[END_TIME_PARAM])) {
    setSearchParams({ [END_TIME_PARAM]: null });
  }

  // Create memos of all query params
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
    // This check is required for the initial load
    // before the query params have been sanitized
    if (isPerfKind(searchParams[KIND_PARAM])) {
      return searchParams[KIND_PARAM];
    } else {
      return DEFAULT_PEF_KIND;
    }
  });
  const start_time = createMemo(() => searchParams[START_TIME_PARAM]);
  const start_date = createMemo(() => ISOToDate(start_time()));
  const end_time = createMemo(() => searchParams[END_TIME_PARAM]);
  const end_date = createMemo(() => ISOToDate(end_time()));

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
    setSearchParams({ [START_TIME_PARAM]: dateToISO(date) });
  const handleEndTime = (date: string) =>
    setSearchParams({ [END_TIME_PARAM]: dateToISO(date) });

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

  const perf_data_options = (token: string) => {
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

  const fetchPerfData = async () => {
    try {
      const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
      if (typeof token !== "string") {
        return;
      }
      let resp = await axios(perf_data_options(token));
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
  const [perf_data] = createResource(perf_query_refresh, fetchPerfData);

  const perf_tab_options = (token: string, perf_tab: PerfTab) => {
    return {
      url: props.config?.plot?.tab_url(props.path_params(), perf_tab),
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
    };
  };

  const fetchPerfTab = async (perf_tab: PerfTab) => {
    try {
      const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
      if (typeof token !== "string") {
        return;
      }
      let resp = await axios(perf_tab_options(token, perf_tab));
      const data = resp.data;
      console.log(data);
      return data;
    } catch (error) {
      console.error(error);
    }
  };

  const [branches_data] = createResource(refresh, async () => {
    return fetchPerfTab(PerfTab.BRANCHES);
  });
  const [testbeds_data] = createResource(refresh, async () => {
    return fetchPerfTab(PerfTab.TESTBEDS);
  });
  const [benchmarks_data] = createResource(refresh, async () => {
    return fetchPerfTab(PerfTab.BENCHMARKS);
  });

  const [branches_tab, setBranchesTab] = createSignal(branches_data());

  const handleBranchesTab = () => {
    setBranchesTab(branches_data());
  };

  const [tabular_refresh, setTabularRefresh] = createSignal();

  createEffect(() => {
    if (
      // At init wait until data is loaded to set tabular_refresh
      (!tabular_refresh() &&
        branches_data() &&
        testbeds_data() &&
        benchmarks_data()) ||
      // If refresh is later triggered also update data
      (tabular_refresh() && tabular_refresh() !== refresh())
    ) {
      setTabularRefresh(refresh());

      handleBranchesTab();
    }
  });

  const [perf_tab, setPerfTab] = createSignal(DEFAULT_PERF_TAB);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  const handlePerfTab = (tab: PerfTab) => {
    if (isPerfTab(tab)) {
      setPerfTab(tab);
    }
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
        start_date={start_date}
        end_date={end_date}
        perf_tab={perf_tab}
        branches_tab={branches_tab}
        handleKind={handleKind}
        handleStartTime={handleStartTime}
        handleEndTime={handleEndTime}
        handlePerfTab={handlePerfTab}
        handleBranchesTab={handleBranchesTab}
      />
    </>
  );
};

export default PerfPanel;
