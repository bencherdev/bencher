/** The new console, which keeps the classic console's paths under `/next`. */
export const NEXT_CONSOLE = "/next/console";
/** The classic console's project pages. */
const CLASSIC_PROJECTS = "/console/projects";
/** The new console serves BMF v1 projects here until version 0 retires. */
export const NEXT_PROJECTS = `/next${CLASSIC_PROJECTS}`;
/** The account pages, under the reader's slug. */
export const NEXT_USERS = `${NEXT_CONSOLE}/users`;

export const profilePath = (user: string) => `${NEXT_USERS}/${user}/settings`;

/** The reader's organizations, with each one's pages under its slug or UUID. */
export const ORGANIZATIONS_PATH = `${NEXT_CONSOLE}/organizations`;

export const organizationPath = (organization: string) =>
	`${ORGANIZATIONS_PATH}/${organization}/settings`;

/** The first segment under `base`, if `pathname` is beneath it. */
const segmentUnder = (base: string, pathname: string) =>
	pathname.startsWith(`${base}/`)
		? pathname.slice(base.length + 1).split("/")[0] || undefined
		: undefined;

/** The organization an organization page is about, if `pathname` is one. */
export const organizationOf = (pathname: string) =>
	segmentUnder(ORGANIZATIONS_PATH, pathname);

/** The dimension lists keep their classic paths, each page under its list. */
export const DIMENSIONS = [
	"branches",
	"testbeds",
	"benchmarks",
	"measures",
] as const;

export type Tab =
	| "explore"
	| "plots"
	| "runs"
	| "alerts"
	| "thresholds"
	| "settings";

/** The tab row, in order, with the path segments each tab owns. */
export const TABS: readonly { tab: Tab; label: string; segments: string[] }[] =
	[
		{ tab: "explore", label: "Explore", segments: ["explore"] },
		{ tab: "plots", label: "Plots", segments: ["plots"] },
		{ tab: "runs", label: "Runs", segments: ["runs"] },
		{ tab: "alerts", label: "Alerts", segments: ["alerts"] },
		{ tab: "thresholds", label: "Thresholds", segments: ["thresholds"] },
		{
			tab: "settings",
			label: "Settings",
			// Dimensions and keys live in Settings.
			segments: ["settings", ...DIMENSIONS, "keys"],
		},
	];

export const projectPath = (slug: string, tab?: Tab) =>
	`${NEXT_PROJECTS}/${slug}/${tab ?? ""}`;

/** A report's page, which sits outside the tab row's paths and belongs to Runs. */
const REPORTS = "reports";

export const reportPath = (slug: string, report: string) =>
	`${NEXT_PROJECTS}/${slug}/${REPORTS}/${report}`;

export const alertPath = (slug: string, alert: string) =>
	`${projectPath(slug, "alerts")}/${alert}`;

/** A run's page, under its project's Runs. */
export const runPath = (slug: string, version: string) =>
	`${projectPath(slug, "runs")}/${version}`;

const TAB_PAGES = {
	explore: "Explore",
	plots: "Plots",
	runs: "Runs",
	alerts: "Alerts",
	thresholds: "Thresholds",
	settings: "Settings",
} as const satisfies Record<Tab, string>;

/** A file under `pages/`: each draws one kind of path. */
export type PageName =
	| (typeof TAB_PAGES)[Tab]
	| "Run"
	| "Report"
	| "Alert"
	| "Threshold"
	| "Dimensions"
	| "Profile"
	| "UserKeys"
	| "LegacyTokens"
	| "OrgGeneral"
	| "Organizations"
	| "NewProject"
	| "NotFound";

