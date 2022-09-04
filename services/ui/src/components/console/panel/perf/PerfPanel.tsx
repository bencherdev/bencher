import axios from "axios";
import { useSearchParams } from "solid-app-router";
import {
  createEffect,
  createMemo,
  createResource,
  createSignal,
} from "solid-js";
import { isPerfKind, isPerfTab, PerfTab, PerKind } from "../../config/types";
import { LOCAL_USER_KEY } from "../../config/util";
import PerfHeader from "./PerfHeader";
import PerfPlot from "./plot/PerfPlot";
import validator from "validator";

const BRANCHES_PARAM = "branches";
const TESTBEDS_PARAM = "testbeds";
const BENCHMARKS_PARAM = "benchmarks";
const KIND_PARAM = "kind";
const START_TIME_PARAM = "start_time";
const END_TIME_PARAM = "end_time";

const TAB_PARAM = "tab";
const KEY_PARAM = "key";

const DEFAULT_PERF_KIND = PerKind.LATENCY;
const DEFAULT_PERF_TAB = PerfTab.BRANCHES;
const DEFAULT_PERF_KEY = true;

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

const resourcesToCheckable = (resources, params) =>
  resources.map((resource) => {
    return {
      uuid: resource?.uuid,
      name: resource?.name,
      checked: params.indexOf(resource?.uuid) > -1,
    };
  });

