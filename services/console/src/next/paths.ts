/** The classic console's project pages. */
const CLASSIC_PROJECTS = "/console/projects";
/** The new console serves BMF v1 projects here until version 0 retires. */
export const NEXT_PROJECTS = `/next${CLASSIC_PROJECTS}`;

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
	| "reports"
	| "alerts"
	| "thresholds"
	| "settings";

/** The tab row, in order, with the path segments each tab owns. */
export const TABS: readonly { tab: Tab; label: string; segments: string[] }[] =
	[
		{ tab: "explore", label: "Explore", segments: ["explore"] },
		{ tab: "plots", label: "Plots", segments: ["plots"] },
		{ tab: "reports", label: "Reports", segments: ["reports"] },
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

export const reportPath = (slug: string, report: string) =>
	`${projectPath(slug, "reports")}/${report}`;

const TAB_PAGES = {
	explore: "Explore",
	plots: "Plots",
	reports: "Reports",
	alerts: "Alerts",
	thresholds: "Thresholds",
	settings: "Settings",
} as const satisfies Record<Tab, string>;

/** A file under `pages/`: each draws one kind of project path. */
export type PageName =
	| (typeof TAB_PAGES)[Tab]
	| "Report"
	| "Threshold"
	| "Dimensions"
	| "NotFound";

/** The page that draws a project path, from its segments after the project. */
export const pageName = (rest: string[]): PageName => {
	if (rest.length === 2 && rest[0] === "reports") {
		return "Report";
	}
	if (rest.length === 2 && rest[0] === "thresholds") {
		return "Threshold";
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

/** The project root is Explore. */
export const tabOf = (rest: string[]): Tab | undefined => {
	const [segment] = rest;
	if (segment === undefined) {
		return "explore";
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
	const kept = rest[0] === "settings" && rest[1] === KEYS ? [KEYS] : rest;
	return { path: `${CLASSIC_PROJECTS}/${slug}/${kept.join("/")}`, hash: true };
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
		return `${NEXT_PROJECTS}/${slug}/settings/${KEYS}`;
	}
	if (!tabOf(rest)) {
		return undefined;
	}
	// The new console creates nothing by hand, so a form lands on what it would edit.
	const last = rest.at(-1);
	const kept = last === "add" || last === "edit" ? rest.slice(0, -1) : rest;
	if (kept.length < rest.length) {
		return `${NEXT_PROJECTS}/${slug}/${kept.join("/")}`;
	}
	return `${NEXT_PROJECTS}/${slug}/${rest.join("/")}${hash}`;
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
