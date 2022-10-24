import validateName from "../../validators/validateName";

const testbedFieldsConfig = {
  name: {
    type: "text",
    placeholder: "Testbed Name",
    icon: "fas fa-server",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  os_name: {
    type: "text",
    placeholder: "Operating System",
    icon: "fas fa-desktop",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  os_version: {
    type: "text",
    placeholder: "v1.2.3",
    icon: "fas fa-desktop",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  runtime_name: {
    type: "text",
    placeholder: "Runtime",
    icon: "fas fa-code",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  runtime_version: {
    type: "text",
    placeholder: "v1.2.3",
    icon: "fas fa-code",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  cpu: {
    type: "text",
    placeholder: "Intel Pentium IV",
    icon: "fas fa-microchip",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  ram: {
    type: "text",
    placeholder: "8GB",
    icon: "fas fa-memory",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  disk: {
    type: "text",
    placeholder: "64GB",
    icon: "fas fa-compact-disc",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default testbedFieldsConfig;
