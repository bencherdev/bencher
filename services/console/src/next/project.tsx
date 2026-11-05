import { type NavigateOptions, useNavigate } from "@solidjs/router";
import {
	type Accessor,
	type Setter,
	createComputed,
	createContext,
	onCleanup,
	useContext,
} from "solid-js";
import { AccountContext } from "./account/context";
import type { Api } from "./api";

interface Project {
	api: Api;
	slug: Accessor<string>;
	/** The API's URL, which `bencher run` needs unless it is Bencher Cloud's. */
	host?: string | undefined;
	/** Where the router keeps project paths, under its base. */
	route?: string | undefined;
	/** The entity the page shows, which the tab title names first. */
	entity?: Setter<Entity | undefined>;
}

export interface Entity {
	/** What the page shows, such as `Alert`. */
	kind: string;
	name: string;
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

/** The API, on a project's pages and on an account's or organization's. */
export const useApi = () =>
	useContext(ProjectContext)?.api ?? useContext(AccountContext)?.api;

/** `navigate` to a project path, `/<slug>/...`, wherever the router keeps them. */
export const useProjectNavigate = () => {
	const navigate = useNavigate();
	const route = useContext(ProjectContext)?.route ?? "";
	return (to: string, options?: Partial<NavigateOptions>) =>
		navigate(`${route}${to}`, options);
};

/** Names the page's entity in the tab title while the page shows it. */
export const useEntityTitle = (
	kind: string,
	name: Accessor<string | undefined>,
) => {
	const set = useContext(ProjectContext)?.entity;
	if (!set) {
		return;
	}
	let shown: Entity | undefined;
	// Before effects, so the announcer never reads the last page's entity.
	createComputed(() => {
		const named = name();
		shown = named === undefined ? undefined : { kind, name: named };
		set(shown);
	});
	// The next page may name its entity before this one leaves.
	onCleanup(() => set((current) => (current === shown ? undefined : current)));
};
