import { validate_string } from "../../../../site/util";
import { is_valid_non_empty, is_valid_slug } from "bencher_valid";

const METRIC_KIND_FIELDS = {
	name: {
		type: "text",
		placeholder: "Metric Kind Name",
		icon: "fas fa-shapes",
		help: "Must be non-empty string",
		validate: (input) => validate_string(input, is_valid_non_empty),
	},
	slug: {
		type: "text",
		placeholder: "Metric Kind Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: (input) => validate_string(input, is_valid_slug),
	},
	units: {
		type: "text",
		placeholder: "units/time",
		icon: "fas fa-ruler",
		help: "Must be non-empty string",
		validate: (input) => validate_string(input, is_valid_non_empty),
	},
};

export default METRIC_KIND_FIELDS;
