import { type Accessor, createContext, useContext } from "solid-js";
import type { Api } from "./api";

interface Project {
	api: Api;
	slug: Accessor<string>;
}

/** The API and the project slug of the page being shown. */
export const ProjectContext = createContext<Project>();

export const useProject = () => {
	const project = useContext(ProjectContext);
	if (!project) {
		throw new Error("useProject outside of the console layout");
	}
	return project;
};
