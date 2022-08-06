import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import branchFieldsConfig from "../../fields/config/project/branchFieldsConfig";
import { Button, Card, Field, Operation, Row } from "./types";
import { BENCHER_API_URL, parentPath, addPath, viewSlugPath } from "./util";

const reportsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Reports",
      buttons: [{ kind: Button.REFRESH }],
    },
    table: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/reports`;
      },
      row: {
        key: "start_time",
        items: [{}, {}, {}, {}],
        path: (pathname, datum) => {
          return `${pathname}/${datum?.uuid}`;
        },
      },
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "name",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    deck: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/reports/${path_params?.report_uuid}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Report Date Time",
          key: "start_time",
        },
      ],
      buttons: false,
    },
  },
};

export default reportsConfig;
