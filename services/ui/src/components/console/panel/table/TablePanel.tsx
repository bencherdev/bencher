import axios from "axios";
import {
  createSignal,
  createResource,
  createMemo,
  createEffect,
} from "solid-js";
import Table from "./Table";

import TableHeader from "./TableHeader";
import { getToken, validate_jwt } from "../../../site/util";
import { useNavigate } from "solid-app-router";

const TablePanel = (props) => {
  const navigate = useNavigate();

  const options = () => {
    return {
      url: props.config?.table?.url(props.path_params()),
      method: "GET",
      headers: {
        "Content-Type": "application/json",
        Authorization: `Bearer ${props.user()?.token}`,
      },
    };
  };

  const [refresh, setRefresh] = createSignal(0);
  const [page, setPage] = createSignal(1);
  const fetcher = createMemo(() => {
    return {
      refresh: refresh(),
      page: page(),
      token: props.user()?.token,
    };
  });
  const [fetcher_cache, setFetcherCache] = createSignal(fetcher());

  const fetchData = async (new_fetcher) => {
    const EMPTY_ARRAY = [];
    if (new_fetcher === fetcher_cache()) {
      return EMPTY_ARRAY;
    }
    setFetcherCache(new_fetcher);

    console.log(refresh());
    console.log(page());
    console.log(props.user()?.token);
    console.log(props.path_params());
    try {
      if (!validate_jwt(props.user()?.token)) {
        return EMPTY_ARRAY;
      }

      let resp = await axios(options());
      const data = resp.data;

      return data;
    } catch (error) {
      console.error(error);

      return EMPTY_ARRAY;
    }
  };

  const [table_data] = createResource(fetcher, fetchData);

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
        path_params={props.path_params}
        refresh={refresh}
        handleRefresh={handleRefresh}
      />
      <Table config={props.config?.table} table_data={table_data} />
    </>
  );
};

export default TablePanel;
