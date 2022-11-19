import axios from "axios";
import {
  createSignal,
  createResource,
  createMemo,
  createEffect,
} from "solid-js";
import Table from "./Table";
import validator from "validator";

import TableHeader from "./TableHeader";
import { getToken } from "../../../site/util";
import { useNavigate } from "solid-app-router";

const TablePanel = (props) => {
  const navigate = useNavigate();

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
      const token = getToken();
      if (token && !validator.isJWT(token)) {
        return [];
      }

      let resp = await axios(options(token));
      const data = resp.data;

      return data;
    } catch (error) {
      console.error(error);

      return [];
    }
  };

  const [refresh, setRefresh] = createSignal(0);
  const [page, setPage] = createSignal(1);
  const [table_data] = createResource(refresh, fetchData);

  const redirect = createMemo(() => props.config.redirect?.(table_data()));

  const handleRefresh = () => {
    setRefresh(refresh() + 1);
  };

  createEffect(() => {
    if (redirect()) {
      navigate(redirect());
    }
  });

  return (
    <>
      <TableHeader
        config={props.config?.header}
        pathname={props.pathname}
        path_params={props.path_params}
        refresh={refresh}
        handleTitle={props.handleTitle}
        handleRefresh={handleRefresh}
      />
      <Table
        config={props.config?.table}
        pathname={props.pathname}
        table_data={table_data}
      />
    </>
  );
};

export default TablePanel;
