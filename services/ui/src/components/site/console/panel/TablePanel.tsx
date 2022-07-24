import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const options = {
  url: `${BENCHER_API_URL}/v0/reports`,
  method: "get",
  headers: {
    "Content-Type": "application/json",
    // Only use with explicit CORS
    // Authorization: `Bearer ${window.localStorage.authToken}`
  },
};

const fetchData = async (page) => {
  try {
    let reports = await axios(options);
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
  handleRedirect(`/console/reports/${datum?.uuid}`);
};

const TablePanel = (props) => {
  const [page, setPage] = createSignal(1);
  const [table_data] = createResource(page, fetchData);

  return (
    <Suspense fallback={<p>Loading...</p>}>
      <div class="pricing-table is-horizontal">
        <For each={table_data()}>
          {(datum, i) => (
            <div class="pricing-plan is-warning">
              <div class="plan-header">{getDate(datum)}</div>
              <div class="plan-items">
                <div class="plan-item">{datum?.project}</div>
                <div class="plan-item">{datum?.adapter_uuid}</div>
                <div class="plan-item">
                  {datum?.testbed_uuid || "No testbed"}
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
