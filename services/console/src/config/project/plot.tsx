import { validOptionResourceName } from "../../util/valid";
import type { JsonProject } from "../../types/bencher";

export const plotFields = (project?: undefined | JsonProject) => {
	return {
		title: {
			type: "text",
			placeholder: project?.name,
			icon: "fas fa-chart-line",
			validate: validOptionResourceName,
		},
		index: {
			help: "Must be a positive integer less than or equal to 64",
		},
		window: {
			help: "Must be a non-zero positive integer less than or equal to 4,294,967,295",
		},
	};
};
