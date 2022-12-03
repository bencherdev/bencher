import validateName from "../../validators/validateName";

const branchFieldsConfig = {
  name: {
    type: "text",
    placeholder: "Branch Name",
    icon: "fas fa-code-branch",
    help: "Must be at least four characters or longer.",
    validate: validateName,
  },
};

export default branchFieldsConfig;
