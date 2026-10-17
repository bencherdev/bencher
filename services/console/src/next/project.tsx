import { type NavigateOptions, useNavigate } from "@solidjs/router";
import { type Accessor, createContext, useContext } from "solid-js";
import type { Api } from "./api";

interface Project {
	api: Api;
	slug: Accessor<string>;
	/** The API's URL, which `bencher run` needs unless it is Bencher Cloud's. */
	host?: string | undefined;
	/** Where the router keeps project paths, under its base. */
	route?: string | undefined;
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

/** `navigate` to a project path, `/<slug>/...`, wherever the router keeps them. */
export const useProjectNavigate = () => {
	const navigate = useNavigate();
	const route = useContext(ProjectContext)?.route ?? "";
	return (to: string, options?: Partial<NavigateOptions>) =>
		navigate(`${route}${to}`, options);
};
