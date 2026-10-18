import type {
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../../types/bencher";

/** Where `/console` sends a signed in reader. */
export type ConsoleDestination =
	| { kind: "next"; path: string }
	| { kind: "classic"; path: string }
	| { kind: "organization"; slug: string }
	| { kind: "organizations" }
	| { kind: "onboard" }
	| { kind: "help" };

export const consoleDestination = (
	reader: Pick<JsonConsoleUser, "organizations" | "projects">,
	cachedOrganization: undefined | string,
	plan: boolean,
): ConsoleDestination => {
	const newest = newestProject(reader.projects);
	if (
		!plan &&
		newest &&
		reader.projects.some((project) => project.bmf_version === 1)
	) {
		return newest.bmf_version === 1
			? { kind: "next", path: `/next/console/projects/${newest.slug}/` }
			: { kind: "classic", path: `/console/projects/${newest.slug}/plots` };
	}
	if (cachedOrganization) {
		return { kind: "organization", slug: cachedOrganization };
	}
	const [organization] = reader.organizations;
	if (!organization) {
		return { kind: "help" };
	}
	if (reader.organizations.length > 1) {
		return { kind: "organizations" };
	}
	return reader.projects.length > 0
		? { kind: "organization", slug: organization.slug }
		: { kind: "onboard" };
};

const newestProject = (projects: JsonConsoleUserProject[]) =>
	projects.reduce<JsonConsoleUserProject | undefined>(
		(newest, project) =>
			project.last_report != null &&
			(newest?.last_report == null || project.last_report > newest.last_report)
				? project
				: newest,
		undefined,
	);
