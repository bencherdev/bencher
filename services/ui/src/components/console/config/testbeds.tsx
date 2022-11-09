import projectFieldsConfig from "../../fields/config/org/projectFieldsConfig";
import testbedFieldsConfig from "../../fields/config/org/testbedFieldsConfig";
import { BENCHER_API_URL } from "../../site/util";
import { Button, Card, Display, Field, Operation, Row } from "./types";
import { parentPath, addPath, viewSlugPath } from "./util";

const testbedsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Testbeds",
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
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds`,
      add: {
        path: addPath,
        text: "Add a Testbed",
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
      title: "Add Testbed",
      path: parentPath,
    },
    form: {
      url: (path_params) =>
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds`,
      fields: [
        {
          kind: Field.HIDDEN,
          key: "project",
          path_param: "project_slug",
        },
        {
          kind: Field.INPUT,
          label: "Name",
          key: "name",
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.name,
        },
        {
          kind: Field.INPUT,
          label: "OS Name",
          key: "os_name",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.os_name,
        },
        {
          kind: Field.INPUT,
          label: "OS Version",
          key: "os_version",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.os_version,
        },
        {
          kind: Field.INPUT,
          label: "Runtime Name",
          key: "runtime_name",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.runtime_name,
        },
        {
          kind: Field.INPUT,
          label: "Runtime Version",
          key: "runtime_version",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.runtime_version,
        },
        {
          kind: Field.INPUT,
          label: "CPU",
          key: "cpu",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.cpu,
        },
        {
          kind: Field.INPUT,
          label: "GPU",
          key: "gpu",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.gpu,
        },
        {
          kind: Field.INPUT,
          label: "RAM",
          key: "ram",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.ram,
        },
        {
          kind: Field.INPUT,
          label: "Disk",
          key: "disk",
          value: "",
          valid: null,
          validate: false,
          nullify: false,
          clear: false,
          config: testbedFieldsConfig.disk,
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
        `${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds/${path_params?.testbed_slug}`,
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
        {
          kind: Card.FIELD,
          label: "OS Name",
          key: "os_name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "OS Version",
          key: "os_version",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Runtime Name",
          key: "runtime_name",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Runtime Version",
          key: "runtime_version",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "CPU",
          key: "cpu",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "RAM",
          key: "ram",
          display: Display.RAW,
        },
        {
          kind: Card.FIELD,
          label: "Disk",
          key: "disk",
          display: Display.RAW,
        },
      ],
      buttons: false,
    },
  },
};

export default testbedsConfig;
