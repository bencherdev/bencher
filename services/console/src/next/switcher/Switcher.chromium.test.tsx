import "./switcher.css";
import "../reports/reports.css";
import { MemoryRouter, Route } from "@solidjs/router";
import { QueryClient, QueryClientProvider } from "@tanstack/solid-query";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type {
	JsonAuthUser,
	JsonConsoleUser,
	JsonConsoleUserProject,
} from "../../types/bencher";
import { AccountContext } from "../account/context";
import { budget } from "../budget";
import { VERSION_KEY, recentSlugs } from "../memory";
import { fakeApi, mount } from "../settings/testing";
import Shell from "../shell/Shell";

const NOW = Date.now();
const HOUR = 60 * 60 * 1000;
/** A project with no reports, as the API sends it. */
const NEVER = null as unknown as number;
const POMPEII = { uuid: "o1", name: "Pompeii LLC", slug: "pompeii-llc" };
const DINER = { uuid: "o2", name: "Bagge Diner", slug: "bagge-diner" };

const project = (
	name: string,
	fields: Partial<JsonConsoleUserProject> = {},
): JsonConsoleUserProject => ({
	uuid: name,
	name,
	slug: name.toLowerCase().replace(/ /g, "-"),
	organization: POMPEII.uuid,
	bmf_version: 1,
	last_report: NOW - HOUR,
	active_alerts: 0,
	...fields,
});

const READER: JsonConsoleUser = {
	uuid: "u",
	name: "Muriel Bagge",
	slug: "muriel-bagge",
	organizations: [POMPEII, DINER],
	projects: [
		project("Hashbrown", { active_alerts: 5 }),
		project("Gizmo", { organization: DINER.uuid, last_report: NOW - 3 * HOUR }),
		project("Cathode", { bmf_version: 0, last_report: NOW - 4 * HOUR }),
		project("Cornbread", { last_report: NEVER }),
	],
};

let unmount: (() => void) | undefined;
afterEach(() => {
	unmount?.();
	localStorage.clear();
});

/**
 * The shell on Hashbrown's Runs, with the reader's projects already cached,
 * or with none cached and an API that fails.
 */
const open = async (
	reader: JsonConsoleUser | null = READER,
	memory: Record<string, number> = { hashbrown: 1 },
) => {
	await page.viewport(1280, 800);
	localStorage.setItem(VERSION_KEY, JSON.stringify(memory));
	const mounted = mount(
		() => (
			<Shell
				slug="hashbrown"
				tab="runs"
				organization={{ name: "Pompeii LLC", uuid: "o1" }}
				project="Hashbrown"
				reader="u"
			/>
		),
		{
			api: fakeApi(undefined, () => Promise.reject(new Error("down"))).api,
			path: "/hashbrown/runs",
			cached: reader ? [[["console", "user", "u"], reader]] : [],
		},
	);
	unmount = mounted.dispose;
	await crumb().click();
	await expect.element(search()).toHaveFocus();
	return mounted;
};

const crumb = () =>
	page.getByRole("button", { name: "Pompeii LLC / Hashbrown, switch project" });
const dialog = () => page.getByRole("dialog", { name: "Switch project" });
const search = () => page.getByRole("combobox", { name: "Search projects" });
const options = () => [
	...document.querySelectorAll<HTMLElement>('[role="option"]'),
];
const names = () =>
	options().map(
		(option) => option.querySelector(".sw-title > span")?.textContent,
	);
const active = () => {
	const id = search().element().getAttribute("aria-activedescendant");
	return id ? document.getElementById(id) : null;
};

// Kills a switcher that opens without its search focused, a crumb that does
// not say it is open, an Escape that leaves it open, and a close that drops
// the focus.
test("the crumb opens the switcher on its search, and Escape gives the focus back to the crumb", async () => {
	await open();
	await expect.element(crumb()).toHaveAttribute("aria-expanded", "true");

	await userEvent.keyboard("{Escape}");

	await expect.element(dialog()).not.toBeInTheDocument();
	await expect.element(crumb()).toHaveAttribute("aria-expanded", "false");
	await expect.element(crumb()).toHaveFocus();
});

// Kills an Escape that does nothing once the focus has moved back to the crumb.
test("Escape on the crumb closes it too", async () => {
	await open();

	await userEvent.keyboard("{Shift>}{Tab}{/Shift}");
	await expect.element(crumb()).toHaveFocus();
	await userEvent.keyboard("{Escape}");

	await expect.element(dialog()).not.toBeInTheDocument();
	await expect.element(crumb()).toHaveFocus();
});

// Kills a crumb that, pressed while open, closes the switcher and opens it again.
test("the crumb pressed while open closes it", async () => {
	await open();

	await crumb().click();

	await expect.element(dialog()).not.toBeInTheDocument();
	await expect.element(crumb()).toHaveAttribute("aria-expanded", "false");
});

