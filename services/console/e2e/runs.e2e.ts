import type { Locator, Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import {
	consolePath,
	crumb,
	expect,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";

const { hashbrown } = seed.projects;

const runs = (search = "") => `${consolePath(hashbrown.slug, "runs")}${search}`;
const listPath = `/v0/console/projects/${hashbrown.slug}/runs`;

const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const table = (page: Page) =>
	page.getByRole("table", { name: "Runs, newest first" });
/** The rows under the header, as many as are drawn. */
const rows = (page: Page) =>
	table(page).getByRole("rowgroup").last().getByRole("row");

/** The title a pointer resting on the middle of `target` reaches, as a hover shows it. */
const hoverTitle = async (page: Page, target: Locator) => {
	const box = await target.boundingBox();
	expect(box).not.toBeNull();
	return page.evaluate(
		([x, y]) =>
			document
				.elementFromPoint(x ?? 0, y ?? 0)
				?.closest("[title]")
				?.getAttribute("title"),
		[
			(box?.x ?? 0) + (box?.width ?? 0) / 2,
			(box?.y ?? 0) + (box?.height ?? 0) / 2,
		],
	);
};

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

// The seed's newest run is two reports on one commit: the newest `main` report
// and, half an hour later, a project key's on its own testbed. The clock is
// frozen at 2026-09-14T00:00Z, and the default window is four weeks.
test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills rows out of order, a run split into its reports, a cell that reads the
// wrong field, a count that is the rows loaded rather than the total, and an
// absolute time missing from hover and from the row's name.
test("rows are newest first, one per run, and read each run", async ({
	page,
}) => {
	await page.goto(runs());

	await expect(heading(page)).toHaveText("15 runs");
	const newest = rows(page).nth(0);
	await expect(newest.getByRole("cell")).toHaveText([
		"2h ago",
		"main",
		seed.last_main_hash,
		"· 2 reports",
		"ubuntu-latest+1",
		"json+1",
		"40",
		/^1 active\s*1 total$/,
		"34m 12s",
		`${seed.member.user.name}+1`,
	]);
	await expect(newest.getByRole("img", { name: "User" })).toBeVisible();
	expect(await hoverTitle(page, newest.getByText("+1").first())).toBe(
		"ubuntu-latest, macos-latest",
	);
	await expect(newest.getByRole("button")).toHaveAccessibleName(
		`main, run ${seed.last_main_hash}, 2 reports, ubuntu-latest, macos-latest, json, magic, by ${seed.member.user.name}, GitHub Actions, Sep 13, 2026, 21:16`,
	);
	await expect(newest.getByRole("button")).toHaveAttribute(
		"aria-expanded",
		"false",
	);
	await expect(newest.getByRole("link")).toHaveAccessibleName(
		seed.last_main_hash,
	);
	await expect(
		newest.getByRole("cell").first().locator("time"),
	).toHaveAttribute("title", "Sep 13, 2026, 21:16");
});

// Kills a split run read as one report per row, and a run without a hash
// drawn blank or without its version on hover.
test("a split run is one row, and a run without a hash reads as its version", async ({
	page,
}) => {
	await page.goto(runs("?branch=feature-simd&window=all"));
	await expect(heading(page)).toHaveText("5 runs");
	const split = rows(page).filter({ hasText: seed.split_hash });
	await expect(split.getByRole("cell")).toHaveText([
		/ago$/,
		"feature-simd",
		seed.split_hash,
		"· 4 reports",
		"ubuntu-latest+1",
		"json",
		"144",
		"0",
		"2m 04s",
		seed.member.user.name,
	]);
	const head = table(page).getByText(/^v\d+$/);
	await expect(head).toBeVisible();
	expect(await hoverTitle(page, head)).toBe(
		`Version ${(await head.textContent())?.slice(1)}`,
	);
});

// Kills a filter or window that never reaches the API, asks for it twice,
// lives outside the URL, or is forgotten on reload.
test("a filter and the window narrow the rows and survive a reload", async ({
	page,
}) => {
	await page.goto(runs());
	await expect(heading(page)).toHaveText("15 runs");
	await settle(page);
	const requests = listRequests(page);

	await page.getByRole("button", { name: "branch any" }).click();
	await page.getByRole("menuitemradio", { name: "main" }).click();
	await expect(heading(page)).toHaveText("12 runs");
	await settle(page);
	expect(requests.map((url) => url.searchParams.get("branch"))).toEqual([
		"main",
	]);
	await page.getByRole("radio", { name: "1w" }).click();
	await expect(heading(page)).toHaveText("3 runs");
	await expect(rows(page)).toHaveCount(3);
	await expect(page).toHaveURL(/[?&]branch=main(&|$)/);
	await settle(page);
	expect(requests).toHaveLength(2);

	await page.reload();
	await expect(heading(page)).toHaveText("3 runs");
	await expect(rows(page)).toHaveCount(3);
	await expect(page.getByRole("radio", { name: "1w" })).toBeChecked();
	await expect(page.getByRole("button", { name: "branch main" })).toBeVisible();

	await page.getByRole("radio", { name: "All" }).click();
	await expect(heading(page)).toHaveText("24 runs");
});

// Kills a testbed or adapter filter that keeps only the matching reports
// rather than their whole run, and an alerts filter that keeps quiet runs.
test("the testbed, adapter, and alerts filters keep whole runs", async ({
	page,
}) => {
	await page.goto(runs("?testbed=macos-latest&window=all"));
	await expect(heading(page)).toHaveText("2 runs");
	await expect(
		rows(page)
			.filter({ hasText: seed.last_main_hash })
			.getByRole("cell")
			.nth(3),
	).toHaveText("· 2 reports");
	await page.goto(runs("?adapter=magic"));
	await expect(heading(page)).toHaveText("1 run");
	await expect(rows(page).first().getByRole("cell").nth(3)).toHaveText(
		"· 2 reports",
	);
	await page.goto(runs("?alerts=active&window=all"));
	await expect(heading(page)).toHaveText("1 run");
	await expect(rows(page).first()).toContainText(seed.last_main_hash);
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
		await page.goto(runs("?window=all"));
		await expect(heading(page)).toHaveText("29 runs");
		await expect(table(page)).toHaveAttribute("aria-rowcount", "30");
		await settle(page);
		expect(requests.map((url) => url.searchParams.get("page"))).toEqual(["1"]);
		const perPage = Number(requests[0]?.searchParams.get("per_page"));
		expect(perPage).toBeGreaterThan(0);
		expect(perPage).toBeLessThan(29);

		const oldest = table(page).locator('[aria-rowindex="30"]');
		await expect(async () => {
			await page.mouse.wheel(0, 1_000);
			await expect(oldest).toBeInViewport({ timeout: 250 });
		}).toPass();

		await expect(heading(page)).toHaveText("29 runs");
		const batches = Math.ceil(29 / perPage);
		expect(requests.map((url) => url.searchParams.get("page"))).toEqual(
			Array.from({ length: batches }, (_, i) => String(i + 1)),
		);
		expect(
			new Set(requests.map((url) => url.searchParams.get("per_page"))),
		).toEqual(new Set([String(perPage)]));
		await expect.poll(() => rows(page).count()).toBeLessThan(29);
	});
});

