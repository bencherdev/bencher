import type { Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import {
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

const reports = (search = "") =>
	`${nextPath(hashbrown.slug, "reports")}${search}`;
const listPath = `/v0/projects/${hashbrown.slug}/reports`;

const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const table = (page: Page) =>
	page.getByRole("table", { name: "Reports, newest first" });
/** The rows under the header, as many as are drawn. */
const rows = (page: Page) =>
	table(page).getByRole("rowgroup").last().getByRole("row");

/** Every request for the list, in the order sent. */
const listRequests = (page: Page) => {
	const urls: URL[] = [];
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (request.url().startsWith(seed.api_url) && url.pathname === listPath) {
			urls.push(url);
		}
	});
	return urls;
};

// The seed's newest report is a run by a project key on its own testbed, half an
// hour after the newest `main` report on the same commit; the clock is frozen at
// 2026-09-14T00:00Z, and the default window is four weeks.
test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills rows out of order, a cell that reads the wrong field, a run by a key
// named as nobody or as a person, a count that is the rows loaded rather than
// the total, and an absolute time missing from hover and from the row's name.
test("rows are newest first and read each report, a run by a project key included", async ({
	page,
}) => {
	await page.goto(reports());

	await expect(heading(page)).toHaveText("16 reports");
	const [byKey, byPerson] = [rows(page).nth(0), rows(page).nth(1)];
	await expect(byKey.getByRole("cell")).toHaveText([
		"2h ago",
		"main",
		seed.last_main_hash,
		"macos-latest",
		"magic",
		"4",
		"0",
		"4m 12s",
		"GitHub Actions",
	]);
	await expect(byKey.getByRole("img", { name: "Project key" })).toBeVisible();
	await expect(byPerson.getByRole("cell")).toHaveText([
		"2h ago",
		"main",
		seed.last_main_hash,
		"ubuntu-latest",
		"json",
		"36",
		/^1 active\s*1 total$/,
		"2m 00s",
		seed.member.user.name,
	]);
	await expect(byPerson.getByRole("img", { name: "User" })).toBeVisible();

	await expect(byKey.getByRole("link")).toHaveAccessibleName(
		`Report on main, macos-latest, magic, Sep 13, 2026, 21:46, ${seed.last_main_hash}`,
	);
	await expect(byKey.getByRole("cell").first().locator("time")).toHaveAttribute(
		"title",
		"Sep 13, 2026, 21:46",
	);
});

// Kills a filter or window that never reaches the API, asks for it twice,
// lives outside the URL, or is forgotten on reload.
test("a filter and the window narrow the rows and survive a reload", async ({
	page,
}) => {
	await page.goto(reports());
	await expect(heading(page)).toHaveText("16 reports");
	await settle(page);
	const requests = listRequests(page);

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	await page.getByRole("menuitemradio", { name: "main" }).click();
	await expect(heading(page)).toHaveText("13 reports");
	await settle(page);
	expect(requests.map((url) => url.searchParams.get("branch"))).toEqual([
		"main",
	]);
	await page.getByRole("radio", { name: "1w" }).click();
	await expect(heading(page)).toHaveText("4 reports");
	await expect(rows(page)).toHaveCount(4);
	await expect(page).toHaveURL(/[?&]branch=main(&|$)/);
	await settle(page);
	expect(requests).toHaveLength(2);

	await page.reload();
	await expect(heading(page)).toHaveText("4 reports");
	await expect(rows(page)).toHaveCount(4);
	await expect(page.getByRole("radio", { name: "1w" })).toBeChecked();
	await expect(
		page.getByRole("button", { name: "Filter by branch, main" }),
	).toBeVisible();

	await page.getByRole("radio", { name: "All" }).click();
	await expect(heading(page)).toHaveText("25 reports");
});

