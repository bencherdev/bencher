import type { JsonConsoleUser, JsonProject } from "../../types/bencher";
import type { Api } from "../api";
import { totalOf } from "../list/pages";
import { organizationIn } from "./data";

/** A project in the organization's list. */
export interface ProjectRow {
	uuid: string;
	name: string;
	slug: string;
	/** When it last ran, `null` if never, and absent where the list does not know. */
	last_report?: number | null;
	/** Its active alerts, absent where the list does not know. */
	active_alerts?: number;
}

const byName = (a: ProjectRow, b: ProjectRow) => a.name.localeCompare(b.name);

/** The organization's projects in the reader's bootstrap, by name. */
export const projectRows = (
	user: JsonConsoleUser,
	organization: string,
): ProjectRow[] =>
	user.projects
		.filter((project) => project.organization === organization)
		.map(({ uuid, name, slug, last_report, active_alerts }) => ({
			uuid,
			name,
			slug,
			last_report: last_report ?? null,
			active_alerts,
		}))
		.sort(byName);

/** The projects the organization's endpoint lists, by name. */
export const endpointRows = (projects: readonly JsonProject[]): ProjectRow[] =>
	projects.map(({ uuid, name, slug }) => ({ uuid, name, slug })).sort(byName);

/**
 * Where the list's rows come from, once the bootstrap has loaded: the
 * bootstrap for one of the reader's organizations, the endpoint for another,
 * which a server admin may read.
 */
export const projectSource = (
	user: JsonConsoleUser | undefined,
	key: string,
):
	| { from: "bootstrap"; rows: ProjectRow[] }
	| { from: "endpoint" }
	| undefined => {
	if (!user) {
		return undefined;
	}
	const organization = organizationIn(user, key);
	return organization
		? { from: "bootstrap", rows: projectRows(user, organization.uuid) }
		: { from: "endpoint" };
};

// The API's largest page.
const PAGE = 255;

/** Every project the organization holds, a page at a time. */
export const organizationProjectsQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "projects"],
	queryFn: async ({ signal }: { signal: AbortSignal }) => {
		const projects: JsonProject[] = [];
		for (let page = 1; ; page++) {
			const { data, headers } = await api.get<JsonProject[]>(
				`/v0/organizations/${encodeURIComponent(organization)}/projects?per_page=${PAGE}&page=${page}`,
				signal,
			);
			projects.push(...data);
			if (data.length < PAGE || projects.length >= totalOf(headers, data)) {
				return projects;
			}
		}
	},
});
