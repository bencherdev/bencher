import type { APIRequestContext, Page } from "@playwright/test";
import { createProject, createReport } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, nextPath, seed, settle, signedIn, test } from "./fixtures";

const { hashbrown } = seed.projects;

const ALERTING = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";
const UNCHECKED = "blake3 input_bytes=65536 simd=avx2 threads=1 Throughput";

/** The newest `main` report on the seed's testbed, whose hash starts `9c1f2e4`. */
const newestMain = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const [report] = (await response.json()) as { uuid: string }[];
	if (!report) {
		throw new Error("The seed has no main report");
	}
	return report.uuid;
};

const reportPath = (slug: string, report: string, search = "") =>
	`${nextPath(slug, "reports")}/${report}${search}`;

const table = (page: Page) =>
	page.getByRole("table", { name: /^Lines in report / });
/** The line rows drawn, each named by its checkbox. */
const lineRows = (page: Page) =>
	table(page)
		.getByRole("row")
		.filter({ has: page.getByRole("checkbox") });
const lineRow = (page: Page, name: string) =>
	table(page)
		.getByRole("row")
		.filter({ has: page.getByRole("checkbox", { name: `Select ${name}` }) });
const lineNames = (page: Page) =>
	lineRows(page)
		.getByRole("checkbox")
		.evaluateAll((boxes) =>
			boxes.map((box) =>
				(box.getAttribute("aria-label") ?? "").replace(/^Select /, ""),
			),
		);

/** Every request for a report's lines, in the order sent. */
const lineRequests = (page: Page) => {
	const urls: URL[] = [];
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (
			request.url().startsWith(seed.api_url) &&
			url.pathname.includes("/console/reports/")
		) {
			urls.push(url);
		}
	});
	return urls;
};

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills a report whose alerting line is not first in its group, a value, delta,
// or limit read from the wrong field or printed in the wrong unit, a delta told
// by color alone, and an alerting row told by color alone.
test("the alerting line leads its group with its value, delta, and limit", async ({
	page,
	request,
}) => {
	await page.goto(reportPath(hashbrown.slug, await newestMain(request)));

	await expect(page.getByRole("heading", { level: 1 })).toHaveText(
		"main · ubuntu-latest",
	);
	await expect(
		page.getByText(
			"Sep 13, 2026, 21:16 · json · 9c1f2e4 · 4 benchmarks, 18 variants, 2 measures, 36 lines · took 2m 00s",
			{ exact: true },
		),
	).toBeVisible();
	await expect(table(page).getByRole("row").nth(1)).toHaveText(
		"blake3 · 8 variants, 16 lines · 1 alert",
	);
	await expect(
		lineRows(page).first().getByRole("checkbox"),
	).toHaveAccessibleName(`Select ${ALERTING}`);
	const row = lineRow(page, ALERTING);
	await expect(row.getByRole("img", { name: "alerting" })).toBeVisible();
	const cells = row.getByRole("cell");
	await expect(cells.nth(5)).toHaveText("20.60 ns");
	await expect(cells.nth(6)).toHaveText("↑ +6.2% worse");
	await expect(cells.nth(7)).toHaveText("19.54 ns");
});

// Kills a grouping or sort that never reaches the API, lives outside the URL,
// or is forgotten on reload.
test("grouping by measure and the delta sort reorder the rows and survive a reload", async ({
	page,
	request,
}) => {
	await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
	await expect(
		lineRows(page).first().getByRole("checkbox"),
	).toHaveAccessibleName(`Select ${ALERTING}`);
	const byName = await lineNames(page);

	await page.getByRole("radio", { name: "worst delta first" }).click();
	await expect(page).toHaveURL(/[?&]sort=delta(&|$)/);
	await expect.poll(() => lineNames(page)).not.toEqual(byName);
	const byDelta = await lineNames(page);
	expect(byDelta[0]).toBe(ALERTING);

	await page.getByRole("radio", { name: "Measure" }).click();
	await expect(table(page)).toHaveAccessibleName(
		"Lines in report 9c1f2e4, grouped by measure",
	);
	await expect(table(page).getByRole("row").nth(1)).toHaveText(
		"Latency · 18 variants, 18 lines · 1 alert",
	);
	const byMeasure = await lineNames(page);

	await page.reload();
	await expect(page.getByRole("radio", { name: "Measure" })).toBeChecked();
	await expect(
		page.getByRole("radio", { name: "worst delta first" }),
	).toBeChecked();
	await expect.poll(() => lineNames(page)).toEqual(byMeasure);
});

