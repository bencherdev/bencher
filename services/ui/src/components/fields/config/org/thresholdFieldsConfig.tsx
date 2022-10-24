import validateName from "../../validators/validateName";

const thresholdFieldsConfig = {
  name: {
    ype: "text",
    placeholder: "Threshold TODO",
    icon: "fas fa-server",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default thresholdFieldsConfig;
