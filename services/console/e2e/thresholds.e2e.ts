import type { APIRequestContext, Page } from "@playwright/test";
import type { JsonConsoleThresholds } from "../src/types/bencher";
import { createManyAlerts, createProject } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, nextPath, seed, settle, signedIn, test } from "./fixtures";
import {
	alertsTopToBottom,
	dismissWhileNextBatchIsOut,
	manyAlertNames,
	refocusThenReread,
} from "./overtake";

const { hashbrown } = seed.projects;

const thresholds = (slug: string, search = "") =>
	`${nextPath(slug, "thresholds")}${search}`;
const thresholdPath = (slug: string, uuid: string, search = "") =>
	`${thresholds(slug)}/${uuid}${search}`;

const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const table = (page: Page) =>
	page.getByRole("table", { name: /thresholds, with the alerts each raised/i });
/** The rows under the header, as many as are drawn. */
const rows = (page: Page) =>
	table(page).getByRole("rowgroup").last().getByRole("row");
const alertsTable = (page: Page) =>
	page.getByRole("table", { name: "Alerts this threshold raised" });
/** A day the API stamped by its own clock, which names its year once that is past the frozen clock's. */
const stampedDay = String.raw`[A-Z][a-z]{2} \d{1,2}(?:, \d{4})?`;

const LATENCY =
	"Threshold on main, ubuntu-latest, Latency, value, every variant";
const ALERTING = "blake3 input_bytes=65536 simd=avx2 threads=1 Latency";
const DISMISSED = "sha256 input_bytes=65536 simd=avx2 threads=4 Latency";

const headers = (token = seed.member.token) => ({
	Authorization: `Bearer ${token}`,
});

/** The seed's one threshold, on Latency on `main` and `ubuntu-latest`. */
const latencyThreshold = async (request: APIRequestContext) => {
	const response = await request.get(
		`${seed.api_url}/v0/console/projects/${hashbrown.slug}/thresholds`,
		{ headers: headers() },
	);
	const { thresholds: [threshold] = [] } =
		(await response.json()) as JsonConsoleThresholds;
	if (!threshold) {
		throw new Error("The seed has no threshold");
	}
	return threshold.uuid;
};

const send = async (
	request: APIRequestContext,
	method: "POST" | "PATCH",
	path: string,
	data: unknown,
) => {
	const response = await request.fetch(`${seed.api_url}${path}`, {
		method,
		headers: headers(),
		data,
	});
	if (!response.ok()) {
		throw new Error(
			`${method} ${path} answered ${response.status()}: ${await response.text()}`,
		);
	}
};

interface Declared {
	measure: string;
	model: Record<string, string | number>;
	parameters?: Record<string, number>[];
}

/** A run on `branch` that reports latency and throughput for two variants and declares `models`. */
const run = (
	request: APIRequestContext,
	project: string,
	branch: string,
	hoursAgo: number,
	models: Declared[],
) => {
	const start = Date.parse(seed.now) - hoursAgo * 60 * 60 * 1_000;
	const results = {
		blake3: [1, 2].map((n) => ({
			parameters: { n },
			measures: {
				latency: { value: 100 + n },
				throughput: { value: 10 + n },
			},
		})),
	};
	return send(request, "POST", `/v0/projects/${project}/reports`, {
		branch,
		hash: "1e2e000000000000000000000000000000000000",
		testbed: "ubuntu-latest",
		start_time: new Date(start).toISOString(),
		end_time: new Date(start + 60_000).toISOString(),
		results: [JSON.stringify(results)],
		settings: { adapter: "json" },
		thresholds: {
			models: models.map(({ measure, model, parameters }) => ({
				measure,
				metric: "value",
				model,
				...(parameters ? { parameters } : {}),
			})),
		},
	});
};

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// The seed's alerts were raised when the suite seeded the API, after the frozen
// clock's 2026-09-14, so every rolling window counts them and a range before
// that day counts none.

