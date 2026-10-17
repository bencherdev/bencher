import { ORGANIZATIONS_PATH, type PageName } from "../paths";

/** An organization's pages, by their path under the organization's slug or UUID. */
const ORGANIZATION_PAGES = {
	settings: { page: "OrgGeneral", title: "General" },
	members: { page: "OrgMembers", title: "Members" },
	"projects/add": { page: "NewProject", title: "New project" },
} as const satisfies Record<string, { page: PageName; title: string }>;

type OrganizationPath = keyof typeof ORGANIZATION_PAGES;

/** The organization page `pathname` draws, with its title, if it is one. */
export const organizationPageAt = (pathname: string) => {
	if (!pathname.startsWith(`${ORGANIZATIONS_PATH}/`)) {
		return undefined;
	}
	const path = pathname
		.slice(ORGANIZATIONS_PATH.length + 1)
		.split("/")
		.filter((part) => part.length > 0)
		.slice(1)
		.join("/");
	return Object.hasOwn(ORGANIZATION_PAGES, path)
		? ORGANIZATION_PAGES[path as OrganizationPath]
		: undefined;
};
