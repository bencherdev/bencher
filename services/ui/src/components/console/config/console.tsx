import validator from "validator";
import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import testbedFieldsConfig from "../../fields/config/project/testbedFieldsConfig";
import validateDescription from "../../fields/validators/validateDescription";
import validateName from "../../fields/validators/validateName";
import { Button, Field, Operation, Resource, Row } from "../console";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const consoleConfig = (pathname) => {
  return {
    [Resource.PROJECTS]: {
      [Operation.LIST]: {
        operation: Operation.LIST,
        header: {
          title: "Projects",
          buttons: [
            {
              kind: Button.ADD,
              path: (pathname) => {
                return `${pathname}/add`;
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
              return `${pathname}/${datum?.slug}`;
            },
          },
        },
      },
      [Operation.ADD]: {
        operation: Operation.ADD,
        header: {
          title: "Add Project",
          path: (pathname) => {
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
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
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
          },
        },
      },
      [Operation.VIEW]: {
        operation: Operation.VIEW,
        header: {
          key: "name",
          path: (pathname) => {
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
          },
        },
        deck: {
          url: (path_params) => {
            return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}`;
          },
          cards: [
            {
              field: "Project Name",
              key: "name",
            },
            {
              field: "Project Slug",
              key: "slug",
            },
          ],
          buttons: true,
        },
      },
      [Operation.PERF]: {
        operation: Operation.PERF,
      },
    },
    [Resource.TESTBEDS]: {
      [Operation.LIST]: {
        operation: Operation.LIST,
        header: {
          title: "Testbeds",
          buttons: [
            {
              kind: Button.ADD,
              path: (pathname) => {
                return `${pathname}/add`;
              },
            },
            { kind: Button.REFRESH },
          ],
        },
        table: {
          url: (path_params) => {
            return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/testbeds`;
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
              return `${pathname}/${datum?.slug}`;
            },
          },
        },
      },
      [Operation.ADD]: {
        operation: Operation.ADD,
        header: {
          title: "Add Testbed",
          path: (pathname) => {
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
          },
        },
        form: {
          url: `${BENCHER_API_URL}/v0/testbeds`,
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
              config: projectFieldsConfig.name,
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
              config: testbedFieldsConfig.name,
            },
            {
              kind: Field.INPUT,
              key: "os_name",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.os_name,
            },
            {
              kind: Field.INPUT,
              key: "os_version",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.os_version,
            },
            {
              kind: Field.INPUT,
              key: "runtime_name",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.runtime_name,
            },
            {
              kind: Field.INPUT,
              key: "runtime_version",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.runtime_version,
            },
            {
              kind: Field.INPUT,
              key: "cpu",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.cpu,
            },
            {
              kind: Field.INPUT,
              key: "ram",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.ram,
            },
            {
              kind: Field.INPUT,
              key: "disk",
              label: true,
              value: "",
              valid: null,
              validate: false,
              nullify: false,
              clear: false,
              config: testbedFieldsConfig.disk,
            },
          ],
          path: (pathname) => {
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
          },
        },
      },
      [Operation.VIEW]: {
        operation: Operation.VIEW,
        header: {
          key: "name",
          path: (pathname) => {
            return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
          },
        },
        deck: {
          url: (path_params) => {
            return `${BENCHER_API_URL}/v0/projects/${path_params?.project_slug}/testbeds/${path_params?.testbed_slug}`;
          },
          cards: [
            {
              field: "Testbed Name",
              key: "name",
            },
            {
              field: "Testbed Slug",
              key: "slug",
            },
          ],
          buttons: false,
        },
      },
    },
  };
};

export default consoleConfig;
