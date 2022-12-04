import projectFieldsConfig from "./fields/project";
import { BENCHER_API_URL } from "../../site/util";
import { Button, Card, Display, Field, Operation, PerfTab, Row } from "./types";
import { parentPath, addPath } from "./util";

const projectsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Projects",
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
        `${BENCHER_API_URL()}/v0/organizations/${
          path_params?.organization_slug
        }/projects`,
      add: {
        path: addPath,
        text: "Add a Project",
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
        button: {
          text: "Select",
          path: (_pathname, datum) => `/console/projects/${datum?.slug}/perf`,
        },
      },
    },
  },
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add Project",
      path: parentPath,
    },
    form: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/organizations/${
          path_params.organization_slug
        }/projects`,
      fields: [
        {
          kind: Field.INPUT,
          label: "Name",
          key: "name",
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: projectFieldsConfig.name,
        },
        {
          kind: Field.TEXTAREA,
          label: "Description",
          key: "description",
          value: "",
          valid: null,
          validate: false,
          nullify: true,
          clear: false,
          config: projectFieldsConfig.description,
        },
        {
          kind: Field.INPUT,
          label: "URL",
          key: "url",
          value: "",
          valid: null,
          validate: false,
          nullify: true,
          clear: false,
          config: projectFieldsConfig.url,
        },
        {
          kind: Field.SWITCH,
          label: "Public Project",
          key: "public",
          type: "switch",
          value: true,
          validate: false,
          nullify: false,
          clear: false,
          config: projectFieldsConfig.public,
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
        `${BENCHER_API_URL()}/v0/organizations/${
          path_params?.organization_slug
        }/projects/${path_params?.project_slug}`,
      cards: [
        {
          kind: Card.FIELD,
          label: "Project Name",
          key: "name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Project Slug",
          key: "slug",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Project Description",
          key: "description",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Project URL",
          key: "url",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Public Project",
          key: "public",
          display: Display.RAW,
        },
      ],
      buttons: {
        path: (path_params) =>
          `/console/projects/${path_params?.project_slug}/perf`,
      },
    },
  },
  [Operation.PERF]: {
    operation: Operation.PERF,
    header: {
      title: "Benchmark Perf",
    },
    plot: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/perf`,
      tab_url: (path_params, tab: PerfTab) =>
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/${tab}`,
      key_url: (path_params, tab: PerfTab, uuid: string) =>
        `${BENCHER_API_URL()}/v0/projects/${
          path_params?.project_slug
        }/${tab}/${uuid}`,
    },
  },
};

export default projectsConfig;