// Kills a switcher left open behind the focus once it moves on.
test("the focus moving on closes it where the focus went", async () => {
	await open();
	const runs = page.getByRole("link", { name: "Runs", exact: true });

	(runs.element() as HTMLElement).focus();

	await expect.element(dialog()).not.toBeInTheDocument();
	await expect.element(runs).toHaveFocus();
});

// Kills an order that ignores the browser's memory of recent projects.
test("the projects this browser visited lead, most recent first", async () => {
	await open(READER, { cathode: 0, hashbrown: 1 });

	expect(names()).toEqual(["Hashbrown", "Cathode", "Gizmo", "Cornbread"]);
});

// Kills a reader with no projects shown and no word why when they fail to load.
test("projects that fail to load say so", async () => {
	await open(null);

	await expect
		.element(dialog().getByText("Projects did not load"))
		.toBeVisible();
});

// Kills a switcher that stays open over the page when the reader clicks away.
test("a click outside closes it", async () => {
	await open();

	await userEvent.click(document.body, { position: { x: 600, y: 700 } });

	await expect.element(dialog()).not.toBeInTheDocument();
});

// Kills rows that drop the organization of a reader of two, the Classic tag,
// No runs, the alerts, or the current project's mark, and a classic row that
// links into the new console.
test("each row names its organization, its last run, and its active alerts", async () => {
	await open();

	expect(names()).toEqual(["Hashbrown", "Gizmo", "Cathode", "Cornbread"]);
	const [hashbrown, gizmo, cathode, cornbread] = options();
	expect(hashbrown?.textContent).toContain("Current");
	expect(hashbrown?.textContent).toContain("5 active alerts");
	expect(gizmo?.textContent).toContain("Bagge Diner");
	expect(gizmo?.textContent).toContain("3h ago");
	expect(cathode?.textContent).toContain("Classic");
	expect(cathode?.getAttribute("href")).toBe("/console/projects/cathode/plots");
	expect(cornbread?.textContent).toContain("No runs");
	expect(cornbread?.getAttribute("href")).toBe(
		"/next/console/projects/cornbread/",
	);
});

// Kills a footer that drops New project or Settings, sends either to an
// organization other than the current project's, or sends Settings to the
// classic page the new console replaces.
test("the footer holds New project and the current organization's Settings", async () => {
	await open();

	await expect
		.element(dialog().getByRole("link", { name: "New project" }))
		.toHaveAttribute("href", "/next/console/organizations/o1/projects/add");
	await expect
		.element(dialog().getByRole("link", { name: "Pompeii LLC settings" }))
		.toHaveAttribute("href", "/next/console/organizations/o1/settings");
});

// Kills an organization's crumb that opens no switcher, one that cannot reach
// the reader's projects without a project's page around it, and a project
// marked current where none is.
test("an organization's crumb opens the switcher on the reader's projects", async () => {
	await page.viewport(1280, 800);
	const { api } = fakeApi(undefined, () => Promise.reject(new Error("down")));
	const client = new QueryClient();
	client.setQueryData(["console", "user", "u"], READER);
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	const dispose = render(
		() => (
			<QueryClientProvider client={client}>
				<AccountContext.Provider
					value={{
						api,
						reader: () => ({}) as JsonAuthUser,
						keepUser: () => {},
						signOut: () => {},
					}}
				>
					<MemoryRouter>
						<Route
							path="*"
							component={() => (
								<Shell
									org
									organization={{ name: "Pompeii LLC", uuid: "o1" }}
									reader="u"
								/>
							)}
						/>
					</MemoryRouter>
				</AccountContext.Provider>
			</QueryClientProvider>
		),
		root,
	);
	unmount = () => {
		dispose();
		root.remove();
	};

	await page
		.getByRole("button", { name: "Pompeii LLC, switch project" })
		.click();
	await expect.element(search()).toHaveFocus();
	await expect
		.poll(names)
		.toEqual(["Hashbrown", "Gizmo", "Cathode", "Cornbread"]);
	expect(document.querySelector(".sw-cur")).toBeNull();
	await expect
		.element(dialog().getByRole("link", { name: "Pompeii LLC settings" }))
		.toHaveAttribute("href", "/next/console/organizations/o1/settings");
});

// Kills arrows that do not move the active row, an active row the list does
// not report, an Enter that does not open it in the app, and a pick that does
// not lead the browser's memory.
test("arrow keys move through the rows and Enter opens one in the app", async () => {
	const { history } = await open();
	expect(active()).toBeNull();

	await userEvent.keyboard("{ArrowDown}{ArrowDown}{ArrowDown}{ArrowUp}");
	expect(active()?.textContent).toContain("Gizmo");
	expect(active()?.getAttribute("aria-selected")).toBe("true");

	await userEvent.keyboard("{Enter}");

	expect(history.get()).toBe("/next/console/projects/gizmo/");
	await expect.element(dialog()).not.toBeInTheDocument();
	// A pick leads the next opening even when nothing about it is fetched.
	expect(recentSlugs(localStorage)[0]).toBe("gizmo");
});