test.describe("a long report", () => {
	// Kills a first request that asks for every line, a next batch that never
	// loads or asks for another size, a list that draws every row it holds, and
	// a view change that asks again for the batches already loaded.
	test("scrolling loads it a batch at a time and draws only the rows on screen", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const { uuid } = await createReport(request, project.slug, {
			benchmarks: 20,
			variants: 15,
		});
		const requests = lineRequests(page);
		await page.goto(reportPath(project.slug, uuid));
		await expect(lineRows(page).first()).toBeVisible();
		await settle(page);
		expect(requests.map((url) => url.searchParams.get("page"))).toEqual(["1"]);
		const perPage = Number(requests[0]?.searchParams.get("per_page"));
		expect(perPage).toBeGreaterThan(0);
		expect(perPage).toBeLessThan(300);

		const last = lineRow(page, "bench-19 n=14 Latency");
		await expect
			.poll(
				async () => {
					await page.evaluate(() =>
						window.scrollTo({
							top: document.documentElement.scrollHeight,
							behavior: "instant",
						}),
					);
					return last.count();
				},
				{ timeout: 20_000 },
			)
			.toBe(1);
		const pages = requests.map((url) => Number(url.searchParams.get("page")));
		expect(pages).toEqual(pages.map((_, index) => index + 1));
		expect(pages.length).toBe(Math.ceil(300 / perPage));
		expect(
			requests.every(
				(url) => Number(url.searchParams.get("per_page")) === perPage,
			),
		).toBe(true);
		expect(await table(page).getByRole("row").count()).toBeLessThan(80);

		const failed: string[] = [];
		page.on("requestfailed", (request) => {
			failed.push(request.url());
		});
		const before = requests.length;
		await page.evaluate(() => window.scrollTo({ top: 0, behavior: "instant" }));
		await page.getByRole("radio", { name: "worst delta first" }).click();
		await expect(table(page)).not.toHaveAttribute("aria-busy", "true");
		await settle(page);
		expect(
			requests
				.slice(before)
				.map((url) => [
					url.searchParams.get("page"),
					url.searchParams.get("sort"),
				]),
		).toEqual([["1", "delta"]]);
		expect(failed).toEqual([]);
	});
});

// Kills a row that expands without its plot or key, rows that cannot be open
// together, and open rows left out of the URL.
test("two rows expand into two plots with their keys", async ({
	page,
	request,
}) => {
	await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
	await page.getByRole("button", { name: `Expand ${ALERTING}` }).click();
	await page.getByRole("button", { name: `Expand ${UNCHECKED}` }).click();

	for (const name of [ALERTING, UNCHECKED]) {
		const plot = page.getByRole("region", { name: `${name}, full plot` });
		await expect(plot.getByRole("button", { name: /^Hide / })).toBeVisible();
	}
	await expect(page).toHaveURL(/expanded=.*&expanded=/);
	await page.reload();
	await expect(page.getByRole("region", { name: /, full plot$/ })).toHaveCount(
		2,
	);
	await expect(
		page.getByRole("button", { name: `Collapse ${ALERTING}` }),
	).toHaveAttribute("aria-expanded", "true");
});

// Kills neighbors that are not the branch's and testbed's, links that lose the
// view, and a Next on the newest report.
test("Previous and Next walk the branch's reports", async ({
	page,
	request,
}) => {
	const newest = await newestMain(request);
	await page.goto(reportPath(hashbrown.slug, newest, "?group=measure"));
	await expect(page.getByRole("button", { name: "Next" })).toBeDisabled();

	await page
		.getByRole("link", {
			name: "Previous report on main, ubuntu-latest: Sep 11, 13:16, 0000000",
		})
		.click();
	await expect(page.getByText(/^Sep 11, 2026, 13:16 · json/)).toBeVisible();
	await expect(page).toHaveURL(/[?&]group=measure(&|$)/);

	await page
		.getByRole("link", {
			name: /^Next report on main, ubuntu-latest: Sep 13, 21:16, 9c1f2e4$/,
		})
		.click();
	await expect(page).toHaveURL(
		new RegExp(`/reports/${newest}\\?group=measure$`),
	);
	await expect(page.getByText(/^Sep 13, 2026, 21:16 · json/)).toBeVisible();
});

// Kills a selection that opens more or fewer lines than chosen, loses the
// report it came from, or does not reach Explore.
test("Open 2 in Explore opens exactly the selected lines", async ({
	page,
	request,
}) => {
	const report = await newestMain(request);
	await page.goto(reportPath(hashbrown.slug, report));
	await expect(
		page.getByRole("button", { name: "Open in Explore" }),
	).toBeDisabled();
	await page.getByRole("checkbox", { name: `Select ${ALERTING}` }).check();
	await page.getByRole("checkbox", { name: `Select ${UNCHECKED}` }).check();

	const open = page.getByRole("link", { name: "Open 2 lines in Explore" });
	const href = new URL(
		(await open.getAttribute("href")) ?? "",
		seed.console_url,
	);
	expect(href.pathname).toBe(nextPath(hashbrown.slug, "explore"));
	expect(href.searchParams.get("only")?.split(",")).toHaveLength(2);
	expect(href.searchParams.get("report")).toBe(report);
	await open.click();
	await expect(page).toHaveURL(/\/explore\?/);
});