test.describe("on a short screen", () => {
	test.use({ viewport: { width: 1280, height: 600 } });

	// Kills a first request that asks for everything, a next batch that never
	// loads or asks for another size, a count that grows with the rows loaded,
	// and a list that draws every row it holds.
	test("scrolling loads the next batch and the count stays the total", async ({
		page,
	}) => {
		const requests = listRequests(page);
		await page.goto(reports("?window=all"));
		await expect(heading(page)).toHaveText("28 reports");
		await expect(table(page)).toHaveAttribute("aria-rowcount", "29");
		await settle(page);
		expect(requests.map((url) => url.searchParams.get("page"))).toEqual(["1"]);
		const perPage = Number(requests[0]?.searchParams.get("per_page"));
		expect(perPage).toBeGreaterThan(0);
		expect(perPage).toBeLessThan(28);

		const oldest = table(page).locator('[aria-rowindex="29"]');
		await expect(async () => {
			await page.mouse.wheel(0, 1_000);
			await expect(oldest).toBeInViewport({ timeout: 250 });
		}).toPass();

		await expect(heading(page)).toHaveText("28 reports");
		const batches = Math.ceil(28 / perPage);
		expect(requests.map((url) => url.searchParams.get("page"))).toEqual(
			Array.from({ length: batches }, (_, i) => String(i + 1)),
		);
		expect(
			new Set(requests.map((url) => url.searchParams.get("per_page"))),
		).toEqual(new Set([String(perPage)]));
		await expect.poll(() => rows(page).count()).toBeLessThan(28);
	});
});

// Kills a row link that reloads the document, leads anywhere but its report,
// or loads the report page's code only once clicked.
test("a row opens its report page, its code loaded on hover", async ({
	page,
}) => {
	await page.goto(reports());
	const link = rows(page).first().getByRole("link");
	await expect(link).toBeVisible();
	await settle(page);

	const documents: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
	});
	const code = page.waitForRequest((request) =>
		/\/Report\.[^/]*\.js$/.test(new URL(request.url()).pathname),
	);
	await link.hover();
	await code;
	await link.click();

	await expect(page).toHaveURL(
		new RegExp(`${nextPath(hashbrown.slug, "reports")}/[0-9a-f-]{36}$`),
	);
	await expect(
		page.getByRole("heading", { level: 1, name: "Report", exact: true }),
	).toBeVisible();
	expect(documents).toEqual([]);
});

// Kills a Reports link that waits for the click to ask for its first batch,
// and a page that asks again for what the hover already fetched.
test("hovering the Reports tab fetches its first batch", async ({ page }) => {
	await page.goto(nextPath(hashbrown.slug, "thresholds"));
	await expect(
		page.getByRole("heading", { level: 1, name: "Thresholds" }),
	).toBeVisible();
	await settle(page);

	const requests = listRequests(page);
	const link = tabRow(page).getByRole("link", { name: "Reports" });
	await link.hover();
	await expect.poll(() => requests.length).toBe(1);
	await link.click();
	await expect(heading(page)).toHaveText("16 reports");
	await settle(page);
	expect(requests).toHaveLength(1);
});

