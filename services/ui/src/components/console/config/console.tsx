import validator from "validator";
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
            config: {
              label: "Name",
              type: "text",
              placeholder: "Project Name",
              icon: "fas fa-project-diagram",
              help: "Must be at least four characters or longer.",
              validate: validateName,
            },
          },
          {
            kind: Field.TEXTAREA,
            key: "description",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: {
              label: "Description",
              type: "textarea",
              placeholder: "Describe the project",
              help: "Must be between 25 and 2,500 characters.",
              validate: validateDescription,
            },
          },
          {
            kind: Field.INPUT,
            key: "url",
            label: true,
            value: "",
            valid: null,
            validate: true,
            clear: false,
            config: {
              label: "URL",
              type: "text",
              placeholder: "www.example.com",
              icon: "far fa-window-maximize",
              help: "Must be a valid public facing URL.",
              validate: validator.isURL,
            },
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