// Kills a model printed without the API's names or out of order, the raised and
// active counts read from each other's field, a count that ignores the window,
// and a window that never reaches the link.
test("the Latency threshold reads its model and counts its alerts in the window", async ({
	page,
}) => {
	await page.goto(thresholds(hashbrown.slug));

	await expect(heading(page)).toHaveText("1 threshold");
	await expect(
		page.getByRole("columnheader", { name: "Alerts in 4w" }),
	).toBeVisible();
	const row = rows(page).filter({
		has: page.getByRole("link", { name: LATENCY }),
	});
	await expect(row.getByRole("cell")).toHaveText([
		"main",
		"ubuntu-latest",
		"Latency",
		"value",
		"every variant",
		"t_test · upper_boundary 0.99 · max_sample_size 64 · min_sample_size 4",
		"1 active 2 raised",
	]);

	await page.getByRole("radio", { name: "Custom" }).click();
	await expect(page).toHaveURL(/[?&]start_time=\d+&end_time=\d+/);
	await expect(row.getByRole("cell").last()).toHaveText("1 active 0 raised");
	await page.reload();
	await expect(rows(page).first().getByRole("cell").last()).toHaveText(
		"1 active 0 raised",
	);

	await page.getByRole("radio", { name: "1w" }).click();
	await expect(page).toHaveURL(/[?&]window=1w(&|$)/);
	await expect(
		page.getByRole("columnheader", { name: "Alerts in 1w" }),
	).toBeVisible();
	await expect(rows(page).first().getByRole("cell").last()).toHaveText(
		"1 active 2 raised",
	);
});

// Kills an Archived toggle that never reaches the API or the link, archived
// thresholds listed as active, a row that does not say what archived it, an
// Archived branch menu without the archived branch, and a measure filter that
// is not sent or not named on its chip.
test("Archived lists a threshold whose branch was archived, and a measure filter narrows the list", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	await run(request, project.slug, "main", 3, [
		{ measure: "latency", model: { test: "t_test", upper_boundary: 0.99 } },
		{
			measure: "throughput",
			model: { test: "percentage", lower_boundary: 0.1 },
			parameters: [{ n: 1 }, { n: 2 }],
		},
	]);
	await run(request, project.slug, "retired", 2, [
		{ measure: "latency", model: { test: "z_score", upper_boundary: 0.95 } },
	]);
	await send(
		request,
		"PATCH",
		`/v0/projects/${project.slug}/branches/retired`,
		{ archived: true },
	);

	await page.goto(thresholds(project.slug));
	await expect(heading(page)).toHaveText("2 thresholds");
	const throughput = rows(page).filter({
		has: page.getByRole("link", {
			name: "Threshold on main, ubuntu-latest, Throughput, value, filtered to 2 parameter sets",
		}),
	});
	await expect(throughput.getByRole("cell").nth(4)).toHaveText("n=1 or n=2");
	await expect(throughput.getByRole("cell").nth(5)).toHaveText(
		"percentage · lower_boundary 0.1",
	);

	await page.getByRole("radio", { name: "Archived" }).click();
	await expect(page).toHaveURL(/[?&]status=archived(&|$)/);
	await expect(heading(page)).toHaveText("1 archived threshold");
	await expect(rows(page)).toHaveCount(1);
	await expect(rows(page).first().getByRole("cell").first()).toHaveText(
		new RegExp(`^retired · archived ${stampedDay}$`),
	);
	await page.getByRole("button", { name: "branch any" }).click();
	await expect(
		page.getByRole("menuitemradio", { name: "retired" }),
	).toBeVisible();
	await expect(page.getByRole("menuitemradio", { name: "main" })).toBeVisible();
	await page.keyboard.press("Escape");

	await page.getByRole("radio", { name: "Active" }).click();
	await page.getByRole("button", { name: "measure any" }).click();
	await page.getByRole("menuitemradio", { name: "Throughput" }).click();
	await expect(heading(page)).toHaveText("1 threshold");
	await expect(
		page.getByRole("button", { name: "measure Throughput" }),
	).toBeVisible();
	await page.reload();
	await expect(
		page.getByRole("button", { name: "measure Throughput" }),
	).toBeVisible();
	await expect(rows(page)).toHaveCount(1);
});