// Kills a search that filters nothing, one whose first match Enter cannot
// reach, no word when nothing matches, a Clear search that keeps the
// query or loses the focus, and a search that names a row or list not drawn.
test("typing searches; with no match the list reads No matches, and Clear search brings every row back", async () => {
	const { history } = await open();

	await userEvent.keyboard("diner");
	expect(names()).toEqual(["Gizmo"]);
	await userEvent.keyboard("{Enter}");
	expect(history.get()).toBe("/next/console/projects/gizmo/");

	await crumb().click();
	await userEvent.keyboard("zeppelin");
	expect(options()).toEqual([]);
	expect(search().element().hasAttribute("aria-activedescendant")).toBe(false);
	expect(search().element().hasAttribute("aria-controls")).toBe(false);
	await expect.element(dialog().getByText("No matches")).toBeVisible();

	await dialog().getByRole("button", { name: "Clear search" }).click();

	await expect.element(search()).toHaveValue("");
	await expect.element(search()).toHaveFocus();
	expect(names()).toHaveLength(4);
});

// Kills a reader with no projects shown an empty box, or a search to clear.
test("a reader with no projects sees No projects", async () => {
	await open({ ...READER, projects: [] });

	await expect.element(dialog().getByText("No projects")).toBeVisible();
	await expect
		.element(dialog().getByRole("button", { name: "Clear search" }))
		.not.toBeInTheDocument();
});

// Kills a list that draws every row, and an active row either arrow leaves
// out of view or undrawn.
test("a long list draws only the rows in view, and the arrows keep the active row in view", async () => {
	await open({
		...READER,
		projects: Array.from({ length: 600 }, (_, i) =>
			project(`Project ${String(i).padStart(3, "0")}`, {
				last_report: NOW - i * HOUR,
			}),
		),
	});
	expect(options().length).toBeLessThan(40);

	await userEvent.keyboard("{ArrowDown}".repeat(30));

	expect(active()?.textContent).toContain("Project 029");
	expectActiveInView();
	expect(active()?.getAttribute("aria-posinset")).toBe("30");
	expect(active()?.getAttribute("aria-setsize")).toBe("600");

	const list = document.querySelector<HTMLElement>('[role="listbox"]');
	list?.scrollTo({ top: 20_000 });
	await expect.poll(() => options()[0]?.textContent).not.toContain("Project 0");
	await userEvent.keyboard("{ArrowUp}");

	expect(active()?.textContent).toContain("Project 028");
	expectActiveInView();
});

/** The active row is drawn wholly inside the list's scrolled view. */
const expectActiveInView = () => {
	const list = document
		.querySelector('[role="listbox"]')
		?.getBoundingClientRect();
	const box = active()?.getBoundingClientRect();
	expect(box && list && box.top >= list.top && box.bottom <= list.bottom).toBe(
		true,
	);
};

const FRAME_MS = 8.3;
const median = (values: number[]) =>
	[...values].sort((a, b) => a - b)[Math.floor(values.length / 2)] ??
	Number.NaN;

// Kills an open or a keystroke that draws every row of a long list, or
// otherwise takes longer than a 120 Hz frame.
test("opening it and typing in its search each fit in a frame", async () => {
	await open({
		...READER,
		projects: Array.from({ length: 5000 }, (_, i) =>
			project(`Project ${i}`, {
				organization: i % 2 ? DINER.uuid : POMPEII.uuid,
				last_report: NOW - i * HOUR,
			}),
		),
	});
	const button = crumb().element() as HTMLElement;
	const opens: number[] = [];
	const types: number[] = [];
	for (let run = 0; run < 9; run++) {
		button.click();
		await expect.element(dialog()).not.toBeInTheDocument();
		const start = performance.now();
		button.click();
		while (!document.querySelector('[role="dialog"]')) {
			await Promise.resolve();
		}
		document.body.getBoundingClientRect();
		opens.push(performance.now() - start);

		const input = search().element() as HTMLInputElement;
		const typed = performance.now();
		input.value = `project ${run}`;
		input.dispatchEvent(new InputEvent("input", { bubbles: true }));
		document.body.getBoundingClientRect();
		types.push(performance.now() - typed);
		expect(options()[0]?.textContent).toContain(`Project ${run}`);
	}
	console.info(
		`speed switcher: open ${median(opens).toFixed(2)} ms, type ${median(types).toFixed(2)} ms, budget ${budget(FRAME_MS).toFixed(2)} ms`,
	);
	expect(median(opens)).toBeLessThan(budget(FRAME_MS));
	expect(median(types)).toBeLessThan(budget(FRAME_MS));
});
