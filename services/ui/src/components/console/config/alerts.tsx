import { Button, Card, Operation, } from "./types";
import {
  BENCHER_API_URL,
  parentPath,
  addPath,
  viewSlugPath,
  viewUuidPath,
} from "./util";

const alertsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Alerts",
      buttons: [
        { kind: Button.REFRESH },
      ],
    },
    table: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/alerts`;
      },
      add: {
        path: (_pathname) => {
          return "/docs/how-to/run-a-report";
        },
        text: "Run a Report",
      },
      row: {
        key: "uuid",
        items: [{}, {}, {}, {}],
        path: (pathname, datum) => {
          return viewUuidPath(pathname, datum);
        },
      },
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "uuid",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    deck: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/alerts/${path_params?.alert_uuid}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Perf UUID",
          key: "perf",
        },
        {
          kind: Card.FIELD,
          field: "Threshold UUID",
          key: "threshold",
        },
        {
          kind: Card.FIELD,
          field: "Statistic UUID",
          key: "statistic",
        },
        {
          kind: Card.FIELD,
          field: "Side",
          key: "side",
        },
        {
          kind: Card.FIELD,
          field: "Boundary",
          key: "boundary",
        },
        {
          kind: Card.FIELD,
          field: "Outlier",
          key: "outlier",
        },
      ],
      buttons: false,
    },
  },
};

export default alertsConfig;