// Kills a run's head that reloads the document, leads anywhere but its run, or
// loads the run page's code only once clicked, a run page that lists anything
// but the run's reports in start order, and a report row that leads anywhere
// but its report.
test("a run's head opens its run page, whose rows open each report", async ({
	page,
}) => {
	await page.goto(runs());
	const link = rows(page)
		.first()
		.getByRole("link", { name: seed.last_main_hash, exact: true });
	await expect(link).toBeVisible();
	await settle(page);

	const documents: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
	});
	const code = page.waitForRequest((request) =>
		/\/Run\.[^/]*\.js$/.test(new URL(request.url()).pathname),
	);
	await link.hover();
	await code;
	await link.click();

	await expect(page).toHaveURL(
		new RegExp(`${consolePath(hashbrown.slug, "runs")}/[0-9a-f-]{36}$`),
	);
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: `main ${seed.last_main_hash}`,
		}),
	).toBeVisible();
	await expect(page.locator(".rp-sub")).toHaveText(
		/^Sep 13, 2026, 21:16 · version \d+ · 2 reports, 2 testbeds · 40 lines · took 34m 12s$/,
	);
	const reports = page
		.getByRole("table", { name: "Reports in this run, by start" })
		.getByRole("rowgroup")
		.last()
		.getByRole("row");
	await expect(reports.nth(0).getByRole("cell")).toHaveText([
		"1",
		"ubuntu-latest",
		"json",
		"21:16:00",
		"36 lines",
		/^1 active\s*1 total$/,
		"2m 00s",
		seed.member.user.name,
	]);
	await expect(reports.nth(1).getByRole("cell")).toHaveText([
		"2",
		"macos-latest",
		"magic",
		"21:46:00",
		"4 lines",
		"0",
		"4m 12s",
		"GitHub Actions",
	]);
	await reports.nth(1).getByRole("link").click();
	await expect(
		page.getByRole("heading", { level: 1, name: "main · macos-latest" }),
	).toBeVisible();
	await page.getByRole("link", { name: "Runs", exact: true }).first().click();
	await expect(heading(page)).toHaveText("15 runs");
	expect(documents).toEqual([]);
});

