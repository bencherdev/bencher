import "@bencherdev/ui/styles.css";
import "../styles/console.css";
import "./reports.css";
import { QueryClientProvider } from "@tanstack/solid-query";
import { Suspense, createSignal } from "solid-js";
import { render } from "solid-js/web";
import { afterEach, expect, test } from "vitest";
import { page, userEvent } from "vitest/browser";
import type { Api } from "../api";
import { createQueryClient } from "../cache";
import { ProjectContext } from "../project";
import Controls from "./Controls";
import {
	ADAPTERS,
	DEFAULT_SEARCH,
	type ReportsSearch,
	customWindow,
	dateValue,
} from "./search";
import { fakeApi, watchFallback } from "./testing";

const DAY = 24 * 60 * 60 * 1_000;
const NOW = Date.parse("2026-09-14T12:00:00Z");

let dispose: (() => void) | undefined;
const searches: ReportsSearch[] = [];

/** Controls under a Suspense boundary, as the page sits under the layout's. */
const mount = (
	api: Api,
	initial: ReportsSearch = DEFAULT_SEARCH,
	names: { branch?: string; testbed?: string } = {},
) => {
	searches.length = 0;
	const root = document.createElement("div");
	root.className = "console";
	document.body.append(root);
	dispose = render(() => {
		const [search, setSearch] = createSignal(initial);
		return (
			<QueryClientProvider client={createQueryClient(() => {})}>
				<ProjectContext.Provider value={{ api, slug: () => "hashbrown" }}>
					<Suspense fallback={<p>Suspended</p>}>
						<main class="page">
							<Controls
								search={search()}
								names={names}
								now={NOW}
								onSearch={(next) => {
									searches.push(next);
									setSearch(next);
								}}
							/>
						</main>
					</Suspense>
				</ProjectContext.Provider>
			</QueryClientProvider>
		);
	}, root);
};

const branch = (name: string, slug: string) => ({ name, slug, uuid: slug });

afterEach(() => {
	dispose?.();
	document.body.replaceChildren();
});

// Kills options read while they load, which suspends the page: it leaves the
// screen, and the search box loses the focus and what was typed.
test("a filter menu whose options are loading leaves the page and the focus alone", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi(() => new Promise<never>(() => {}));
	mount(api);
	const suspended = () => document.body.textContent?.includes("Suspended");

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	const search = page.getByRole("searchbox", { name: "Search branches" });
	await expect.element(search).toHaveFocus();
	await userEvent.keyboard("fea");
	await new Promise((resolve) => setTimeout(resolve, 400));
	await userEvent.keyboard("t");

	await expect.element(search).toHaveFocus();
	await expect.element(search).toHaveValue("feat");
	expect(suspended()).toBe(false);
	await expect
		.element(page.getByRole("menuitemradio", { name: "Any branch" }))
		.toBeVisible();
});

// Kills a filter menu whose options, arriving or changing, put the layout's
// fallback in place of the page.
test("options that arrive and change never take the page out", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi(async (url) => {
		await new Promise((resolve) => setTimeout(resolve, 50));
		const text = url.searchParams.get("search") ?? "";
		return {
			data: [
				branch("main", "main"),
				branch("feature-simd", "feature-simd"),
			].filter(({ name }) => name.includes(text)),
		};
	});
	const stop = watchFallback();
	mount(api);

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	await expect
		.element(page.getByRole("menuitemradio", { name: "main" }))
		.toBeVisible();
	await userEvent.keyboard("fea");
	await expect
		.element(page.getByRole("menuitemradio", { name: "main" }))
		.not.toBeInTheDocument();
	await expect
		.element(page.getByRole("searchbox", { name: "Search branches" }))
		.toHaveFocus();
	expect(stop()).toBe(false);
});

// Kills a search box whose text never reaches the API, for branches or testbeds.
test("the branch and testbed menus ask the API for what is typed", async () => {
	await page.viewport(1280, 720);
	const { api, requests } = fakeApi(() => ({ data: [] }));
	mount(api);
	const asked = (resource: string, text: string) =>
		requests.some(
			(url) =>
				url.pathname === `/v0/projects/hashbrown/${resource}` &&
				url.searchParams.get("search") === text,
		);

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	await userEvent.keyboard("fea");
	await expect.poll(() => asked("branches", "fea")).toBe(true);
	await userEvent.keyboard("{Escape}");

	await page.getByRole("button", { name: "Filter by testbed, any" }).click();
	await userEvent.keyboard("mac");
	await expect.poll(() => asked("testbeds", "mac")).toBe(true);
});

