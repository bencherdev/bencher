import type { APIRequestContext, Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import { dimensions, explore, twoLines } from "./explore";
import { expect, consolePath, seed, signedIn, test } from "./fixtures";
import { loadedAndIdle, observeLayoutShift } from "./speed";
import ceilings from "./speed-ceilings.json" with { type: "json" };

// The seed's iterations project: six runs of three iterations on main, the
// newest raising an alert on its second iteration's latency.
const { iterations, hashbrown } = seed.projects;
const LATENCY = "blake3 input_bytes=65536 Latency";
const THROUGHPUT = "blake3 input_bytes=65536 Throughput";
const NEWEST = ["18.68 ns", "25.30 ns", "20.43 ns"];

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

const combo = (page: Page, includeHidden = false) =>
	page.getByRole("combobox", { name: "Iteration", includeHidden });
/** A readout row of one iteration, its label then its value. */
const iterationRow = (iteration: number, value: string) =>
	new RegExp(`iteration ${iteration}\\s*${value}`);

/** The project's newest run. */
const newestRun = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/projects/${iterations.slug}/reports?per_page=1`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const [report] = (await response.json()) as { uuid: string }[];
	if (!report) {
		throw new Error("The seed has no run of three iterations");
	}
	return report.uuid;
};

/** Explore drawing the latency every run ran three times. */
const latencyPlot = async (request: APIRequestContext) => {
	const id = await dimensions(request, iterations.slug);
	return explore(iterations.slug, {
		branches: [{ uuid: id.branch("main") }],
		testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
		benchmarks: [id.benchmark("blake3")],
		measures: [id.measure("Latency")],
	});
};

const reportPath = (report: string) =>
	`${consolePath(iterations.slug, "reports")}/${report}`;

/** The plot area inside `scope`, which reads out the report under the pointer. */
const plotArea = (scope: Page | ReturnType<Page["getByRole"]>) =>
	scope.getByRole("group", { name: /^1 line, / });

/** Pins the readout at the newest report, the plot's last, as the keys do, and returns it. */
const pinNewest = async (scope: Page | ReturnType<Page["getByRole"]>) => {
	const area = plotArea(scope);
	await area.focus();
	await area.press("End");
	return scope.getByRole("figure").getByRole("status");
};

/**
 * How many pixels of the plot's column at its newest report its canvas drew,
 * as a share of the plot's height: a range bar across the report's iterations
 * fills most of it, a single point a little.
 */
const inkAtNewest = (scope: Page | ReturnType<Page["getByRole"]>) =>
	plotArea(scope).evaluate((area) => {
		const canvas = area.parentElement?.querySelector("canvas");
		const context = canvas?.getContext("2d");
		if (!(canvas && context)) {
			throw new Error("The plot has no canvas");
		}
		const plotBox = area.getBoundingClientRect();
		const canvasBox = canvas.getBoundingClientRect();
		const ratio = canvas.width / canvasBox.width;
		const x = Math.round((plotBox.right - canvasBox.left) * ratio) - 1;
		const top = Math.round((plotBox.top - canvasBox.top) * ratio);
		const bottom = Math.round((plotBox.bottom - canvasBox.top) * ratio);
		const { data } = context.getImageData(x, top, 1, bottom - top);
		let drawn = 0;
		for (let alpha = 3; alpha < data.length; alpha += 4) {
			if ((data[alpha] ?? 0) > 40) {
				drawn++;
			}
		}
		return drawn / (bottom - top);
	});

test.describe("Explore", () => {
	// Kills reports whose iterations draw as one zigzag or not at all, a combo
	// that draws every iteration whatever it says, one the URL forgets, Back
	// that keeps a choice, and a Clear that keeps the iteration.
	test("draws each report's iterations as a bar, and the combo draws one that Back and Clear undo", async ({
		page,
		request,
	}) => {
		await page.goto(await latencyPlot(request));
		await expect(combo(page)).toBeVisible();
		await expect(combo(page)).toHaveValue("all");
		await expect(combo(page).locator("option")).toHaveText([
			"All",
			"1",
			"2",
			"3",
		]);
		const status = await pinNewest(page);
		for (const [index, value] of NEWEST.entries()) {
			await expect(status).toContainText(iterationRow(index + 1, value));
		}
		await expect(status.getByRole("img", { name: "alerted" })).toHaveCount(1);
		expect(await inkAtNewest(page)).toBeGreaterThan(0.5);

		await combo(page).selectOption("2");
		await expect(page).toHaveURL(/[?&]iteration=2(&|$)/);
		await pinNewest(page);
		await expect(status).toContainText(iterationRow(2, NEWEST[1] ?? ""));
		await expect(status).not.toContainText("iteration 1");
		expect(await inkAtNewest(page)).toBeLessThan(0.3);

		await page.goBack();
		await expect(page).not.toHaveURL(/iteration=/);
		await expect(combo(page)).toHaveValue("all");
		await expect.poll(() => inkAtNewest(page)).toBeGreaterThan(0.5);

		await page.goForward();
		await expect(combo(page)).toHaveValue("2");
		await page.getByRole("button", { name: "Clear", exact: true }).click();
		await expect(page).not.toHaveURL(/iteration=/);
		await expect(combo(page, true)).toHaveCount(0);
	});

	// Kills a combo offered where every report ran once, or its place dropped there.
	test("holds the combo's place, hidden, where every report ran once", async ({
		page,
		request,
	}) => {
		const { query } = await twoLines(request);
		await page.goto(explore(hashbrown.slug, query));
		await expect(page.getByRole("figure")).toHaveAccessibleName("2 lines");
		await expect(combo(page, true)).toBeAttached();
		await expect(combo(page, true)).toBeHidden();
	});
});

test.describe("a report run three times", () => {
	// Kills a row that names its last iteration when its second alerted, a tag
	// counted from 0, a combo that asks the API again or leaves the rows as
	// they were, one the URL forgets, and Back that keeps it.
	test("each row names its iteration, and the combo shows one in every row without a request", async ({
		page,
		request,
	}) => {
		const asked: string[] = [];
		page.on("request", (sent) => {
			if (/\/v0\/console\/projects\/[^/]+\/reports\//.test(sent.url())) {
				asked.push(sent.url());
			}
		});
		await page.goto(reportPath(await newestRun(request)));
		const latency = page.getByRole("row").filter({
			has: page.getByRole("checkbox", { name: `Select ${LATENCY}` }),
		});
		const throughput = page.getByRole("row").filter({
			has: page.getByRole("checkbox", { name: `Select ${THROUGHPUT}` }),
		});
		await expect(latency).toContainText("iteration 2 of 3");
		await expect(latency.getByRole("cell").nth(5)).toHaveText(NEWEST[1] ?? "");
		await expect(latency.getByRole("img", { name: "alerting" })).toBeVisible();
		await expect(throughput).toContainText("iteration 3 of 3");
		await expect(combo(page)).toBeVisible();
		const sent = asked.length;

		await combo(page).selectOption("1");
		await expect(page).toHaveURL(/[?&]iteration=1(&|$)/);
		await expect(latency).toContainText("iteration 1 of 3");
		await expect(latency.getByRole("cell").nth(5)).toHaveText(NEWEST[0] ?? "");
		await expect(latency.getByRole("img", { name: "alerting" })).toHaveCount(0);
		await expect(throughput).toContainText("iteration 1 of 3");

		await page.goBack();
		await expect(page).not.toHaveURL(/iteration=/);
		await expect(latency).toContainText("iteration 2 of 3");
		await expect(latency.getByRole("cell").nth(5)).toHaveText(NEWEST[1] ?? "");
		await expect(combo(page)).toHaveValue("all");
		expect(asked.length).toBe(sent);
	});

	// Kills a row's plot that draws every iteration once one is chosen, or reads out only one where all are drawn.
	test("a row's plot reads out every iteration of the report, then only the chosen one", async ({
		page,
		request,
	}) => {
		await page.goto(reportPath(await newestRun(request)));
		await page.getByRole("button", { name: `Expand ${LATENCY}` }).click();
		const region = page.getByRole("region", { name: `${LATENCY}, full plot` });
		await expect(plotArea(region)).toBeVisible();
		const status = await pinNewest(region);
		for (const [index, value] of NEWEST.entries()) {
			await expect(status).toContainText(iterationRow(index + 1, value));
		}
		expect(await inkAtNewest(region)).toBeGreaterThan(0.5);

		await combo(page).selectOption("3");
		const chosen = page.getByRole("region", { name: `${LATENCY}, full plot` });
		await expect(plotArea(chosen)).toBeVisible();
		const only = await pinNewest(chosen);
		await expect(only).toContainText(iterationRow(3, NEWEST[2] ?? ""));
		await expect(only).not.toContainText("iteration 2");
	});

	// Kills a combo offered for a report run once.
	test("a report run once offers no combo", async ({ page, request }) => {
		const response = await request.get(
			`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
			{ headers: { Authorization: `Bearer ${seed.member.token}` } },
		);
		const [report] = (await response.json()) as { uuid: string }[];
		await page.goto(
			`${consolePath(hashbrown.slug, "reports")}/${report?.uuid}`,
		);
		await expect(
			page.getByRole("table", { name: /^Lines in report / }),
		).toBeVisible();
		await expect(combo(page, true)).toBeAttached();
		await expect(combo(page, true)).toBeHidden();
	});
});

