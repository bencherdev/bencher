import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, beforeEach, expect, test } from "vitest";
import { page } from "vitest/browser";
import type { Api } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { runFixture, runPageFixture, runReportFixture } from "../runs/testing";
import Runs from "./Runs";

// Each test here watches code load that loads once per page, so each needs
// its chunk unloaded: this file loads none of them anywhere else.

let dispose: (() => void) | undefined;

const mount = (api: Api) => {
	const history = createMemoryHistory();
	history.set({ value: "/hashbrown" });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(
		() => (
			<QueryClientProvider client={createQueryClient(() => {})}>
				<MemoryRouter
					history={history}
					root={(props) => (
						<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
							<Suspense>{props.children}</Suspense>
						</ProjectContext.Provider>
					)}
				>
					<Route path="/:project" component={Runs} />
				</MemoryRouter>
			</QueryClientProvider>
		),
		root,
	);
};

/** When the page started fetching the module at `path`, once it has. */
const fetched = (path: string) =>
	performance
		.getEntriesByType("resource")
		.find((entry) => entry.name.includes(path))?.startTime;

const table = () => page.getByRole("table", { name: "Runs, newest first" });

beforeEach(async () => {
	await page.viewport(1280, 720);
});

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
	window.history.replaceState(null, "");
});

// Kills a restored opened run whose reports wait for the list, or whose rows'
// code waits for the list to draw them, each a round after the first.
test("a restored opened run reads its reports and loads its rows' code without waiting for the list", async () => {
	const opened = runFixture(0, { reports: 2 });
	const { api, requests } = fakeApi((url) =>
		url.pathname.endsWith(`/runs/${opened.uuid}`)
			? { data: runPageFixture([runReportFixture(0)], { uuid: opened.uuid }) }
			: new Promise<never>(() => {}),
	);
	window.history.replaceState({ runs: [opened.uuid] }, "");
	mount(api);
	await expect
		.poll(() =>
			requests.filter((url) => url.pathname.endsWith(`/runs/${opened.uuid}`)),
		)
		.toHaveLength(1);
	await expect.poll(() => fetched("/runs/RunChildren.tsx")).toBeDefined();
});

/** Whether the page has fetched the code that draws a run's alerts. */
const alertsCodeLoaded = () => fetched("/runs/RunAlerts.tsx") !== undefined;

// Kills a run page whose alerts' code is loaded for a run that raised none, or
// not before the reader opens a run that raised some.
test("moving onto the head of a run that raised alerts starts their code", async () => {
	const { api } = fakeApi(() => ({
		data: {
			runs: [runFixture(0), runFixture(1, { alerts: { total: 2, active: 1 } })],
			total: 2,
		},
	}));
	mount(api);
	await expect.element(table()).toBeVisible();
	const [quiet, alerting] = [
		...document.querySelectorAll(".reports-hash a"),
	] as [Element, Element];
	quiet.dispatchEvent(
		new PointerEvent("pointermove", { bubbles: true, movementX: 4 }),
	);
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(alertsCodeLoaded()).toBe(false);

	alerting.dispatchEvent(
		new PointerEvent("pointermove", { bubbles: true, movementX: 4 }),
	);
	await expect.poll(alertsCodeLoaded).toBe(true);
});
