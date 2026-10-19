import { shortcutPage } from "./account/paths";
import {
	CONSOLE_PATH,
	DIMENSIONS,
	ORGANIZATIONS_PATH,
	PROJECTS_PATH,
	USERS_PATH,
} from "./paths";
import { exploreSearchFromClassic } from "./query/classic";
import { readsExplore } from "./query/query";

type Place = Pick<URL, "pathname" | "search">;

/** Where a classic console or public path now lands, from its path and query alone; nothing when a page draws it in place. */
export const classicRedirect = ({
	pathname,
	search,
}: Place): string | undefined => {
	const [root, area, owner, ...rest] = pathname
		.split("/")
		.filter((part) => part.length > 0);
	if (root === "perf") {
		return area === undefined
			? "/explore"
			: publicRedirect(area, [owner, ...rest].filter(isSegment));
	}
	if (root !== "console") {
		return undefined;
	}
	if (area === "projects") {
		return owner === undefined
			? CONSOLE_PATH
			: projectRedirect(owner, rest, search);
	}
	if (owner === undefined || drawnAt(area, owner, rest)) {
		return undefined;
	}
	if (area === "organizations") {
		return organizationRedirect(owner, rest, search);
	}
	if (area === "users" && !shortcutPage(pathname)) {
		return userRedirect(owner, rest);
	}
	return undefined;
};

const isSegment = (part: string | undefined): part is string =>
	part !== undefined;

// Pages of their own that sit where an organization's or a user's slug would.
const OWN_PAGES: Readonly<Record<string, readonly string[]>> = {
	organizations: ["add"],
	users: ["apps", "help"],
};

const drawnAt = (area: string | undefined, owner: string, rest: string[]) =>
	rest.length === 0 && (OWN_PAGES[area ?? ""] ?? []).includes(owner);

// The public plot and its embed draw in place; every other public page is the console's.
const publicRedirect = (slug: string, rest: string[]) => {
	if (rest.length === 0 || (rest.length === 1 && rest[0] === "embed")) {
		return undefined;
	}
	return (
		projectRedirect(slug, rest, "") ??
		`${PROJECTS_PATH}/${slug}/${rest.join("/")}`
	);
};

// Each list whose classic add form the console replaces with the list itself.
const LISTS: readonly string[] = [...DIMENSIONS, "thresholds", "keys"];

const projectRedirect = (slug: string, rest: string[], search: string) => {
	const project = `${PROJECTS_PATH}/${slug}`;
	const explore = `${project}/explore${exploreSearchFromClassic(search)}`;
	const [page, id, form, ...more] = rest;
	if (more.length > 0) {
		return undefined;
	}
	if (page === undefined) {
		return readsExplore(new URLSearchParams(search)) ? explore : undefined;
	}
	if (id === undefined) {
		return page === "perf"
			? explore
			: page === "reports"
				? `${project}/runs`
				: page === "metrics"
					? `${project}/plots`
					: undefined;
	}
	if (form === undefined) {
		if (id === "add" && LISTS.includes(page)) {
			return `${project}/${page}`;
		}
		return page === "plots"
			? `${project}/explore?plot=${id}`
			: page === "keys"
				? `${project}/keys`
				: undefined;
	}
	return page === "thresholds" && form === "edit"
		? `${project}/thresholds/${id}`
		: undefined;
};

const organizationRedirect = (
	organization: string,
	rest: string[],
	search: string,
) => {
	const base = `${ORGANIZATIONS_PATH}/${organization}`;
	const [page, id, ...more] = rest;
	if (more.length > 0) {
		return undefined;
	}
	if (page === undefined || (page === "projects" && id === undefined)) {
		return `${base}/settings`;
	}
	if (page === "members" && id !== undefined) {
		return `${base}/members`;
	}
	// Stripe returns to the classic checkout, whose query Billing records.
	return page === "checkout" && id === undefined
		? `${base}/billing${search}`
		: undefined;
};

const userRedirect = (user: string, rest: string[]) => {
	const base = `${USERS_PATH}/${user}`;
	const [page, id, ...more] = rest;
	if (page === undefined) {
		return `${base}/settings`;
	}
	return (page === "keys" || page === "tokens") &&
		id !== undefined &&
		more.length === 0
		? `${base}/${page}`
		: undefined;
};