/** The page that draws a project path, from its segments after the project. */
export const pageName = (rest: string[]): PageName => {
	if (rest.length === 2 && rest[0] === "runs") {
		return "Run";
	}
	if (isReport(rest)) {
		return "Report";
	}
	if (rest.length === 2 && rest[0] === "thresholds") {
		return "Threshold";
	}
	if (rest.length === 2 && rest[0] === "alerts") {
		return "Alert";
	}
	if (DIMENSIONS.some((dimension) => dimension === rest[0])) {
		return "Dimensions";
	}
	const tab = tabOf(rest);
	return tab ? TAB_PAGES[tab] : "NotFound";
};

export interface ProjectLocation {
	slug: string;
	/** The segments after the project. */
	rest: string[];
}

export const parseNextPath = (pathname: string) =>
	parseProjectPath(NEXT_PROJECTS, pathname);

const isReport = (rest: string[]) => rest.length === 2 && rest[0] === REPORTS;

/** The project root is Explore, and a report belongs to Runs. */
export const tabOf = (rest: string[]): Tab | undefined => {
	const [segment] = rest;
	if (segment === undefined) {
		return "explore";
	}
	if (isReport(rest)) {
		return "runs";
	}
	return TABS.find(({ segments }) => segments.includes(segment))?.tab;
};

/** A location, such as `window.location` or a `URL`. */
type Place = Pick<URL, "pathname" | "hash">;

// The classic perf page and Explore are the same place.
const CLASSIC_PERF = "perf";
const NEXT_EXPLORE = "explore";
// The classic keys list is Settings' Keys.
const KEYS = "keys";
// The classic console lists reports where the new one lists runs, and has no run page.
const NEXT_RUNS = "runs";

// A redirect between the consoles keeps the place and drops the query: each
// console reads its own, and the classic one adds paging to its lists.

/** The same place in the classic console. */
export const classicHref = ({ pathname, hash }: Place) => {
	const place = classicPlace(pathname);
	return place && `${place.path}${place.hash ? hash : ""}`;
};

/** The same place in the classic console, before its hash. */
export const classicPlace = (pathname: string): ClassicPlace | undefined => {
	const place = parseNextPath(pathname);
	if (!place) {
		return undefined;
	}
	const { slug, rest } = place;
	if (rest.length === 0 || rest[0] === NEXT_EXPLORE) {
		return { path: `${CLASSIC_PROJECTS}/${slug}/${CLASSIC_PERF}`, hash: false };
	}
	const path = rest[0] === NEXT_RUNS ? REPORTS : rest.join("/");
	return { path: `${CLASSIC_PROJECTS}/${slug}/${path}`, hash: true };
};

export interface ClassicPlace {
	path: string;
	/** Whether the page reads a hash; the perf page reads none. */
	hash: boolean;
}

/** The same place in the new console, if it has a page for it. */
export const nextHref = ({ pathname, hash }: Place) => {
	const place = parseProjectPath(CLASSIC_PROJECTS, pathname);
	if (!place) {
		return undefined;
	}
	const { slug, rest } = place;
	if (rest[0] === CLASSIC_PERF) {
		return projectPath(slug, NEXT_EXPLORE);
	}
	// Keys are one list in Settings, with no page of their own.
	if (rest[0] === KEYS) {
		return `${NEXT_PROJECTS}/${slug}/${KEYS}`;
	}
	// The new console creates nothing by hand, so a form lands on what it would edit.
	const last = rest.at(-1);
	const kept = last === "add" || last === "edit" ? rest.slice(0, -1) : rest;
	const target = kept.length === 1 && kept[0] === REPORTS ? [NEXT_RUNS] : kept;
	if (!tabOf(target)) {
		return undefined;
	}
	return `${NEXT_PROJECTS}/${slug}/${target.join("/")}${kept.length < rest.length ? "" : hash}`;
};

const parseProjectPath = (
	base: string,
	pathname: string,
): ProjectLocation | undefined => {
	if (!pathname.startsWith(`${base}/`)) {
		return undefined;
	}
	const [slug, ...rest] = pathname
		.slice(base.length + 1)
		.split("/")
		.filter((segment) => segment.length > 0);
	return slug ? { slug, rest } : undefined;
};
