import organizationFieldsConfig from "../../fields/config/org/organizationFieldsConfig";
import { BENCHER_API_URL } from "../../site/util";
import { Button, Card, Display, Field, Operation, PerfTab, Row } from "./types";
import { parentPath, addPath, viewSlugPath } from "./util";

const organizationsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    redirect: (table_data) =>
      table_data?.length === 1
        ? `/console/organizations/${table_data[0]?.slug}/projects`
        : null,
    header: {
      title: "Organizations",
      buttons: [{ kind: Button.REFRESH }],
    },
    table: {
      url: (_path_params) => `${BENCHER_API_URL()}/v0/organizations`,
      row: {
        key: "name",
        items: [
          {
            kind: Row.TEXT,
            key: "slug",
          },
          {},
          {},
          {},
        ],
        button: {
          text: "Select",
          path: (pathname, datum) =>
            viewSlugPath(pathname, datum) + "/projects",
        },
      },
    },
  },
  [Operation.VIEW]: {
    operation: Operation.VIEW,
    header: {
      key: "name",
      path: parentPath,
    },
    deck: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/organizations/${path_params?.organization_slug}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Organization Name",
          key: "name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Organization Slug",
          key: "slug",
          display: Display.RAW,
        },
      ],
      buttons: {
        path: (path_params) =>
          `/console/organizations/${path_params?.organization_slug}/projects`,
      },
    },
  },
};

export default organizationsConfig;