// Kills a sheet without the exact threshold's parameters and metric, counts
// read from the lines loaded or swapped between the thresholds, a self-hosted
// server left out, and a simpler threshold that keeps the filter.
test("the no-threshold sheet offers the exact and the simpler threshold with their counts", async ({
	page,
	request,
}) => {
	await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
	await lineRow(page, UNCHECKED)
		.getByRole("button", {
			name: `No threshold checks ${UNCHECKED}. Show the run snippet that declares one.`,
		})
		.click();

	const sheet = page.getByRole("dialog", {
		name: "No threshold checks this line",
	});
	const exact = sheet.getByRole("region", { name: "Exactly this line" });
	await expect(exact).toContainText("export BENCHER_API_KEY=");
	await expect(exact).toContainText(`--host ${seed.api_url}`);
	await expect(exact).toContainText("--threshold-measure throughput");
	await expect(exact).toContainText("--threshold-metric value");
	await expect(exact).toContainText(
		`--threshold-parameters '{"input_bytes": 65536, "simd": "avx2", "threads": 1}'`,
	);
	await expect(exact).toContainText(
		"2 lines in this report, this one included",
	);

	const simple = sheet.getByRole("region", {
		name: "Every line of the measure",
	});
	await expect(simple).toContainText("--threshold-measure throughput");
	await expect(simple).toContainText("--threshold-metric value");
	await expect(simple).not.toContainText("--threshold-parameters");
	await expect(simple).toContainText(
		"18 lines in this report, this one included",
	);
});

// Kills a missing report drawn as a failure to retry, or as an empty report.
test("an unknown report says so", async ({ page }) => {
	await page.goto(
		reportPath(hashbrown.slug, "00000000-0000-4000-8000-000000000000"),
	);
	await expect(page.getByRole("heading", { level: 1 })).toHaveText(
		"Report not found",
	);
	await expect(
		page.getByRole("heading", { name: `${hashbrown.name} has no report here` }),
	).toBeVisible();
	await expect(
		page.getByRole("link", { name: "Open Reports" }),
	).toHaveAttribute("href", nextPath(hashbrown.slug, "reports"));
});

// Kills a malformed link, such as a short hash, sent to the API, which refuses
// it, and shown as a failure to retry.
test("a malformed report id is not found at once", async ({ page }) => {
	const requests = lineRequests(page);
	await page.goto(reportPath(hashbrown.slug, "9c1f2e4"));
	await expect(page.getByRole("heading", { level: 1 })).toHaveText(
		"Report not found",
	);
	await settle(page);
	expect(requests).toEqual([]);
});

// Kills a reader shown controls they cannot use without the line that says why.
test.describe("a reader who cannot edit", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	test("sees the read-only line and can still select lines", async ({
		page,
		request,
	}) => {
		await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
		await expect(
			page.getByText("Read only. Ask a project Maintainer for access."),
		).toBeVisible();
		await page.getByRole("checkbox", { name: `Select ${ALERTING}` }).check();
		await expect(page.getByText("1 line selected")).toBeVisible();
	});
});

// Kills a contrast or structure failure in either theme, with a row open.
test("the report passes axe in both themes", async ({ page, request }) => {
	await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
	await page.getByRole("button", { name: `Expand ${ALERTING}` }).click();
	const path = new URL(page.url()).pathname + new URL(page.url()).search;
	await axeInBothThemes(page, path, async () => {
		await expect(
			page
				.getByRole("region", { name: `${ALERTING}, full plot` })
				.getByRole("button", { name: /^Hide / }),
		).toBeVisible();
	});
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a narrow row that drops its numbers, its history, or its controls,
	// targets under 44 px, Clear's included, and a window that is not one chip.
	test("rows fold to two lines and still select and expand", async ({
		page,
		request,
	}) => {
		await page.goto(reportPath(hashbrown.slug, await newestMain(request)));
		const row = lineRow(page, ALERTING);
		await expect(
			row.getByRole("img", { name: `${ALERTING}, history` }),
		).toBeVisible();
		await expect(row).toContainText("20.60 ns");
		await expect(row).toContainText("+6.2% worse");
		await expect(
			page.getByRole("button", {
				name: "Window, ending at this report, 4 weeks",
			}),
		).toBeVisible();
		await expect(
			page.getByRole("link", { name: /^Previous report/ }),
		).toBeVisible();

		const select = row.getByRole("checkbox", { name: `Select ${ALERTING}` });
		const expand = row.getByRole("button", { name: `Expand ${ALERTING}` });
		for (const target of [select.locator(".."), expand]) {
			const box = await target.boundingBox();
			expect(box?.width).toBeGreaterThanOrEqual(44);
			expect(box?.height).toBeGreaterThanOrEqual(44);
		}
		await select.check();
		await expect(page.getByText("1 line selected")).toBeVisible();
		const clear = await page
			.getByRole("button", { name: "Clear", exact: true })
			.boundingBox();
		expect(clear?.width).toBeGreaterThanOrEqual(44);
		expect(clear?.height).toBeGreaterThanOrEqual(44);
		await expand.click();
		await expect(
			page
				.getByRole("region", { name: `${ALERTING}, full plot` })
				.getByRole("button", { name: /^Hide / }),
		).toBeVisible();
		const width = await page.evaluate(
			() => document.documentElement.scrollWidth,
		);
		expect(width).toBeLessThanOrEqual(390);
	});
});
