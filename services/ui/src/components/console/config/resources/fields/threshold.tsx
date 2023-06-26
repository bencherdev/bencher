import {
	BENCHER_API_URL,
	validate_boundary,
	validate_u32,
} from "../../../../site/util";

const THRESHOLD_FIELDS = {
	branch: {
		icon: "fas fa-code-branch",
		option_key: "name",
		value_key: "uuid",
		url: (path_params) =>
			`${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/branches`,
	},
	testbed: {
		icon: "fas fa-server",
		option_key: "name",
		value_key: "uuid",
		url: (path_params) =>
			`${BENCHER_API_URL()}/v0/projects/${path_params?.project_slug}/testbeds`,
	},
	metric_kind: {
		icon: "fas fa-shapes",
		option_key: "name",
		value_key: "uuid",
		url: (path_params) =>
			`${BENCHER_API_URL()}/v0/projects/${
				path_params?.project_slug
			}/metric-kinds`,
	},
	test: {
		icon: "fas fa-vial",
	},
	lower_boundary: {
		type: "input",
		placeholder: "0.95",
		icon: "fas fa-arrow-down",
		help: "Must be between 0.5000 and 1.0000",
		validate: validate_boundary,
	},
	upper_boundary: {
		type: "input",
		placeholder: "0.95",
		icon: "fas fa-arrow-up",
		help: "Must be between 0.5000 and 1.0000",
		validate: validate_boundary,
	},
	min_sample_size: {
		type: "number",
		placeholder: "30",
		icon: "fas fa-cube",
		help: "Must be an integer greater than zero",
		validate: validate_u32,
	},
	max_sample_size: {
		type: "number",
		placeholder: "100",
		icon: "fas fa-cubes",
		help: "Must be an integer greater than zero",
		validate: validate_u32,
	},
	window: {
		type: "number",
		placeholder: "525600",
		icon: "fas fa-calendar-week",
		help: "Must be an integer greater than zero",
		validate: validate_u32,
	},
};

export default THRESHOLD_FIELDS;