// Kills a split run's reports out of start order, or numbered by anything but
// their start.
test("a split run lists its reports by start, told apart to the second", async ({
	page,
}) => {
	await page.goto(runs("?branch=feature-simd&window=all"));
	await rows(page)
		.getByRole("link", { name: seed.split_hash, exact: true })
		.click();
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: `feature-simd ${seed.split_hash}`,
		}),
	).toBeVisible();
	const reports = page
		.getByRole("table", { name: "Reports in this run, by start" })
		.getByRole("rowgroup")
		.last()
		.getByRole("row");
	await expect(reports).toHaveCount(4);
	const cells = async (index: number) =>
		(await reports.nth(index).getByRole("cell").allTextContents()).slice(0, 4);
	expect([
		await cells(0),
		await cells(1),
		await cells(2),
		await cells(3),
	]).toEqual([
		["1", "ubuntu-latest", "json", "00:00:00"],
		["2", "ubuntu-latest", "json", "00:00:02"],
		["3", "macos-latest", "json", "00:00:03"],
		["4", "ubuntu-latest", "json", "00:00:04"],
	]);
});

// Kills a run that opens its page rather than its reports in place, reports
// out of start order or in the wrong columns, a report row that leads anywhere
// but its report or reloads the document, an expand that loads the run page's
// code, opened runs kept in the URL or forgotten on Back, and a head that
// opens anything but the run's page.
test("a run opens in place to its reports, stays open across Back from one, and its head opens its page", async ({
	page,
}) => {
	const documents: string[] = [];
	const runCode: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
		if (/\/Run(Alerts)?\.[^/]*\.js$/.test(new URL(request.url()).pathname)) {
			runCode.push(request.url());
		}
	});
	const listed = runs("?branch=feature-simd&window=all");
	await page.goto(listed);
	await expect(heading(page)).toHaveText("5 runs");
	documents.length = 0;
	const split = rows(page).filter({ hasText: seed.split_hash });
	const toggle = split.getByRole("button", {
		name: new RegExp(`^feature-simd, run ${seed.split_hash}, 4 reports, `),
	});
	await toggle.click();
	await expect(toggle).toHaveAttribute("aria-expanded", "true");
	const reports = rows(page).filter({
		has: page.getByRole("link", { name: /, report \d, / }),
	});
	await expect(reports).toHaveCount(4);
	const cells = async (index: number) =>
		(await reports.nth(index).getByRole("cell").allTextContents()).slice(0, 6);
	expect([
		await cells(0),
		await cells(1),
		await cells(2),
		await cells(3),
	]).toEqual([
		["00:00:00", "1", "", "", "ubuntu-latest", "json"],
		["00:00:02", "2", "", "", "ubuntu-latest", "json"],
		["00:00:03", "3", "", "", "macos-latest", "json"],
		["00:00:04", "4", "", "", "ubuntu-latest", "json"],
	]);
	await settle(page);
	expect(runCode).toEqual([]);
	await expect(page).toHaveURL(listed);

	await reports.nth(2).getByRole("link").click();
	await expect(heading(page)).toHaveText("feature-simd · macos-latest");
	await expect(page.locator(".rp-sub")).toHaveText(/^#3 of 4 · /);
	await page.goBack();
	await expect(page).toHaveURL(listed);
	await expect(toggle).toHaveAttribute("aria-expanded", "true");
	await expect(reports).toHaveCount(4);

	await split.getByRole("link", { name: seed.split_hash, exact: true }).click();
	await expect(heading(page)).toHaveText(`feature-simd · ${seed.split_hash}`);
	expect(documents).toEqual([]);
});