// Kills options that blink empty while a new search loads, and a pick that
// writes anything but the slug.
test("options stay while the next search loads, and a pick filters by slug", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi((url) =>
		url.searchParams.get("search") === "412"
			? new Promise<never>(() => {})
			: { data: [branch("main", "main"), branch("412/merge", "412-merge")] },
	);
	mount(api);

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	const merge = page.getByRole("menuitemradio", { name: "412/merge" });
	await expect.element(merge).toBeVisible();
	await userEvent.keyboard("412");
	await new Promise((resolve) => setTimeout(resolve, 400));
	await expect.element(merge).toBeVisible();

	await merge.click();
	expect(searches.at(-1)?.branch).toBe("412-merge");
});

// Kills chips that show the slug of a branch or testbed whose name differs.
test("the chips name the branch and the testbed", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi(() => ({ data: [] }));
	mount(
		api,
		{ ...DEFAULT_SEARCH, branch: "412-merge", testbed: "linux-x86-64" },
		{ branch: "412/merge", testbed: "Linux x86 64" },
	);

	await expect
		.element(page.getByRole("button", { name: "Filter by branch, 412/merge" }))
		.toHaveTextContent("branch412/merge");
	await expect
		.element(
			page.getByRole("button", { name: "Filter by testbed, Linux x86 64" }),
		)
		.toBeVisible();
});

// Kills a custom window that does not start as the four weeks ending today.
test("Custom starts as the four weeks ending today", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi(() => ({ data: [] }));
	mount(api);

	await page.getByRole("radio", { name: "Custom" }).click();
	expect(searches.at(-1)?.window).toEqual(
		customWindow(dateValue(NOW - 27 * DAY), dateValue(NOW)),
	);
});

// Kills a From that moves the end, or a To that moves the start.
test("From moves the start of a custom range and To its end", async () => {
	await page.viewport(1280, 720);
	const { api } = fakeApi(() => ({ data: [] }));
	const range = customWindow("2026-08-01", "2026-08-31");
	mount(api, { ...DEFAULT_SEARCH, window: range ?? DEFAULT_SEARCH.window });

	await page.getByLabelText("From", { exact: true }).fill("2026-08-10");
	expect(searches.at(-1)?.window).toEqual(
		customWindow("2026-08-10", "2026-08-31"),
	);
	await page.getByLabelText("To", { exact: true }).fill("2026-08-20");
	expect(searches.at(-1)?.window).toEqual(
		customWindow("2026-08-10", "2026-08-20"),
	);
});

// Kills filter menus in the phone's Filters sheet that open past the bottom
// of the screen, where options are reached only by scrolling the sheet, or
// beside their chip rather than below it, and, on a short phone where the
// sheet scrolls, a menu left partly out of view.
test("on a phone, every option of a filter menu in the Filters sheet is on screen or a scroll of the menu away", async () => {
	const { api } = fakeApi(() => new Promise<never>(() => {}));
	for (const [width, height] of [
		[390, 844],
		[375, 560],
	] as const) {
		await page.viewport(width, height);
		mount(api);
		await page.getByRole("button", { name: "Filters, none applied" }).click();
		const sheet = page.getByRole("dialog", { name: "Filters" });

		for (const [name, count] of [
			["adapter", ADAPTERS.length + 1],
			["alerts", 2],
		] as const) {
			const chip = sheet.getByRole("button", {
				name: new RegExp(`^Filter by ${name},`),
			});
			await chip.click();
			const list = page
				.getByRole("menu", { name: `Filter by ${name}` })
				.element() as HTMLElement;
			expect(list.getBoundingClientRect().top).toBeGreaterThanOrEqual(
				chip.element().getBoundingClientRect().bottom,
			);
			const items = [
				...list.querySelectorAll<HTMLElement>('[role="menuitemradio"]'),
			];
			expect(items).toHaveLength(count);
			for (const item of items) {
				list.scrollTop +=
					item.getBoundingClientRect().top - list.getBoundingClientRect().top;
				const box = item.getBoundingClientRect();
				const hit = document.elementFromPoint(
					box.left + box.width / 2,
					box.top + box.height / 2,
				);
				expect(
					hit !== null && item.contains(hit),
					`${height}: ${item.textContent}`,
				).toBe(true);
			}
			await userEvent.keyboard("{Escape}");
			await expect.element(sheet).toBeVisible();
		}
		dispose?.();
		dispose = undefined;
		document.body.replaceChildren();
	}
});