// Kills a threshold page that reads another threshold or none, a model field
// dropped or renamed, alerts asked for under the API's default status rather
// than All, a status toggle that never reaches the API, and a row that does not
// name the report it came from.
test("a row opens the threshold, its model, and the alerts it raised", async ({
	page,
	request,
}) => {
	const uuid = await latencyThreshold(request);
	await page.goto(thresholds(hashbrown.slug));
	await page.getByRole("link", { name: LATENCY }).click();

	await expect(page).toHaveURL(thresholdPath(hashbrown.slug, uuid));
	await expect(heading(page)).toHaveText(
		"main · ubuntu-latest · Latency · value",
	);
	const model = page.getByRole("region", { name: "Model", exact: true });
	await expect(model).toContainText("testt_test");
	await expect(model).toContainText("upper_boundary0.99");
	await expect(model).toContainText("lower_boundarynone");
	await expect(model).toContainText("max_sample_size64");
	await expect(model).toContainText("min_sample_size4");
	const history = page.getByRole("region", { name: "Model history" });
	await expect(history.getByRole("listitem")).toHaveCount(1);
	await expect(history.getByRole("listitem")).toContainText(
		"current t_test · upper_boundary 0.99 · max_sample_size 64 · min_sample_size 4",
	);

	await expect(
		page.getByText("1 active · 1 dismissed", { exact: true }),
	).toBeVisible();
	const expand = (name: string) =>
		alertsTable(page).getByRole("button", { name: `Expand ${name}` });
	await expect(expand(ALERTING)).toBeVisible();
	await expect(expand(DISMISSED)).toBeVisible();
	const dismissedRow = alertsTable(page)
		.getByRole("row")
		.filter({
			has: page.getByRole("button", { name: `Expand ${DISMISSED}` }),
		});
	await expect(dismissedRow).toContainText(/Dismissed [A-Z][a-z]{2} \d+/);
	await expect(
		dismissedRow.getByRole("button", { name: /^Reactivate the alert on / }),
	).toBeVisible();
	await expect(
		dismissedRow.getByRole("link", { name: /, open the report$/ }),
	).toHaveAttribute(
		"href",
		new RegExp(`${nextPath(hashbrown.slug, "reports")}/[0-9a-f-]{36}$`),
	);

	await page.getByRole("radio", { name: "Active" }).click();
	await expect(page).toHaveURL(/[?&]status=active(&|$)/);
	await expect(expand(DISMISSED)).toBeHidden();
	await expect(expand(ALERTING)).toBeVisible();

	await expand(ALERTING).click();
	await expect(
		page
			.getByRole("region", { name: `${ALERTING}, full plot` })
			.getByRole("button", { name: /: hide its line$/ }),
	).toBeVisible();
});

