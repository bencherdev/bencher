import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const options = (token: string, slug: string) => {
  return {
    url: `${BENCHER_API_URL}/v0/projects/${slug}`,
    method: "get",
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${token}`,
    },
  };
};

const fetchData = async (panel) => {
  try {
    const token = JSON.parse(window.localStorage.getItem("user"))?.uuid;
    if (typeof token !== "string") {
      return;
    }
    let reports = await axios(options(token, panel?.slug));
    console.log(reports);
    return reports.data;
  } catch (error) {
    console.error(error);
  }
};

const DeckPanel = (props) => {
  const [deck_data] = createResource(props.panel, fetchData);

  return (
    <div class="columns">
      <div class="column">
        <div class="card">
          <div class="card-header">
            <div class="card-header-title">Report field</div>
          </div>
          <div class="card-content">
            <div class="content">field value</div>
          </div>
          <div class="card-footer">
            <div class="card-footer-item">Update</div>
          </div>
        </div>
      </div>
    </div>
  );
};

export default DeckPanel;