const PerfPanel = (props) => {
  const [searchParams, setSearchParams] = useSearchParams();

  // Sanitize all query params at init
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
    setSearchParams({ [KIND_PARAM]: DEFAULT_PERF_KIND });
  }
  if (!dateToISO(searchParams[START_TIME_PARAM])) {
    setSearchParams({ [START_TIME_PARAM]: null });
  }
  if (!dateToISO(searchParams[END_TIME_PARAM])) {
    setSearchParams({ [END_TIME_PARAM]: null });
  }

  // Sanitize all UI state query params
  if (!isPerfTab(searchParams[TAB_PARAM])) {
    setSearchParams({ [TAB_PARAM]: null });
  }
  if (
    searchParams[KEY_PARAM] !== "false" &&
    searchParams[KEY_PARAM] !== "true"
  ) {
    setSearchParams({ [KEY_PARAM]: DEFAULT_PERF_KEY });
  }

  // Create marshalized memos of all query params
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
      return DEFAULT_PERF_KIND;
    }
  });
  // start/end_time is used for the query
  const start_time = createMemo(() => searchParams[START_TIME_PARAM]);
  const end_time = createMemo(() => searchParams[END_TIME_PARAM]);
  // start/end_date is used for the GUI selector
  const start_date = createMemo(() => ISOToDate(start_time()));
  const end_date = createMemo(() => ISOToDate(end_time()));

  const tab = createMemo(() => {
    // This check is required for the initial load
    // before the query params have been sanitized
    if (isPerfTab(searchParams[TAB_PARAM])) {
      return searchParams[TAB_PARAM];
    } else {
      return DEFAULT_PERF_TAB;
    }
  });

  const key = createMemo(() => {
    // This check is required for the initial load
    // before the query params have been sanitized
    if (
      searchParams[KEY_PARAM] === "false" ||
      searchParams[KEY_PARAM] === "true"
    ) {
      return searchParams[KEY_PARAM] === "true";
    } else {
      return DEFAULT_PERF_KEY;
    }
  });

  // The perf query sent to the server
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

  const isPlotInit = () =>
    branches().length === 0 ||
    testbeds().length === 0 ||
    benchmarks().length === 0;

  const perf_data_options = (token: string) => {
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
      const token = JSON.parse(
        window.localStorage.getItem(LOCAL_USER_KEY)
      )?.token;
      // Don't even send query if there isn't at least one: branch, testbed, and benchmark
      if (isPlotInit() || !validator.isJWT(token)) {
        return {};
      }

      let resp = await axios(perf_data_options(token));
      const data = resp.data;
      console.log(data);
      return data;
    } catch (error) {
      console.error(error);

      return {};
    }
  };

  // Refresh pref query
  const [refresh, setRefresh] = createSignal(0);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

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
      const token = JSON.parse(
        window.localStorage.getItem(LOCAL_USER_KEY)
      )?.token;
      if (!validator.isJWT(token)) {
        return [];
      }

      let resp = await axios(perf_tab_options(token, perf_tab));
      const data = resp.data;

      return data;
    } catch (error) {
      console.error(error);

      return [];
    }
  };

  const project_refresh = createMemo(() => {
    return {
      project_slug: props.path_params().project_slug,
      refresh: refresh(),
    };
  });

  // Resource tabs data: Branches, Testbeds, Benchmarks
  const [branches_data] = createResource(project_refresh, async () => {
    return fetchPerfTab(PerfTab.BRANCHES);
  });
  const [testbeds_data] = createResource(project_refresh, async () => {
    return fetchPerfTab(PerfTab.TESTBEDS);
  });
  const [benchmarks_data] = createResource(project_refresh, async () => {
    return fetchPerfTab(PerfTab.BENCHMARKS);
  });

  // Initialize as empty, wait for resources to load
  const [branches_tab, setBranchesTab] = createSignal([]);
  const [testbeds_tab, setTestbedsTab] = createSignal([]);
  const [benchmarks_tab, setBenchmarksTab] = createSignal([]);

  // Keep state on whether the resources have been refreshed
  const [tabular_refresh, setTabularRefresh] = createSignal();

  createEffect(() => {
    // At init wait until data is loaded to set *_tab and tabular_refresh
    // Otherwise, check to see if there is a new refresh
    if (
      (!tabular_refresh() &&
        branches_data() &&
        testbeds_data() &&
        benchmarks_data()) ||
      (tabular_refresh() && tabular_refresh() !== refresh())
    ) {
      setTabularRefresh(refresh());

      setBranchesTab(resourcesToCheckable(branches_data(), branches()));
      setTestbedsTab(resourcesToCheckable(testbeds_data(), testbeds()));
      setBenchmarksTab(resourcesToCheckable(benchmarks_data(), benchmarks()));
    }
  });

  const handleChecked = (
    resource_tab: any[],
    index: number,
    param: string,
    param_array: string[],
    uuid: string
  ) => {
    const tab = resource_tab;
    const checked = tab?.[index].checked;
    if (typeof checked !== "boolean") {
      return;
    }
    if (checked) {
      setSearchParams({
        [param]: arrayToString(removeFromAray(param_array, uuid)),
      });
    } else {
      setSearchParams({ [param]: arrayToString(addToAray(param_array, uuid)) });
    }
    tab[index].checked = !checked;
    return tab;
  };

  const handleBranchChecked = (index: number, uuid: string) => {
    setBranchesTab(
      handleChecked(branches_tab(), index, BRANCHES_PARAM, branches(), uuid)
    );
  };
  const handleTestbedChecked = (index: number, uuid: string) => {
    setTestbedsTab(
      handleChecked(testbeds_tab(), index, TESTBEDS_PARAM, testbeds(), uuid)
    );
  };
  const handleBenchmarkChecked = (index: number, uuid: string) => {
    setBenchmarksTab(
      handleChecked(
        benchmarks_tab(),
        index,
        BENCHMARKS_PARAM,
        benchmarks(),
        uuid
      )
    );
  };

  const handleKind = (kind: PerKind) => {
    if (isPerfKind(kind)) {
      setSearchParams({
        [KIND_PARAM]: kind,
      });
    }
  };
  const handleStartTime = (date: string) =>
    setSearchParams({ [START_TIME_PARAM]: dateToISO(date) });
  const handleEndTime = (date: string) =>
    setSearchParams({ [END_TIME_PARAM]: dateToISO(date) });

  const handleTab = (tab: PerfTab) => {
    if (isPerfTab(tab)) {
      setSearchParams({ [TAB_PARAM]: tab });
    }
  };

  const handleKey = (key: boolean) => {
    if (typeof key === "boolean") {
      setSearchParams({ [KEY_PARAM]: key });
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
        project_slug={props.project_slug}
        config={props.config?.plot}
        path_params={props.path_params}
        isPlotInit={isPlotInit}
        branches={branches}
        testbeds={testbeds}
        benchmarks={benchmarks}
        kind={kind}
        start_date={start_date}
        end_date={end_date}
        perf_data={perf_data}
        tab={tab}
        key={key}
        branches_tab={branches_tab}
        testbeds_tab={testbeds_tab}
        benchmarks_tab={benchmarks_tab}
        handleKind={handleKind}
        handleStartTime={handleStartTime}
        handleEndTime={handleEndTime}
        handleTab={handleTab}
        handleKey={handleKey}
        handleBranchChecked={handleBranchChecked}
        handleTestbedChecked={handleTestbedChecked}
        handleBenchmarkChecked={handleBenchmarkChecked}
      />
    </>
  );
};

export default PerfPanel;
