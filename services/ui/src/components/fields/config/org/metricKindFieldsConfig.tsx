import validateName from "../../validators/validateName";

const metricKindFieldsConfig = {
  name: {
    type: "text",
    placeholder: "Name",
    icon: "fas fa-stopwatch",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
  units: {
    type: "text",
    placeholder: "Units",
    icon: "fas fa-ruler",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default metricKindFieldsConfig;
