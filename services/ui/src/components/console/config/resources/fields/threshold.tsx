import { BENCHER_API_URL, validate_u32 } from "../../../../site/util";

const validate_boundary = (input: string) => {
	if (input.length === 0) {
		return false;
	}
	const num = Number(input);
	return Number.isFinite(num) && num >= 0.5 && num <= 1.0;
};

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
	left_side: {
		type: "input",
		placeholder: "0.95",
		icon: "fas fa-hand-point-left",
		help: "Must be between 0.5000 and 1.0000",
		validate: validate_boundary,
	},
	right_side: {
		type: "input",
		placeholder: "0.95",
		icon: "fas fa-hand-point-right",
		help: "Must be between 0.5000 and 1.0000",
		validate: validate_boundary,
	},
};

export default THRESHOLD_FIELDS;