/** Where the list's opened runs sit among its runs, in list order. */
const opened = (page: Page) =>
	table(page)
		.getByRole("button")
		.evaluateAll((toggles) =>
			toggles.flatMap((toggle, index) =>
				toggle.getAttribute("aria-expanded") === "true" ? [index] : [],
			),
		);

// Kills opened runs a search change keeps on the history entry it leaves, not
// the one it makes, so Back from a report closes them.
test("a run opened before a window change is still open on Back from one of its reports", async ({
	page,
}) => {
	await page.goto(runs("?branch=feature-simd&window=all"));
	const toggle = rows(page)
		.filter({ hasText: seed.split_hash })
		.getByRole("button");
	await toggle.click();
	await expect(toggle).toHaveAttribute("aria-expanded", "true");
	await page.getByRole("radio", { name: "3m" }).click();
	await expect(page.getByRole("radio", { name: "3m" })).toBeChecked();
	await expect(toggle).toHaveAttribute("aria-expanded", "true");

	await table(page)
		.getByRole("link", { name: /^macos-latest, report 3, / })
		.click();
	await expect(heading(page)).toHaveText("feature-simd · macos-latest");
	await page.goBack();
	await expect(page.getByRole("radio", { name: "3m" })).toBeChecked();
	await expect(toggle).toHaveAttribute("aria-expanded", "true");
});

// Kills a project root opened from the Runs tab that opens the tab's runs,
// and a page that, on the way back, writes the runs opened on one entry onto
// the other.
test("the Runs tab and the project root each keep their own opened runs across Back and Forward", async ({
	page,
}) => {
	await page.goto(runs());
	await table(page).getByRole("button").first().click();
	await expect.poll(() => opened(page)).toEqual([0]);

	// The Runs tab leads to the project root, a page of its own.
	await tabRow(page).getByRole("link", { name: "Runs" }).click();
	await expect(page).toHaveURL(consolePath(hashbrown.slug));
	await expect(heading(page)).toHaveText("15 runs");
	await expect.poll(() => opened(page)).toEqual([]);
	await table(page).getByRole("button").nth(1).click();
	await expect.poll(() => opened(page)).toEqual([1]);

	await page.goBack();
	await expect(page).toHaveURL(runs());
	await expect.poll(() => opened(page)).toEqual([0]);
	await page.goForward();
	await expect(page).toHaveURL(consolePath(hashbrown.slug));
	await expect.poll(() => opened(page)).toEqual([1]);
	await page.goBack();
	await expect.poll(() => opened(page)).toEqual([0]);
});

