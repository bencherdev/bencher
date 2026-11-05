import type {
	JsonConsoleOrganization,
	JsonConsoleUser,
} from "../../types/bencher";

// The API counts bytes, not letters.
const MAX_BYTES = 64;

/** Why the API would refuse `name`, past an empty one. */
export const nameProblem = (name: string) => {
	const bytes = new TextEncoder().encode(name.trim()).length;
	return bytes > MAX_BYTES ? `${bytes} of ${MAX_BYTES} bytes` : undefined;
};

export interface OrganizationRow extends JsonConsoleOrganization {
	projects: number;
}

/** The reader's organizations by name, each with how many projects it holds. */
export const organizationRows = (user: JsonConsoleUser): OrganizationRow[] =>
	user.organizations
		.map((organization) => ({
			...organization,
			projects: user.projects.filter(
				(project) => project.organization === organization.uuid,
			).length,
		}))
		.sort((a, b) => a.name.localeCompare(b.name));
