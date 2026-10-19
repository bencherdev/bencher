import { projectPath } from "../../../next/paths";
import type {
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../../types/bencher";

/** Where `/console` sends a signed in reader. */
export type ConsoleDestination =
	| { kind: "project"; path: string }
	| { kind: "organization"; slug: string }
	| { kind: "organizations" }
	| { kind: "onboard" }
	| { kind: "help" };

export const consoleDestination = (
	reader: Pick<JsonConsoleUser, "organizations" | "projects">,
	cachedOrganization: undefined | string,
	plan: boolean,
): ConsoleDestination => {
	const [main] = [...reader.projects].sort(newestFirst);
	if (!plan && main) {
		return { kind: "project", path: projectPath(main.slug) };
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

// As the switcher orders them: the newest run first, then by name.
const newestFirst = (a: JsonConsoleUserProject, b: JsonConsoleUserProject) =>
	(b.last_report ?? 0) - (a.last_report ?? 0) || a.name.localeCompare(b.name);
