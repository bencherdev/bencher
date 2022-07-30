import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

import DeckHeader from "./DeckHeader";
import Card from "./Card";

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
  const [deck_data] = createResource(props?.panel, fetchData);

  return (
    <>
      <DeckHeader title={deck_data()?.name} />
      <div class="columns">
        <div class="column">
          <div class="card">
            <Card field={"Project Name"} value={deck_data()?.name} />
          </div>
        </div>
      </div>
      <div class="columns">
        <div class="column">
          <div class="card">
            <Card field={"Project Slug"} value={deck_data()?.slug} />
          </div>
        </div>
      </div>
    </>
  );
};

export default DeckPanel;
