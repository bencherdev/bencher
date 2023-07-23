import { is_valid_branch_name, is_valid_slug } from "bencher_valid";
import { validate_string } from "../../../../site/util";

const BRANCH_FIELDS = {
	name: {
		type: "text",
		placeholder: "branch-name",
		icon: "fas fa-code-branch",
		help: "Must be a valid git reference",
		validate: (input) => validate_string(input, is_valid_branch_name),
	},
	slug: {
		type: "text",
		placeholder: "Branch Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: (input) => validate_string(input, is_valid_slug),
	},
};

export default BRANCH_FIELDS;
