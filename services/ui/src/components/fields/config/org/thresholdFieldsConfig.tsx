import validateName from "../../validators/validateName";

const thresholdFieldsConfig = {
  name: {
    label: "Name",
    type: "text",
    placeholder: "Testbed Name",
    icon: "fas fa-server",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default thresholdFieldsConfig;
