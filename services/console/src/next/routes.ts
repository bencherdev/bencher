import type { RouteDefinition, RoutePreloadFunc } from "@solidjs/router";
import { useQueryClient } from "@tanstack/solid-query";
import { type Component, lazy, useContext } from "solid-js";
import { type PageName, TABS, pageName } from "./paths";
import { ProjectContext } from "./project";

type Page = Component & { preload: () => Promise<unknown> };

const reportsPage = () => import("./pages/Reports");

/** Each page is its own chunk, loaded on first need or when a link to it is hovered. */
export const PAGES: Record<PageName, Page> = {
	// Reading only the default export spares the bundler a namespace object,
	// whose helper it would load from an unrelated chunk: Explore's chunk also
	// holds code its plot shares.
	Explore: lazy<Component>(() =>
		import("./pages/Explore").then(({ default: page }) => ({ default: page })),
	),
	Plots: lazy(() => import("./pages/Plots")),
	Reports: lazy(reportsPage),
	// Reading only the default export spares the bundler a namespace object,
	// whose helper it would load from an unrelated chunk.
	Report: lazy<Component>(() =>
		import("./pages/Report").then(({ default: page }) => ({ default: page })),
	),
	Alerts: lazy(() => import("./pages/Alerts")),
	Thresholds: lazy(() => import("./pages/Thresholds")),
	Threshold: lazy(() => import("./pages/Threshold")),
	Settings: lazy(() => import("./pages/Settings")),
	NotFound: lazy(() => import("./pages/NotFound")),
};

// A link to Reports hovered, focused, or touched starts its first batch too.
const reportsData: RoutePreloadFunc = ({ params, location }) => {
	const client = useQueryClient();
	const project = useContext(ProjectContext);
	const slug = params.project;
	if (!(project && slug)) {
		return;
	}
	reportsPage()
		.then(({ prefetch }) =>
			prefetch(client, project.api, slug, location.search),
		)
		.catch(() => {});
};

/** Relative to the router's base, the new console's project path. */
export const ROUTES: RouteDefinition[] = [
	{ path: "/:project", component: PAGES.Explore },
	{ path: "/:project/reports/:report", component: PAGES.Report },
	{ path: "/:project/thresholds/:threshold", component: PAGES.Threshold },
	...TABS.flatMap(({ segments }) =>
		segments.map((segment) => {
			const page = pageName([segment]);
			return {
				path: `/:project/${segment}/*rest`,
				component: PAGES[page],
				...(page === "Reports" ? { preload: reportsData } : {}),
			};
		}),
	),
	{ path: "/:project/*rest", component: PAGES.NotFound },
];
