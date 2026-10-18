import type {
	JsonConsoleOrganization,
	JsonConsoleUser,
} from "../../types/bencher";

// The API counts bytes, not letters.
const MAX_BYTES = 64;

/**
 * The slug the API derives from `name`, as its `slug` crate would for Latin
 * text; the API transliterates other scripts, so it alone has the last word.
 */
export const slugOf = (name: string) => {
	const slug = name
		.normalize("NFKD")
		.replace(/\p{M}/gu, "")
		.toLowerCase()
		.replace(/[^a-z0-9]+/g, "-")
		.replace(/^-|-$/g, "");
	return slug.length > MAX_BYTES
		? slug.slice(0, MAX_BYTES).replace(/-$/, "")
		: slug;
};

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
