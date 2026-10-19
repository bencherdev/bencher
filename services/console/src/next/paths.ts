export const CONSOLE_PATH = "/console";
export const PROJECTS_PATH = `${CONSOLE_PATH}/projects`;
/** The account pages, under the reader's slug. */
export const USERS_PATH = `${CONSOLE_PATH}/users`;

export const profilePath = (user: string) => `${USERS_PATH}/${user}/settings`;

/** The reader's organizations, with each one's pages under its slug or UUID. */
export const ORGANIZATIONS_PATH = `${CONSOLE_PATH}/organizations`;

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
		{ tab: "runs", label: "Runs", segments: ["runs"] },
		{ tab: "alerts", label: "Alerts", segments: ["alerts"] },
		{ tab: "thresholds", label: "Thresholds", segments: ["thresholds"] },
		{ tab: "plots", label: "Plots", segments: ["plots"] },
		{
			tab: "settings",
			label: "Settings",
			// Dimensions and keys live in Settings.
			segments: ["settings", ...DIMENSIONS, "keys"],
		},
	];

export const projectPath = (slug: string, tab?: Tab) =>
	`${PROJECTS_PATH}/${slug}/${tab ?? ""}`;

/** A report's page, which sits outside the tab row's paths and belongs to Runs. */
const REPORTS = "reports";

export const reportPath = (slug: string, report: string) =>
	`${PROJECTS_PATH}/${slug}/${REPORTS}/${report}`;

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
	| "Metric"
	| "Dimensions"
	| "Profile"
	| "UserKeys"
	| "ConnectedApps"
	| "LegacyTokens"
	| "OrgGeneral"
	| "OrgMembers"
	| "OrgBilling"
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
	// A metric's landing, which opens Explore on its line.
	if (rest.length === 2 && rest[0] === "metrics") {
		return "Metric";
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

export const parseConsolePath = (pathname: string) =>
	parseProjectPath(PROJECTS_PATH, pathname);

const isReport = (rest: string[]) => rest.length === 2 && rest[0] === REPORTS;

/** The project root is Runs, and a report belongs to Runs. */
export const tabOf = (rest: string[]): Tab | undefined => {
	const [segment] = rest;
	if (segment === undefined) {
		return "runs";
	}
	if (isReport(rest)) {
		return "runs";
	}
	return TABS.find(({ segments }) => segments.includes(segment))?.tab;
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
