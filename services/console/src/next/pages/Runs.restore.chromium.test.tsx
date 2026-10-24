import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "../reports/reports.css";
import { MemoryRouter, Route, createMemoryHistory } from "@solidjs/router";
import { QueryClientProvider } from "@tanstack/solid-query";
import { Suspense } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test, vi } from "vitest";
import { page } from "vitest/browser";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import { fakeApi } from "../reports/testing";
import { runFixture, runPageFixture, runReportFixture } from "../runs/testing";
import Runs from "./Runs";

/** The rows' code, held back until the test lets it arrive. */
const code = vi.hoisted(() => {
	let release = () => {};
	const arrived = new Promise<void>((resolve) => {
		release = resolve;
	});
	return { arrived, release: () => release() };
});

vi.mock("../runs/RunChildren", async (original) => {
	await code.arrived;
	return original();
});

afterEach(() => {
	document.body.replaceChildren();
	window.history.replaceState(null, "");
});

// Kills a restored opened run whose rows, read before their code arrives,
// hold no place while it loads, so its arrival pushes the runs below.
test("a restored opened run holds its rows' place until their code arrives", async () => {
	await page.viewport(1280, 720);
	const [opened, below] = [
		runFixture(0, { reports: 2 }),
		runFixture(1, { reports: 1 }),
	];
	const { api, requests } = fakeApi((url) =>
		url.pathname.endsWith(`/runs/${opened.uuid}`)
			? {
					data: runPageFixture([runReportFixture(0), runReportFixture(1)], {
						uuid: opened.uuid,
					}),
				}
			: { data: { runs: [opened, below], total: 2 } },
	);
	window.history.replaceState({ runs: [opened.uuid] }, "");
	const history = createMemoryHistory();
	history.set({ value: "/hashbrown" });
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const dispose = render(
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
	const table = page.getByRole("table", { name: "Runs, newest first" });
	const last = () =>
		document
			.querySelector(`a[href$="${below.uuid}"]`)
			?.closest("tr")
			?.getBoundingClientRect().top;

	await expect.element(table).toBeVisible();
	await expect.poll(() => requests.length).toBe(2);
	await expect
		.poll(() => document.querySelectorAll('tbody tr[aria-busy="true"]').length)
		.toBe(2);
	// The reports have answered by now, so only the code is awaited.
	await new Promise((resolve) => setTimeout(resolve, 200));
	expect(document.querySelectorAll('tbody tr[aria-busy="true"]')).toHaveLength(
		2,
	);
	const held = last();
	expect(held).toBeDefined();

	code.release();
	await expect
		.element(table.getByRole("link", { name: /^ubuntu-latest, report 2, / }))
		.toBeVisible();
	expect(document.querySelectorAll('tbody tr[aria-busy="true"]')).toHaveLength(
		0,
	);
	expect(last()).toBe(held);
	dispose();
});
