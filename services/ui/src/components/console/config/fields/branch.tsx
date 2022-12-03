import { validate_branch_name } from "../../../site/util";

const branchFieldsConfig = {
  name: {
    type: "text",
    placeholder: "Branch Name",
    icon: "fas fa-code-branch",
    help: "Must be a valid git reference.",
    validate: validate_branch_name,
  },
};

export default branchFieldsConfig;
