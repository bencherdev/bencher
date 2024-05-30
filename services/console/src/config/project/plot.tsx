import { validOptionResourceName } from "../../util/valid";
import type { JsonProject } from "../../types/bencher";

export const plotFields = (project: undefined | JsonProject) => {
	return {
		title: {
			type: "text",
			placeholder: project?.name,
			icon: "fas fa-chart-line",
			validate: validOptionResourceName,
		},
		rank: {},
	};
};
