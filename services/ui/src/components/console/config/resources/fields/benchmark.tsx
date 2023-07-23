import { validate_string } from "../../../../site/util";
import { is_valid_benchmark_name, is_valid_slug } from "bencher_valid";

const BENCHMARK_FIELDS = {
	name: {
		type: "text",
		placeholder: "Benchmark Name",
		icon: "fas fa-tachometer-alt",
		help: "Must be a non-empty string",
		validate: (input) => validate_string(input, is_valid_benchmark_name),
	},
	slug: {
		type: "text",
		placeholder: "Benchmark Slug",
		icon: "fas fa-exclamation-triangle",
		help: "Must be a valid slug",
		validate: (input) => validate_string(input, is_valid_slug),
	},
};

export default BENCHMARK_FIELDS;