// Kills a model history that keeps only the current model, a replaced model
// drawn as current, and a snippet that does not declare the model as it is.
test("a threshold whose model changed lists both, and its snippet declares the current one", async ({
	page,
	request,
	context,
}) => {
	await context.grantPermissions(["clipboard-read", "clipboard-write"]);
	const project = await createProject(request);
	await run(request, project.slug, "main", 3, [
		{
			measure: "latency",
			model: { test: "t_test", upper_boundary: 0.99, max_sample_size: 30 },
		},
	]);
	await run(request, project.slug, "main", 2, [
		{
			measure: "latency",
			model: {
				test: "z_score",
				lower_boundary: 0.9,
				upper_boundary: 0.95,
				window: 7_776_000,
			},
		},
	]);

	await page.goto(thresholds(project.slug));
	await page
		.getByRole("link", {
			name: "Threshold on main, ubuntu-latest, Latency, value, every variant",
		})
		.click();
	const history = page.getByRole("region", { name: "Model history" });
	await expect(history.getByRole("listitem")).toHaveText([
		/^current z_score · upper_boundary 0\.95 · lower_boundary 0\.9 · window 90 days since /,
		new RegExp(
			String.raw`^replaced t_test · upper_boundary 0\.99 · max_sample_size 30 ${stampedDay} to `,
		),
	]);
	await expect(
		page.getByRole("region", { name: "Model", exact: true }),
	).toContainText("window90 days");

	const code = page.getByRole("group", {
		name: "The run that declares this threshold",
	});
	for (const flag of [
		`--project ${project.slug}`,
		"--branch main",
		"--testbed ubuntu-latest",
		"--threshold-measure latency",
		"--threshold-metric value",
		"--threshold-test z_score",
		"--threshold-window 7776000",
		"--threshold-lower-boundary 0.9",
		"--threshold-upper-boundary 0.95",
	]) {
		await expect(code).toContainText(flag);
	}
	await code
		.getByRole("button", { name: "Copy the bencher run command" })
		.click();
	await expect(
		code.getByRole("button", { name: "Copy the bencher run command" }),
	).toHaveText("Copied");
	const copied = await page.evaluate(() => navigator.clipboard.readText());
	expect(copied).toContain("--threshold-test z_score \\\n");
	expect(copied).toMatch(
		/^export BENCHER_API_KEY=bencher_run_\.\.\.\nbencher run \\\n/,
	);
});

// Kills an Explore link built from anything but the threshold's own branch,
// testbed, measure, and metric and the lines it alerted on.
test("Open in Explore names the threshold's lines that alerted", async ({
	page,
	request,
}) => {
	const uuid = await latencyThreshold(request);
	await page.goto(thresholdPath(hashbrown.slug, uuid));
	const explore = page.getByRole("link", {
		name: "Open in Explore: the lines this threshold alerted on",
	});
	await expect(explore).toHaveAttribute(
		"href",
		new RegExp(`^${nextPath(hashbrown.slug, "explore")}\\?`),
	);
	const href = new URL(
		(await explore.getAttribute("href")) ?? "",
		seed.console_url,
	);
	const {
		thresholds: [row] = [],
		branches,
		testbeds,
		measures,
	} = (await (
		await request.get(
			`${seed.api_url}/v0/console/projects/${hashbrown.slug}/thresholds`,
			{ headers: headers() },
		)
	).json()) as JsonConsoleThresholds;
	expect(href.searchParams.get("branches")).toBe(
		branches[row?.branch ?? -1]?.uuid,
	);
	expect(href.searchParams.get("testbeds")).toBe(
		testbeds[row?.testbed ?? -1]?.uuid,
	);
	expect(href.searchParams.get("measures")).toBe(
		measures[row?.measure ?? -1]?.uuid,
	);
	expect(href.searchParams.getAll("metrics")).toEqual(["value"]);
	expect(href.searchParams.get("benchmarks")?.split(",")).toHaveLength(2);
	expect(href.searchParams.get("only")?.split(",")).toHaveLength(2);
});

// Kills a malformed link sent to the API, which refuses it, and an unknown
// threshold shown as a failure to retry.
test("a malformed or unknown threshold is not found", async ({ page }) => {
	const requests: string[] = [];
	page.on("request", (request) => {
		if (/\/v0\/console\/projects\/[^/]+\/thresholds\//.test(request.url())) {
			requests.push(request.url());
		}
	});
	await page.goto(thresholdPath(hashbrown.slug, "latency"));
	await expect(heading(page)).toHaveText("Threshold not found");
	await settle(page);
	expect(requests).toEqual([]);

	await page.goto(
		thresholdPath(hashbrown.slug, "00000000-0000-4000-8000-000000000000"),
	);
	await expect(heading(page)).toHaveText("Threshold not found");
	await expect(
		page
			.getByRole("main")
			.getByRole("link", { name: "Thresholds", exact: true }),
	).toHaveAttribute("href", thresholds(hashbrown.slug));
});

