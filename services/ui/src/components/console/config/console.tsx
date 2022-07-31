import validator from "validator";
import projectFieldsConfig from "../../fields/config/project/projectFieldsConfig";
import validateDescription from "../../fields/validators/validateDescription";
import validateName from "../../fields/validators/validateName";
import { Button, Field, Operation, Resource } from "../console";

const consoleConfig = (pathname) => {
  console.log(pathname);
  return {
    [Resource.PROJECTS]: {
      [Operation.LIST]: {
        operation: Operation.LIST,
        title: "Projects",
        header: "name",
        items: [
          {
            kind: "text",
            key: "slug",
          },
          {},
          {
            kind: "bool",
            key: "owner_default",
            text: "Default",
          },
          {},
        ],
        buttons: [
          { kind: Button.ADD, path: "/console/projects/add" },
          { kind: Button.REFRESH },
        ],
      },
      [Operation.ADD]: {
        operation: Operation.ADD,
        title: "Add Project",
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
        buttons: {
          [Button.BACK]: { path: "/console/projects" },
        },
      },
    },
  };
};

export default consoleConfig;
