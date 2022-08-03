import validator from "validator";
import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import validateDescription from "../../fields/validators/validateDescription";
import validateName from "../../fields/validators/validateName";
import { Button, Field, Operation, Resource, Row } from "../console";

const BENCHER_API_URL: string = import.meta.env.VITE_BENCHER_API_URL;

const consoleConfig = (pathname) => {
  console.log(pathname);
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
          url: `${BENCHER_API_URL}/v0/projects`,
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
          url: (project_slug) => {
            return `${BENCHER_API_URL}/v0/projects/${project_slug}`;
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
        },
      },
      [Operation.PERF]: {
        operation: Operation.PERF,
      },
    },
  };
};

export default consoleConfig;