// Kills a combo whose arrival with the data moves what is already drawn: one
// without its held place, at the widths where it would take a line of its own.
for (const [width, where] of [
	[390, "Explore"],
	[320, "the report page"],
	[1100, "the report page"],
] as const) {
	test(`the combo arrives without moving anything on ${where} at ${width}`, async ({
		page,
		request,
	}) => {
		await page.setViewportSize({ width, height: 900 });
		await observeLayoutShift(page);
		await page.goto(
			where === "Explore"
				? await latencyPlot(request)
				: reportPath(await newestRun(request)),
		);
		await expect(combo(page)).toBeVisible();
		await loadedAndIdle(page);
		expect(
			await page.evaluate(() => window.__layoutShift ?? 1),
		).toBeLessThanOrEqual(ceilings.coldLoad.cls);
	});
}

// Kills an iteration combo, a readout of iterations, or a row tag whose names,
// roles, or colors fail axe in either theme or on a phone.
test("the combo, the readout of iterations, and the row tags pass axe in both themes", async ({
	page,
	request,
}) => {
	const explored = await latencyPlot(request);
	await axeInBothThemes(
		page,
		explored,
		async () => {
			await expect(page.getByRole("figure").getByRole("status")).toContainText(
				iterationRow(3, NEWEST[2] ?? ""),
			);
		},
		async () => {
			await expect(combo(page)).toBeVisible();
			await pinNewest(page);
		},
	);
	const report = reportPath(await newestRun(request));
	await axeInBothThemes(
		page,
		report,
		async () => {
			await expect(
				page
					.getByRole("region", { name: `${LATENCY}, full plot` })
					.getByRole("status"),
			).toContainText(iterationRow(3, NEWEST[2] ?? ""));
		},
		async () => {
			await expect(combo(page)).toBeVisible();
			await page.getByRole("button", { name: `Expand ${LATENCY}` }).click();
			await pinNewest(
				page.getByRole("region", { name: `${LATENCY}, full plot` }),
			);
		},
	);
});
