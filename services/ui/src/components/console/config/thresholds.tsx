import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import thresholdFieldsConfig from "../../fields/config/project/thresholdFieldsConfig";
import { Button, Card, Field, Operation, Row } from "./types";
import {
  BENCHER_API_URL,
  parentPath,
  addPath,
  viewSlugPath,
  viewUuidPath,
} from "./util";

const thresholdsConfig = {
  [Operation.LIST]: {
    operation: Operation.LIST,
    header: {
      title: "Thresholds",
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
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/thresholds`;
      },
      add: {
        path: (pathname) => {
          return addPath(pathname);
        },
        text: "Add a Threshold",
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
  [Operation.ADD]: {
    operation: Operation.ADD,
    header: {
      title: "Add Threshold",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    form: {
      url: `${BENCHER_API_URL}/v0/thresholds`,
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
          key: "branch",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: thresholdFieldsConfig.name,
        },
        {
          kind: Field.INPUT,
          key: "testbed",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: thresholdFieldsConfig.name,
        },
        {
          kind: Field.INPUT,
          key: "kind",
          label: true,
          value: "",
          valid: null,
          validate: true,
          nullify: false,
          clear: false,
          config: thresholdFieldsConfig.name,
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
      key: "uuid",
      path: (pathname) => {
        return parentPath(pathname);
      },
    },
    deck: {
      url: (path_params) => {
        return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/thresholds/${path_params?.threshold_uuid}`;
      },
      cards: [
        {
          kind: Card.FIELD,
          field: "Branch UUID",
          key: "branch",
        },
        {
          kind: Card.FIELD,
          field: "Testbed UUID",
          key: "testbed",
        },
      ],
      buttons: false,
    },
  },
};

export default thresholdsConfig;
