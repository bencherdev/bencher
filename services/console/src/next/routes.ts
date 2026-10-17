import type { RouteDefinition, RoutePreloadFunc } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { type Component, lazy, useContext } from "solid-js";
import { type PageName, TABS, pageName } from "./paths";
import { ProjectContext } from "./project";

type Page = Component & { preload: () => Promise<unknown> };

const runsPage = () => import("./pages/Runs");
// Reading only the default export spares the bundler a namespace object.
const alertPage = () =>
	import("./pages/Alert").then(({ default: page }) => ({ default: page }));

/** Each page is its own chunk, loaded on first need or when a link to it is hovered. */
export const PAGES: Record<PageName, Page> = {
	// Reading only the default export spares the bundler a namespace object,
	// whose helper it would load from an unrelated chunk: Explore's chunk also
	// holds code its plot shares.
	Explore: lazy<Component>(() =>
		import("./pages/Explore").then(({ default: page }) => ({ default: page })),
	),
	// Every pinned plot draws, so the plot's code starts with the page's rather than a round after it.
	Plots: lazy(() => {
		import("./plot/Plot").catch(() => {});
		return import("./pages/Plots");
	}),
	Runs: lazy(runsPage),
	// Reading only the default export spares the bundler a namespace object.
	Run: lazy<Component>(() =>
		import("./pages/Run").then(({ default: page }) => ({ default: page })),
	),
	// Reading only the default export spares the bundler a namespace object,
	// whose helper it would load from an unrelated chunk.
	Report: lazy<Component>(() =>
		import("./pages/Report").then(({ default: page }) => ({ default: page })),
	),
	Alerts: lazy<Component>(() =>
		import("./pages/Alerts").then(({ default: page }) => ({ default: page })),
	),
	// An alert draws its plot at once, so the plot's code starts with the page's.
	Alert: lazy<Component>(() => {
		import("./plot/Plot").catch(() => {});
		return alertPage();
	}),
	Thresholds: lazy(() => import("./pages/Thresholds")),
	Threshold: lazy(() => import("./pages/Threshold")),
	Settings: lazy(() => import("./pages/Settings")),
	Dimensions: lazy(() => import("./pages/Dimensions")),
	Profile: lazy(() => import("./pages/Profile")),
	NotFound: lazy(() => import("./pages/NotFound")),
};

// A link to Runs hovered, focused, or touched starts its first batch too.
const runsData: RoutePreloadFunc = ({ params, location }) => {
	const client = useQueryClient();
	const project = useContext(ProjectContext);
	const slug = params.project;
	if (!(project && slug)) {
		return;
	}
	runsPage()
		.then(({ prefetch }) =>
			prefetch(client, project.api, slug, location.search),
		)
		.catch(() => {});
};

/** Relative to the router's base, the new console's project path. */
export const ROUTES: RouteDefinition[] = [
	{ path: "/:project", component: PAGES.Explore },
	{ path: "/:project/runs/:run", component: PAGES.Run },
	{ path: "/:project/reports/:report", component: PAGES.Report },
	{ path: "/:project/thresholds/:threshold", component: PAGES.Threshold },
	{ path: "/:project/alerts/:alert", component: PAGES.Alert },
	...TABS.flatMap(({ segments }) =>
		segments.map((segment) => {
			const page = pageName([segment]);
			return {
				path: `/:project/${segment}/*rest`,
				component: PAGES[page],
				...(page === "Runs" ? { preload: runsData } : {}),
			};
		}),
	),
	{ path: "/:project/*rest", component: PAGES.NotFound },
];

/** Where the project routes sit under the new console's root. */
export const PROJECTS_ROUTE = "/projects";

/** Relative to the new console's root, every path its router owns. */
export const CONSOLE_ROUTES: RouteDefinition[] = [
	{ path: PROJECTS_ROUTE, children: ROUTES },
	{ path: "/users/:user/settings", component: PAGES.Profile },
];
