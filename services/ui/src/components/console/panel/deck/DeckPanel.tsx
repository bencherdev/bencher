import axios from "axios";
import { createSignal, createResource, createMemo } from "solid-js";

import DeckHeader from "./DeckHeader";
import Deck from "./Deck";
import { getToken, validate_jwt } from "../../../site/util";
import validator from "validator";

const DeckPanel = (props) => {
  const url = createMemo(() => props.config?.deck?.url(props.path_params()));

  const [refresh, setRefresh] = createSignal(0);
  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  const options = () => {
    return {
      url: url(),
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${props.user()?.token}`,
      },
    };
  };

  const fetchData = async () => {
    const EMPTY_OBJECT = {};

    try {
      if (!validate_jwt(props.user()?.token)) {
        return EMPTY_OBJECT;
      }

      let reports = await axios(options());
      return reports.data;
    } catch (error) {
      console.error(error);
      return EMPTY_OBJECT;
    }
  };

  const [deck_data] = createResource(refresh, fetchData);

  return (
    <>
      <DeckHeader
        config={props.config?.header}
        data={deck_data()}
        handleRefresh={handleRefresh}
      />
      <Deck
        user={props.user}
        config={props.config?.deck}
        data={deck_data()}
        url={url}
        path_params={props.path_params}
        handleRefresh={handleRefresh}
      />
    </>
  );
};

export default DeckPanel;
