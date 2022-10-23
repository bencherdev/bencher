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
        button: {
          text: "View",
          path: (pathname, datum) => {
            return viewUuidPath(pathname, datum);
          },
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
          label: "Perf UUID",
          key: "perf",
        },
        {
          kind: Card.FIELD,
          label: "Threshold UUID",
          key: "threshold",
        },
        {
          kind: Card.FIELD,
          label: "Statistic UUID",
          key: "statistic",
        },
        {
          kind: Card.FIELD,
          label: "Side",
          key: "side",
        },
        {
          kind: Card.FIELD,
          label: "Boundary",
          key: "boundary",
        },
        {
          kind: Card.FIELD,
          label: "Outlier",
          key: "outlier",
        },
      ],
      buttons: false,
    },
  },
};

export default alertsConfig;
