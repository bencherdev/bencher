import type { RouteDefinition } from "@solidjs/router";
import { type Component, lazy } from "solid-js";
import { TABS, type Tab } from "./paths";

type Page = Component & { preload: () => Promise<unknown> };

/** Each tab's page is its own chunk, loaded on first need or when a link to it is hovered. */
export const PAGES: Record<Tab, Page> = {
	explore: lazy(() => import("./pages/Explore")),
	plots: lazy(() => import("./pages/Plots")),
	reports: lazy(() => import("./pages/Reports")),
	alerts: lazy(() => import("./pages/Alerts")),
	thresholds: lazy(() => import("./pages/Thresholds")),
	settings: lazy(() => import("./pages/Settings")),
};

export const NOT_FOUND: Page = lazy(() => import("./pages/NotFound"));

/** Relative to the router's base, the new console's project path. */
export const ROUTES: RouteDefinition[] = [
	{ path: "/:project", component: PAGES.explore },
	...TABS.flatMap(({ tab, segments }) =>
		segments.map((segment) => ({
			path: `/:project/${segment}/*rest`,
			component: PAGES[tab],
		})),
	),
	{ path: "/:project/*rest", component: NOT_FOUND },
];
