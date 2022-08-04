import axios from "axios";
import {
  createSignal,
  createResource,
  createEffect,
  Suspense,
  For,
} from "solid-js";

import DeckHeader from "./DeckHeader";
import Deck from "./Deck";
import Card from "./Card";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const DeckPanel = (props) => {
  const [refresh, setRefresh] = createSignal(0);

  const options = (token: string) => {
    return {
      url: props.config?.deck?.url(props.path_params()),
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${token}`,
      },
    };
  };

  const fetchData = async () => {
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

  const [deck_data] = createResource(refresh, fetchData);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  return (
    <>
      <DeckHeader
        config={props.config?.header}
        data={deck_data()}
        pathname={props.pathname}
        handleRedirect={props.handleRedirect}
        handleRefresh={handleRefresh}
      />
      <Deck
        config={props.config?.deck}
        data={deck_data()}
        pathname={props.pathname}
        handleRedirect={props.handleRedirect}
      />
    </>
  );
};

export default DeckPanel;
