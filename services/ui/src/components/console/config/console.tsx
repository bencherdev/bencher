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
        fields: [
          {
            kind: Field.INPUT,
            key: "name",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: projectFieldsConfig.name,
          },
          {
            kind: Field.TEXTAREA,
            key: "description",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: projectFieldsConfig.description,
          },
          {
            kind: Field.INPUT,
            key: "url",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: projectFieldsConfig.url,
          },
        ],
      },
    },
  };
};

export default consoleConfig;