// Kills a contrast or structure failure in either theme.
test("Thresholds and a threshold pass axe in both themes", async ({
	page,
	request,
}) => {
	await axeInBothThemes(page, thresholds(hashbrown.slug), async () => {
		await expect(page.getByRole("link", { name: LATENCY })).toBeVisible();
	});
	const uuid = await latencyThreshold(request);
	await axeInBothThemes(page, thresholdPath(hashbrown.slug, uuid), async () => {
		// On a phone the alerts sit below the cards, and only rows on screen are drawn.
		await alertsTable(page).scrollIntoViewIfNeeded();
		await expect(
			alertsTable(page).getByRole("button", { name: `Expand ${ALERTING}` }),
		).toBeVisible();
		await settle(page);
	});
});

// Kills a threshold page that scrolls sideways between tablet and desktop
// widths, and an alert row's controls drawn past the edge of the screen or of
// the list that holds them.
for (const width of [768, 820, 900, 980, 1024]) {
	test(`at ${width} px the threshold page scrolls nothing sideways and keeps every control in view`, async ({
		page,
		request,
	}) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(
			thresholdPath(
				hashbrown.slug,
				await latencyThreshold(request),
				"?status=all",
			),
		);
		await alertsTable(page).scrollIntoViewIfNeeded();
		await expect(
			alertsTable(page).getByRole("button", {
				name: `Reactivate the alert on ${DISMISSED}`,
			}),
		).toBeVisible();
		await settle(page);
		const measured = await page.evaluate(() => {
			const list = document
				.querySelector('table[aria-label="Alerts this threshold raised"]')
				?.parentElement?.getBoundingClientRect();
			const right = Math.min(window.innerWidth, list?.right ?? 0);
			const controls = [
				...document.querySelectorAll(
					'table[aria-label="Alerts this threshold raised"] :is(button, a)',
				),
			];
			return {
				page: document.documentElement.scrollWidth,
				controls: controls.length,
				out: controls
					.filter((control) => {
						const box = control.getBoundingClientRect();
						return (
							box.right > right + 0.5 || box.left < (list?.left ?? 0) - 0.5
						);
					})
					.map((control) => control.getAttribute("aria-label")),
			};
		});
		expect(measured.page).toBeLessThanOrEqual(width);
		expect(measured.controls).toBeGreaterThan(0);
		expect(measured.out).toEqual([]);
	});
}

test.describe("a list longer than a batch", () => {
	test.use({ viewport: { width: 1280, height: 400 } });

	// Kills a next batch the API read after a dismissal it took while that
	// batch was on its way, which starts an alert late and is never asked for
	// again from the recounted offset, so that alert is never drawn.
	test("a dismissal the API takes while the next batch is on its way skips and repeats no alert", async ({
		page,
		request,
	}) => {
		const project = await createManyAlerts(request, 60);
		const response = await request.get(
			`${seed.api_url}/v0/console/projects/${project.slug}/thresholds`,
			{ headers: headers() },
		);
		const { thresholds: [threshold] = [] } =
			(await response.json()) as JsonConsoleThresholds;
		const label = "Alerts this threshold raised";
		const offsets = await dismissWhileNextBatchIsOut(
			page,
			thresholdPath(project.slug, threshold?.uuid ?? "", "?status=active"),
			label,
		);
		expect(
			await alertsTopToBottom(page, label, "bench-59 n=0 Latency"),
		).toEqual(manyAlertNames(60));
		const [, size = 0] = offsets;
		expect(offsets.slice(0, 3)).toEqual([0, size, size - 1]);
	});

	// Kills later batches read again from the offsets they were first asked
	// from, on returning to the window or by a reread after a change the API
	// made to no alert, rather than only the first batch, then each later one
	// from its recounted offset.
	test("after dismissals, returning to the window reads only the first batch again, and a reread recounts each later batch", async ({
		page,
		request,
	}) => {
		const project = await createManyAlerts(request, 60);
		const response = await request.get(
			`${seed.api_url}/v0/console/projects/${project.slug}/thresholds`,
			{ headers: headers() },
		);
		const { thresholds: [threshold] = [] } =
			(await response.json()) as JsonConsoleThresholds;
		const label = "Alerts this threshold raised";
		const { size, focus, reread } = await refocusThenReread(
			page,
			request,
			project.slug,
			thresholdPath(project.slug, threshold?.uuid ?? "", "?status=active"),
			label,
		);
		expect(focus).toEqual([0]);
		expect(
			await alertsTopToBottom(page, label, "bench-59 n=0 Latency"),
		).toEqual(
			manyAlertNames(60).filter((name) => name !== "bench-03 n=0 Latency"),
		);
		expect(reread()).toEqual([0, size, 2 * size, 3 * size]);
	});
});

