import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

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

const fetchData = async (page) => {
  try {
    const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
    if (typeof token !== "string") {
      return;
    }
    let reports = await axios(options(token));
    console.log(reports);
    return reports.data;
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
  const [page, setPage] = createSignal(1);
  const [table_data] = createResource(page, fetchData);

  return (
    <Suspense fallback={<p>Loading...</p>}>
      <TableHeader title={"Projects"} />
      <div class="pricing-table is-horizontal">
        <For each={table_data()}>
          {(datum, i) => (
            <div class="pricing-plan is-warning">
              <div class="plan-header">{datum.name}</div>
              <div class="plan-items">
                <div class="plan-item">{datum?.slug}</div>
                <div class="plan-item">-</div>
                <div class="plan-item">
                  Default: {datum.owner_default ? "true" : "false"}
                </div>
                <div class="plan-item">-</div>
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
  );
};

export default TablePanel;
