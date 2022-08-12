import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import { Button, Card, Field, Operation, Row } from "./types";
import { BENCHER_API_URL, parentPath, addPath, viewSlugPath } from "./util";

const projectsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Projects",
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
      url: (_path_params) => {
        return `${BENCHER_API_URL}/v0/projects`;
      },
      row: {
        key: "name",
        items: [
          {
            kind: Row.TEXT,
            key: "slug",
          },
          {},
          {
            kind: Row.BOOL,
            key: "owner_default",
            text: "Default",
          },
          {},
        ],
        path: (pathname, datum) => {
          return viewSlugPath(pathname, datum);
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add Project",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    form: {
      url: `${BENCHER_API_URL}/v0/projects`,
      fields: [
        {
          kind: Field.INPUT,
          key: "name",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: projectFieldsConfig.name,
        },
        {
          kind: Field.TEXTAREA,
          key: "description",
          label: true,
          value: "",
          valid: null,
          validate: false,
          nullify: true,
          clear: false,
          config: projectFieldsConfig.description,
        },
        {
          kind: Field.INPUT,
          key: "url",
          label: true,
          value: "",
          valid: null,
          validate: false,
          nullify: true,
          clear: false,
          config: projectFieldsConfig.url,
        },
        {
          kind: Field.SWITCH,
          key: "public",
          type: "switch",
          label: true,
          value: false,
          validate: false,
          nullify: false,
          clear: false,
          config: projectFieldsConfig.public,
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
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Project Name",
          key: "name",
        },
        {
          kind: Card.FIELD,
          field: "Project Slug",
          key: "slug",
        },
        {
          kind: Card.FIELD,
          field: "Project Description",
          key: "description",
        },
        {
          kind: Card.FIELD,
          field: "Project URL",
          key: "url",
        },
        {
          kind: Card.FIELD,
          field: "Public Project",
          key: "public",
        },
      ],
      buttons: true,
    },
  },
  [Operation.PERF]: {
    operation: Operation.PERF,
    header: {
      title: "Benchmark Perf",
    },
    plot: {
      url: () => {
        return `${BENCHER_API_URL}/v0/perf`;
      },
    },
  },
};

export default projectsConfig;
