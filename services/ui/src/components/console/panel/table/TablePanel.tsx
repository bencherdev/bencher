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
import Table from "./Table";

import TableHeader from "./TableHeader";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const TablePanel = (props) => {
  const options = (token: string) => {
    return {
      url: props.config?.table?.url(props.path_params()),
      method: "GET",
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
  const [page, setPage] = createSignal(1);
  const [table_data] = createResource(refresh, fetchData);

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  return (
    <>
      <TableHeader
        config={props.config?.header}
        pathname={props.pathname}
        refresh={refresh}
        handleRedirect={props.handleRedirect}
        handleRefresh={handleRefresh}
      />
      <Table
        config={props.config?.table}
        pathname={props.pathname}
        table_data={table_data}
        handleRedirect={props.handleRedirect}
      />
    </>
  );
};

export default TablePanel;
