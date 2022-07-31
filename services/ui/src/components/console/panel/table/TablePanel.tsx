import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
  Switch,
  Match,
} from "solid-js";
import { Row } from "../../console";

import TableHeader from "./TableHeader";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const options = (token: string) => {
  return {
    url: `${BENCHER_API_URL}/v0/projects`,
    method: "get",
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

const getDate = (datum) => {
  const date = new Date(datum?.start_time);
  return date.toUTCString();
};

const handleRowButton = (event, datum, handleRedirect) => {
  event.preventDefault();
  handleRedirect(`/console/projects/${datum?.slug}`);
};

const TablePanel = (props) => {
  const [refresh, setRefresh] = createSignal(0);
  const [page, setPage] = createSignal(1);
  const [table_data] = createResource(refresh, fetchData);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  return (
    <>
      <TableHeader
        config={props.config?.header}
        refresh={refresh}
        handleRedirect={props.handleRedirect}
        handleRefresh={handleRefresh}
      />
      <Suspense fallback={<p>Loading...</p>}>
        <div class="pricing-table is-horizontal">
          <For each={table_data()}>
            {(datum, i) => (
              <div class="pricing-plan is-primary">
                <div class="plan-header">{datum[props.config?.row?.key]}</div>
                <div class="plan-items">
                  <For each={props.config?.row?.items}>
                    {(item, i) => (
                      <div class="plan-item">
                        <Switch fallback="-">
                          <Match when={item.kind === Row.TEXT}>
                            {datum[item.key]}
                          </Match>
                          <Match when={item.kind === Row.BOOL}>
                            {item.text}: {datum[item.key] ? "true" : "false"}
                          </Match>
                        </Switch>
                      </div>
                    )}
                  </For>
                </div>
                <div class="plan-footer">
                  <button
                    class="button is-fullwidth"
                    onClick={(e) =>
                      handleRowButton(e, datum, props.handleRedirect)
                    }
                  >
                    View
                  </button>
                </div>
              </div>
            )}
          </For>
        </div>
      </Suspense>
    </>
  );
};

export default TablePanel;