// Kills a list that, once the switcher moves it to another project, stops
// keeping the runs opened there or keeps the first project's, and one that on
// the way back opens anything but what each project's entry kept.
test("each project's list keeps its own opened runs across the switcher, Back, and Forward", async ({
	page,
}) => {
	const { runner } = seed.projects;
	await page.goto(consolePath(hashbrown.slug));
	const first = () => table(page).getByRole("button").first();
	await first().click();
	await expect(first()).toHaveAttribute("aria-expanded", "true");

	await crumb(page).click();
	const dialog = page.getByRole("dialog", { name: "Switch project" });
	await dialog
		.getByRole("combobox", { name: "Search projects" })
		.fill(runner.name);
	await dialog
		.getByRole("option", { name: new RegExp(`^${runner.name}`) })
		.click();
	await expect(page).toHaveURL(consolePath(runner.slug));
	await expect(
		table(page).getByRole("link", { name: seed.jobs_hash, exact: true }),
	).toBeVisible();
	await expect.poll(() => page.evaluate(() => history.state?.runs)).toEqual([]);
	expect(await opened(page)).toEqual([]);
	await first().click();
	await expect(first()).toHaveAttribute("aria-expanded", "true");

	await page.goBack();
	await expect(page).toHaveURL(consolePath(hashbrown.slug));
	await expect(
		table(page).getByRole("link", { name: seed.last_main_hash, exact: true }),
	).toBeVisible();
	await expect.poll(() => opened(page)).toEqual([0]);
	await page.goForward();
	await expect(page).toHaveURL(consolePath(runner.slug));
	await expect(
		table(page).getByRole("link", { name: seed.jobs_hash, exact: true }),
	).toBeVisible();
	await expect(first()).toHaveAttribute("aria-expanded", "true");
});

const runReports = (page: Page) =>
	page
		.getByRole("table", { name: "Reports in this run, by start" })
		.getByRole("rowgroup")
		.last()
		.getByRole("row");
const subLine = (page: Page) => page.locator(".rp-sub");

