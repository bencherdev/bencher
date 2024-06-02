import { validOptionResourceName } from "../../util/valid";

export const PLOT_FIELDS = {
	title: {
		type: "text",
		placeholder: "Plot Name",
		icon: "fas fa-chart-line",
		validate: validOptionResourceName,
	},
	index: {
		help: "Must be a positive integer less than or equal to 64",
	},
	window: {
		help: "Must be a non-zero positive integer",
	},
};
