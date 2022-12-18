import BRANCH_FIELDS from "./fields/branch";
import { Button, Card, Display, Operation, Row } from "../types";
import { parentPath, addPath, viewSlugPath } from "../util";
import { BENCHER_API_URL } from "../../../site/util";
import FieldKind from "../../../field/kind";

const branchesConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Branches",
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
        `${BENCHER_API_URL()}/v0/projects/${
          path_params?.project_slug
        }/branches`,
      add: {
        path: addPath,
        text: "Add a Branch",
      },
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
          text: "View",
          path: (pathname, datum) => viewSlugPath(pathname, datum),
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add Branch",
      path: parentPath,
    },
    form: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/projects/${
          path_params?.project_slug
        }/branches`,
      fields: [
        {
          kind: FieldKind.HIDDEN,
          key: "project",
          path_param: "project_slug",
        },
        {
          kind: FieldKind.INPUT,
          label: "Name",
          key: "name",
          value: "",
          valid: null,
          validate: true,
          config: BRANCH_FIELDS.name,
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
        }/branches/${path_params?.branch_slug}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Branch Name",
          key: "name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Branch Slug",
          key: "slug",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Branch UUID",
          key: "uuid",
          display: Display.RAW,
        },
        // {
        //   kind: Card.TABLE,
        //   label: "Versions",
        //   key: "versions",
        // },
      ],
      buttons: false,
    },
  },
};

export default branchesConfig;