// Kills a filter menu that suspends the page while its options load: the
// table leaves the screen and the search box loses the focus and the text.
test("a filter menu loading its options leaves the page and the typing alone", async ({
	page,
}) => {
	await page.goto(reports());
	await expect(heading(page)).toHaveText("16 reports");
	await page.route(`${seed.api_url}/**/branches**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 1_000));
		await route.continue();
	});

	await page.getByRole("button", { name: "Filter by branch, any" }).click();
	const search = page.getByRole("searchbox", { name: "Search branches" });
	await expect(search).toBeFocused();
	await page.keyboard.type("fea", { delay: 30 });
	await page.waitForTimeout(500);
	await expect(table(page)).toBeVisible();
	await page.keyboard.type("t", { delay: 30 });
	await expect(
		page.getByRole("menuitemradio", { name: "feature-simd" }),
	).toBeVisible();
	await expect(search).toBeFocused();
	await expect(search).toHaveValue("feat");
	await expect(table(page)).toBeVisible();
});

// Kills a zero state that hides which filters emptied the list, and a Clear
// filters that keeps them or drops the window.
test("filters that match nothing say which, and clearing them brings rows back", async ({
	page,
}) => {
	await page.goto(reports("?branch=feature-simd&adapter=magic&window=1w"));

	await expect(
		page.getByRole("heading", {
			level: 2,
			name: "No reports match these filters",
		}),
	).toBeVisible();
	await expect(
		page.getByText(
			"Nothing on feature-simd with the magic adapter in the last week.",
		),
	).toBeVisible();
	await expect(heading(page)).toHaveText("0 reports");

	await page.getByRole("button", { name: "Clear filters" }).click();
	await expect(heading(page)).toHaveText("7 reports");
	await expect(page).toHaveURL(/\/reports\?window=1w$/);
});

// Kills names, roles, and contrast that fail in either theme.
test("Reports passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(page, reports(), async () => {
		await expect(rows(page).first()).toBeVisible();
	});
});

test.describe("narrow", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a table that keeps its columns on a phone, a window or filters left
	// in a row of controls, a Filters sheet that does not open, stops being
	// modal once a filter is picked, or forgets what it applied, a row whose
	// numbers are not on the right, a row only its text opens, and targets
	// (inputs included) under 44 px.
	test("rows fold, the window is one chip, and the filters open in a sheet", async ({
		page,
	}) => {
		await page.goto(reports());
		await expect(heading(page)).toHaveText("16 reports");
		await expect(
			page.getByRole("columnheader", { name: "Lines" }),
		).toBeHidden();
		await expect(page.getByRole("radiogroup", { name: "Window" })).toBeHidden();

		const first = rows(page).first();
		const row = await first.boundingBox();
		const alerts = await first
			.getByRole("cell", { name: "0", exact: true })
			.boundingBox();
		expect(row?.height).toBeGreaterThanOrEqual(44);
		expect((row?.x ?? 0) + (row?.width ?? 0)).toBeLessThanOrEqual(390);
		expect((alerts?.x ?? 0) + (alerts?.width ?? 0)).toBeGreaterThan(
			(row?.x ?? 0) + (row?.width ?? 0) - 40,
		);

		const windowChip = page.getByRole("button", { name: "Window, 4 weeks" });
		await windowChip.click();
		await page.getByRole("menuitemradio", { name: "1 week" }).click();
		await expect(heading(page)).toHaveText("7 reports");

		await page.getByRole("button", { name: "Filters, none applied" }).click();
		const sheet = page.getByRole("dialog", { name: "Filters" });
		await expect(sheet).toBeVisible();
		await sheet.getByRole("button", { name: "Filter by branch, any" }).click();
		const search = sheet.getByRole("searchbox", { name: "Search branches" });
		expect((await search.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		await sheet.getByRole("menuitemradio", { name: "feature-simd" }).click();
		// The sheet is still the modal dialog on top, so a second filter in it can be picked.
		expect(
			await page.evaluate(() =>
				document.querySelector("dialog")?.matches(":modal"),
			),
		).toBe(true);
		await sheet.getByRole("button", { name: "Filter by adapter, any" }).click();
		await expect(
			sheet.getByRole("menuitemradio", { name: "Any adapter" }),
		).toBeVisible();
		await page.keyboard.press("Escape");
		await sheet.getByRole("button", { name: "Done" }).click();
		await expect(sheet).toBeHidden();
		await expect(heading(page)).toHaveText("3 reports");
		const filtersChip = page.getByRole("button", {
			name: "Filters, 1 applied",
		});
		await expect(filtersChip).toBeVisible();

		for (const target of [
			page.getByRole("button", { name: "Window, 1 week" }),
			filtersChip,
		]) {
			const size = await target.boundingBox();
			expect(size?.height).toBeGreaterThanOrEqual(44);
		}

		await page.getByRole("button", { name: "Window, 1 week" }).click();
		await page.getByRole("menuitemradio", { name: "Custom range" }).click();
		for (const label of ["From", "To"]) {
			const size = await page.getByLabel(label, { exact: true }).boundingBox();
			expect(size?.height).toBeGreaterThanOrEqual(44);
		}

		await axeInBothThemes(page, page.url(), async () => {
			await expect(rows(page).first()).toBeVisible();
		});

		const target = await rows(page).first().boundingBox();
		await page.mouse.click(
			(target?.x ?? 0) + (target?.width ?? 0) - 8,
			(target?.y ?? 0) + (target?.height ?? 0) / 2,
		);
		await expect(page).toHaveURL(
			new RegExp(`${nextPath(hashbrown.slug, "reports")}/[0-9a-f-]{36}$`),
		);
	});
});