// Kills a move between a run and its reports, through the reports, back to the
// run, or on to the next run on the branch that reloads the document, steps
// out of the run or the branch, or that Back cannot walk again.
test("a run, its reports, and the next run walk together, and Back walks it all again", async ({
	page,
}) => {
	const documents: string[] = [];
	page.on("request", (request) => {
		if (request.resourceType() === "document") {
			documents.push(request.url());
		}
	});
	await page.goto(runs("?branch=feature-simd&window=all"));
	const split = `feature-simd ${seed.split_hash}`;
	await rows(page)
		.getByRole("link", { name: seed.split_hash, exact: true })
		.click();
	await expect(heading(page)).toHaveText(`feature-simd · ${seed.split_hash}`);
	await expect(page.getByRole("button", { name: "Previous" })).toBeDisabled();

	await runReports(page).nth(1).getByRole("link").click();
	await expect(heading(page)).toHaveText("feature-simd · ubuntu-latest");
	await expect(subLine(page)).toHaveText(
		/^#2 of 4 · Aug 10, 2026, 00:00:02 · json · /,
	);
	await page.getByRole("link", { name: /^Next #3 · macos-latest/ }).click();
	await expect(heading(page)).toHaveText("feature-simd · macos-latest");
	await expect(subLine(page)).toHaveText(/^#3 of 4 · /);
	await page.getByRole("link", { name: /^Next #4 · ubuntu-latest/ }).click();
	await expect(subLine(page)).toHaveText(/^#4 of 4 · /);
	await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();
	await page.getByRole("link", { name: /^Previous #3 · macos-latest/ }).click();
	await expect(subLine(page)).toHaveText(/^#3 of 4 · /);

	await page.getByRole("link", { name: split, exact: true }).click();
	await expect(heading(page)).toHaveText(`feature-simd · ${seed.split_hash}`);
	await page
		.getByRole("link", { name: /^Next .*: the run on feature-simd, v\d+$/ })
		.click();
	await expect(heading(page)).toHaveText(/^feature-simd · v\d+$/);
	await expect(
		page.getByRole("link", { name: /^Previous / }),
	).toHaveAccessibleName(
		new RegExp(`: the run on feature-simd, ${seed.split_hash}$`),
	);

	await page.goBack();
	await expect(heading(page)).toHaveText(`feature-simd · ${seed.split_hash}`);
	for (const place of ["#3", "#4", "#3", "#2"]) {
		await page.goBack();
		await expect(subLine(page)).toHaveText(new RegExp(`^${place} of 4 · `));
	}
	await page.goBack();
	await expect(heading(page)).toHaveText(`feature-simd · ${seed.split_hash}`);
	await page.goBack();
	await expect(table(page)).toBeVisible();
	expect(documents).toHaveLength(1);
});

// Kills a run's alerts that are not drawn, drawn without the report they came
// from, or a report link, a plot, or an Explore link that leads elsewhere.
test("the newest run's alert names its report, opens its plot in place, and opens in Explore", async ({
	page,
}) => {
	await page.goto(runs());
	await rows(page)
		.first()
		.getByRole("link", { name: seed.last_main_hash, exact: true })
		.click();
	const section = page.getByRole("region", { name: "Alerts in this run" });
	await expect(section.locator(".count")).toHaveText("1");
	const alerts = section.getByRole("table", {
		name: `Alerts in run ${seed.last_main_hash}, by report`,
	});
	await alerts.getByRole("button", { name: /^Expand blake3 / }).click();
	await expect(
		page.getByRole("region", { name: /^blake3 .*, full plot$/ }),
	).toBeVisible();
	await expect(
		alerts.getByRole("link", { name: /^Open in Explore: blake3 / }),
	).toHaveAttribute("href", /\/explore\?.*report=/);
	await alerts.getByRole("link", { name: /^#1 · ubuntu-latest/ }).click();
	await expect(heading(page)).toHaveText("main · ubuntu-latest");
	await expect(subLine(page)).toHaveText(/^#1 of 2 · /);
});

// Kills a Runs link that waits for the click to ask for its first batch, and
// a page that asks again for what the hover already fetched.
test("hovering the Runs tab fetches its first batch", async ({ page }) => {
	await page.goto(consolePath(hashbrown.slug, "plots"));
	await expect(
		page.getByRole("heading", { level: 1, name: "Plots" }),
	).toBeVisible();
	await settle(page);

	const requests = listRequests(page);
	const link = tabRow(page).getByRole("link", { name: "Runs" });
	await link.hover();
	await expect.poll(() => requests.length).toBe(1);
	await link.click();
	await expect(heading(page)).toHaveText("15 runs");
	await settle(page);
	expect(requests).toHaveLength(1);
});

// Kills a filter menu that suspends the page while its options load: the
// table leaves the screen and the search box loses the focus and the text.
test("a filter menu loading its options leaves the page and the typing alone", async ({
	page,
}) => {
	await page.goto(runs());
	await expect(heading(page)).toHaveText("15 runs");
	await page.route(`${seed.api_url}/**/branches**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 1_000));
		await route.continue();
	});

	await page.getByRole("button", { name: "branch any" }).click();
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
test("filters that match nothing show which, and clearing them brings rows back", async ({
	page,
}) => {
	await page.goto(runs("?branch=feature-simd&adapter=magic&window=1w"));

	await expect(
		page.getByRole("heading", { level: 2, name: "No runs" }),
	).toBeVisible();
	for (const name of ["branch feature-simd", "adapter magic"]) {
		await expect(page.getByRole("button", { name })).toBeVisible();
	}
	await expect(heading(page)).toHaveText("0 runs");

	await page.getByRole("button", { name: "Clear filters" }).click();
	await expect(heading(page)).toHaveText("6 runs");
	await expect(page).toHaveURL(/\/runs\?window=1w$/);
});

// Kills a Clear that leaves a filter or drops the window, asks for the list
// more than once, adds more than one history entry, or drops the focus, and a
// Back that does not bring the filter back.
test("Clear asks once for every run in the window, and Back brings the filter back", async ({
	page,
}) => {
	await page.goto(runs("?branch=main&window=1w"));
	await expect(heading(page)).toHaveText("3 runs");
	await settle(page);
	const requests = listRequests(page);
	const entries = await page.evaluate(() => history.length);
	const clear = page.getByRole("button", { name: "Clear filters" });

	await clear.click();
	await expect(heading(page)).toHaveText("6 runs");
	await expect(page).toHaveURL(/\/runs\?window=1w$/);
	await expect(page.getByRole("button", { name: "branch any" })).toBeFocused();
	await expect(clear).toHaveCount(0);
	await settle(page);
	expect(requests.map((url) => url.searchParams.has("branch"))).toEqual([
		false,
	]);
	expect(await page.evaluate(() => history.length)).toBe(entries + 1);

	await page.goBack();
	await expect(heading(page)).toHaveText("3 runs");
	await expect(page.getByRole("button", { name: "branch main" })).toBeVisible();
	await expect(clear).toBeVisible();
});

// Kills names, roles, and contrast that fail in either theme, at the project
// root where a reader lands, a run opened there included.
test("Runs at the project root with a run opened, and a run, pass axe in both themes", async ({
	page,
}) => {
	await axeInBothThemes(page, consolePath(hashbrown.slug), async () => {
		const toggle = rows(page).first().getByRole("button");
		// A reload keeps the run opened on its history entry.
		if ((await toggle.getAttribute("aria-expanded")) !== "true") {
			await toggle.click();
		}
		await expect(
			table(page).getByRole("link", { name: /, report 2, / }),
		).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
		).toBeVisible();
	});
	await page.goto(consolePath(hashbrown.slug));
	await rows(page)
		.first()
		.getByRole("link", { name: seed.last_main_hash, exact: true })
		.click();
	await expect(page).toHaveURL(/\/runs\/[0-9a-f-]{36}$/);
	await axeInBothThemes(page, page.url(), async () => {
		await expect(
			page
				.getByRole("table", { name: "Reports in this run, by start" })
				.getByRole("link")
				.first(),
		).toBeVisible();
		// The newest run alerted, so its alerts table, which loads with code of its own, is checked too.
		await expect(
			page
				.getByRole("table", { name: /^Alerts in run / })
				.getByRole("button", { name: /^Expand / })
				.first(),
		).toBeVisible();
	});
});

test.describe("narrow", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a table that keeps its columns on a phone, a window or filters left
	// in a row of controls, a Filters sheet that does not open, stops being
	// modal once a filter is picked, or forgets what it applied, a row whose
	// numbers are not on the right, a row only its text opens, an opened run's
	// reports that do not fold as tall as it, and targets (inputs included)
	// under 44 px.
	test("rows fold, the window is one chip, and the filters open in a sheet", async ({
		page,
	}) => {
		await page.goto(runs());
		await expect(heading(page)).toHaveText("15 runs");
		await expect(
			page.getByRole("columnheader", { name: "Lines" }),
		).toBeHidden();
		await expect(page.getByRole("radiogroup", { name: "Window" })).toBeHidden();

		const first = rows(page).first();
		const row = await first.boundingBox();
		const alerts = await first
			.getByRole("cell", { name: /total$/ })
			.boundingBox();
		expect(row?.height).toBeGreaterThanOrEqual(44);
		expect((row?.x ?? 0) + (row?.width ?? 0)).toBeLessThanOrEqual(390);
		expect((alerts?.x ?? 0) + (alerts?.width ?? 0)).toBeGreaterThan(
			(row?.x ?? 0) + (row?.width ?? 0) - 40,
		);

		const windowChip = page.getByRole("button", { name: "Window 4w, 4 weeks" });
		await windowChip.click();
		await page.getByRole("menuitemradio", { name: "1 week" }).click();
		await expect(heading(page)).toHaveText("6 runs");

		await page.getByRole("button", { name: "Filters, none applied" }).click();
		const sheet = page.getByRole("dialog", { name: "Filters" });
		await expect(sheet).toBeVisible();
		await sheet.getByRole("button", { name: "branch any" }).click();
		const search = sheet.getByRole("searchbox", { name: "Search branches" });
		expect((await search.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		await sheet.getByRole("menuitemradio", { name: "feature-simd" }).click();
		// The sheet is still the modal dialog on top, so a second filter in it can be picked.
		expect(
			await page.evaluate(() =>
				document.querySelector("dialog")?.matches(":modal"),
			),
		).toBe(true);
		await sheet.getByRole("button", { name: "adapter any" }).click();
		await expect(
			sheet.getByRole("menuitemradio", { name: "Any adapter" }),
		).toBeVisible();
		await page.keyboard.press("Escape");
		await sheet.getByRole("button", { name: "Done" }).click();
		await expect(sheet).toBeHidden();
		await expect(heading(page)).toHaveText("3 runs");
		const filtersChip = page.getByRole("button", {
			name: "Filters, 1 applied",
		});
		await expect(filtersChip).toBeVisible();

		for (const target of [
			page.getByRole("button", { name: "Window 1w, 1 week" }),
			filtersChip,
			rows(page).first().getByRole("link"),
		]) {
			const size = await target.boundingBox();
			expect(size?.height).toBeGreaterThanOrEqual(44);
		}

		await page.getByRole("button", { name: "Window 1w, 1 week" }).click();
		await page.getByRole("menuitemradio", { name: "Custom range" }).click();
		for (const label of ["From", "To"]) {
			const size = await page.getByLabel(label, { exact: true }).boundingBox();
			expect(size?.height).toBeGreaterThanOrEqual(44);
		}

		await axeInBothThemes(page, page.url(), async () => {
			await expect(rows(page).first()).toBeVisible();
		});

		// A run opens in place from the end of its row, its reports folding too.
		const target = await rows(page).first().boundingBox();
		await page.mouse.click(
			(target?.x ?? 0) + (target?.width ?? 0) - 8,
			(target?.y ?? 0) + (target?.height ?? 0) / 2,
		);
		await expect(rows(page).first().getByRole("button")).toHaveAttribute(
			"aria-expanded",
			"true",
		);
		await expect(rows(page).nth(1).getByRole("link")).toBeVisible();
		const child = await rows(page).nth(1).boundingBox();
		expect(child?.height).toBe(target?.height);
		expect((child?.x ?? 0) + (child?.width ?? 0)).toBeLessThanOrEqual(390);
		await rows(page).first().getByRole("link").click();
		await expect(page).toHaveURL(
			new RegExp(`${consolePath(hashbrown.slug, "runs")}/[0-9a-f-]{36}$`),
		);

		// The run's reports fold too, each opening its report from anywhere.
		const report = page
			.getByRole("table", { name: "Reports in this run, by start" })
			.getByRole("rowgroup")
			.last()
			.getByRole("row")
			.first();
		await expect(report).toBeVisible();
		const box = await report.boundingBox();
		expect(box?.height).toBeGreaterThanOrEqual(44);
		expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(390);
		await expect(
			page.getByRole("columnheader", { name: "Adapter" }),
		).toBeHidden();
		await page.mouse.click(
			(box?.x ?? 0) + (box?.width ?? 0) - 8,
			(box?.y ?? 0) + (box?.height ?? 0) / 2,
		);
		await expect(page).toHaveURL(
			new RegExp(`${consolePath(hashbrown.slug, "reports")}/[0-9a-f-]{36}$`),
		);
	});
});