test.describe("on a phone", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a table that keeps its columns on a phone, a window or filters left
	// in a row of controls, a Filters sheet that does not open, a phone bar
	// without Clear or one that drops the focus, targets under 44 px, and a page
	// wider than the phone.
	test("rows fold, the window is one chip, the filters open in a sheet, and the threshold fits", async ({
		page,
		request,
	}) => {
		await page.goto(thresholds(hashbrown.slug));
		await expect(heading(page)).toHaveText("1 threshold");
		await expect(
			page.getByRole("columnheader", { name: "Model" }),
		).toBeHidden();
		await expect(
			page.getByRole("radiogroup", { name: "Alerts in" }),
		).toBeHidden();
		const row = await rows(page).first().boundingBox();
		expect(row?.height).toBeGreaterThanOrEqual(44);

		const windowChip = page.getByRole("button", {
			name: "Alerts in 4w, 4 weeks",
		});
		await windowChip.click();
		await page.getByRole("menuitemradio", { name: "1 week" }).click();
		await expect(page).toHaveURL(/[?&]window=1w(&|$)/);

		const filters = page.getByRole("button", { name: "Filters, none applied" });
		await filters.click();
		const sheet = page.getByRole("dialog", { name: "Filters" });
		await expect(
			sheet.getByRole("button", { name: "testbed any" }),
		).toBeVisible();
		await sheet.getByRole("button", { name: "branch any" }).click();
		await sheet.getByRole("menuitemradio", { name: "main" }).click();
		await sheet.getByRole("button", { name: "Done" }).click();
		await expect(page).toHaveURL(/[?&]branch=/);
		const clear = page.getByRole("button", { name: "Clear filters" });
		const size = await clear.boundingBox();
		expect(
			Math.min(size?.width ?? 0, size?.height ?? 0),
		).toBeGreaterThanOrEqual(44);
		await clear.click();
		await expect(filters).toBeFocused();
		await expect(page).not.toHaveURL(/[?&]branch=/);
		for (const target of [
			page.getByRole("button", { name: "Alerts in 1w, 1 week" }),
			filters,
			page.getByRole("radio", { name: "Archived" }).locator(".."),
		]) {
			expect((await target.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		}

		await page.goto(
			thresholdPath(hashbrown.slug, await latencyThreshold(request)),
		);
		// Only the rows on screen are drawn, and the alerts sit below the cards.
		await alertsTable(page).scrollIntoViewIfNeeded();
		await expect(
			alertsTable(page).getByRole("button", { name: `Expand ${ALERTING}` }),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: "Window 4w, 4 weeks" }),
		).toBeVisible();
		const width = await page.evaluate(
			() => document.documentElement.scrollWidth,
		);
		expect(width).toBeLessThanOrEqual(390);
	});
});
