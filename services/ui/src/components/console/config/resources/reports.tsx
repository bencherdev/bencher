import { BENCHER_API_URL } from "../../../site/util";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, viewUuidPath } from "../util";

const reportsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Reports",
      buttons: [{ kind: Button.REFRESH }],
    },
    table: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/reports`,
      add: {
        path: (_pathname) => "/docs/how-to/quick-start",
        text: "Run a Report",
      },
      row: {
        key: "start_time",
        items: [
          {
            kind: Row.TEXT,
            key: "adapter",
          },
          {},
          {},
          {},
        ],
        button: {
          text: "View",
          path: viewUuidPath,
        },
      },
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "start_time",
      path: parentPath,
    },
    deck: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/projects/${
          path_params?.project_slug
        }/reports/${path_params?.report_uuid}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Report Start Time",
          key: "start_time",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Report End Time",
          key: "end_time",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Report UUID",
          key: "uuid",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Results Adapter",
          key: "adapter",
          display: Display.RAW,
        },
      ],
    },
  },
};

export default reportsConfig;
