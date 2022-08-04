import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import branchFieldsConfig from "../../fields/config/project/branchFieldsConfig";
import { Button, Card, Field, Operation, Row } from "./types";
import { BENCHER_API_URL, parentPath, addPath, viewPath } from "./util";

const branchesConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Branches",
      buttons: [
        {
          kind: Button.ADD,
          path: (pathname) => {
            return addPath(pathname);
          },
        },
        { kind: Button.REFRESH },
      ],
    },
    table: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/branches`;
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
        path: (pathname, datum) => {
          return viewPath(pathname, datum);
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add Branch",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    form: {
      url: `${BENCHER_API_URL}/v0/branches`,
      fields: [
        {
          kind: Field.FIXED,
          key: "project",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: projectFieldsConfig.slug,
        },
        {
          kind: Field.INPUT,
          key: "name",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: branchFieldsConfig.name,
        },
      ],
      path: (pathname) => {
        return parentPath(pathname);
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
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/branches/${path_params?.branch_slug}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Branch Name",
          key: "name",
        },
        {
          kind: Card.FIELD,
          field: "Branch Slug",
          key: "slug",
        },
        {
          kind: Card.TABLE,
          field: "Versions",
          key: "versions",
        },
      ],
      buttons: false,
    },
  },
};

export default branchesConfig;
