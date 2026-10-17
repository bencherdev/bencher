import type {
	JsonConsoleOrganization,
	JsonConsoleUser,
	JsonOrganization,
} from "../../types/bencher";
import type { Api } from "../api";

const path = (organization: string) =>
	`/v0/organizations/${encodeURIComponent(organization)}`;

export const organizationQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization],
	queryFn: async ({ signal }: { signal: AbortSignal }) =>
		(await api.get<JsonOrganization>(path(organization), signal)).data,
});

/** What the reader may do to the organization, as the API rules it. */
export interface Allowed {
	edit: boolean;
	delete: boolean;
}

export const allowedQuery = (api: Api, organization: string) => ({
	queryKey: ["organization", organization, "allowed"],
	queryFn: async ({ signal }: { signal: AbortSignal }): Promise<Allowed> => {
		const allowed = async (permission: keyof Allowed) =>
			(
				await api.get<{ allowed: boolean }>(
					`${path(organization)}/allowed/${permission}`,
					signal,
				)
			).data.allowed;
		const [edit, remove] = await Promise.all([
			allowed("edit"),
			allowed("delete"),
		]);
		return { edit, delete: remove };
	},
});

/** The organization at `key`, its slug or UUID, among the reader's own. */
export const organizationIn = (
	user: JsonConsoleUser | undefined,
	key: string,
) => user?.organizations.find(({ slug, uuid }) => slug === key || uuid === key);

/**
 * The reader's organizations and projects with `organization` as the API now
 * has it, or without it and its projects once it is deleted.
 */
export const withOrganization = (
	user: JsonConsoleUser,
	uuid: string,
	organization: JsonConsoleOrganization | undefined,
): JsonConsoleUser => ({
	...user,
	organizations: organization
		? user.organizations.map((known) =>
				known.uuid === uuid ? organization : known,
			)
		: user.organizations.filter((known) => known.uuid !== uuid),
	projects: organization
		? user.projects
		: user.projects.filter((project) => project.organization !== uuid),
});
