import { validate_string } from "../../../../site/util";
import { is_valid_benchmark_name } from "bencher_valid";

const BENCHMARK_FIELDS = {
	name: {
		type: "text",
		placeholder: "Benchmark Name",
		icon: "fas fa-tachometer-alt",
		help: "Must be a non-empty string",
		validate: (input) => validate_string(input, is_valid_benchmark_name),
	},
};

export default BENCHMARK_FIELDS;
