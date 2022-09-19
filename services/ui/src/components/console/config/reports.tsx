import projectFieldsConfig from "../../fields/config/org/projectFieldsConfig";
import branchFieldsConfig from "../../fields/config/org/branchFieldsConfig";
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
      add: {
        path: (_pathname) => {
          return "/docs/how-to/run-a-report";
        },
        text: "Run a Report",
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
