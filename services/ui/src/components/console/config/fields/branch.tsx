import { is_valid_branch_name } from "bencher_valid";
import { validate_string } from "../../../site/util";

const BRANCH_FIELDS = {
  name: {
    type: "text",
    placeholder: "branch-name",
    icon: "fas fa-code-branch",
    help: "Must be a valid git reference",
    validate: (input) => validate_string(input, is_valid_branch_name),
  },
};

export default BRANCH_FIELDS;
