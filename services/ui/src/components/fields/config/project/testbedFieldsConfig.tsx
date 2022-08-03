import validateDescription from "../../validators/validateDescription";
import validateName from "../../validators/validateName";
import validator from "validator";

const projectFieldsConfig = {
  name: {
    label: "Name",
    type: "text",
    placeholder: "Testbed Name",
    icon: "fas fa-server",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  os_name: {
    label: "OS Name",
    type: "text",
    placeholder: "",
    icon: "fas fa-desktop",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  os_version: {
    label: "OS Version",
    type: "text",
    placeholder: "",
    icon: "fas fa-desktop",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  runtime_name: {
    label: "Runtime Name",
    type: "text",
    placeholder: "",
    icon: "fas fa-code",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  runtime_version: {
    label: "Runtime Version",
    type: "text",
    placeholder: "",
    icon: "fas fa-code",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  cpu: {
    label: "CPU",
    type: "text",
    placeholder: "",
    icon: "fas fa-microchip",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  ram: {
    label: "RAM",
    type: "text",
    placeholder: "",
    icon: "fas fa-memory",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  disk: {
    label: "Disk",
    type: "text",
    placeholder: "",
    icon: "fas fa-compact-disc",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default projectFieldsConfig;
