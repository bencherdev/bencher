import { type Accessor, createContext, useContext } from "solid-js";
import type { Api } from "./api";

interface Project {
	api: Api;
	slug: Accessor<string>;
	/** The API's URL, which `bencher run` needs unless it is Bencher Cloud's. */
	host?: string | undefined;
}

// Bencher Cloud's API, which `bencher run` reports to unless told otherwise.
const CLOUD = "https://api.bencher.dev";

/** The `--host` a run names: the API's URL, unless it is Bencher Cloud's. */
export const runHost = (host: string | undefined) =>
	host && host !== CLOUD ? host : undefined;

/** The API and the project slug of the page being shown. */
export const ProjectContext = createContext<Project>();

export const useProject = () => {
	const project = useContext(ProjectContext);
	if (!project) {
		throw new Error("useProject outside of the console layout");
	}
	return project;
};
