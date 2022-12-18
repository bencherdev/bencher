import FieldKind from "../../../field/kind";
import { BENCHER_API_URL } from "../../../site/util";
import TESTBED_FIELDS from "./fields/testbed";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";

const tokensConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "API Tokens",
      buttons: [
        {
          kind: Button.ADD,
          path: addPath,
        },
        { kind: Button.REFRESH },
      ],
    },
    table: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}/tokens`,
      add: {
        path: addPath,
        text: "Add an API Token",
      },
      row: {
        key: "name",
        items: [{}, {}, {}, {}],
        button: {
          text: "View",
          path: (pathname, datum) => viewSlugPath(pathname, datum),
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add API Token",
      path: parentPath,
    },
    form: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/users/${path_params?.user_slug}/tokens`,
      fields: [
        {
          kind: FieldKind.INPUT,
          label: "Name",
          key: "name",
          value: "",
          valid: null,
          validate: true,
          config: TESTBED_FIELDS.name,
        },
      ],
      path: parentPath,
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
        `${BENCHER_API_URL()}/v0/projects/${
          path_params?.project_slug
        }/testbeds/${path_params?.testbed_slug}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Testbed Name",
          key: "name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Testbed Slug",
          key: "slug",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Testbed UUID",
          key: "uuid",
          display: Display.RAW,
        },
      ],
    },
  },
};

export default tokensConfig;
