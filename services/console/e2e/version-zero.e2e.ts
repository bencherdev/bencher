import { execFile } from "node:child_process";
import { mkdtemp, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { promisify } from "node:util";
import type { APIRequestContext, Locator, Page } from "@playwright/test";
import type { JsonPlot, JsonThreshold } from "../src/types/bencher";
import { createProject } from "./api";
import { dimensions, explore } from "./explore";
import { approach, expect, nextPath, seed, signedIn, test } from "./fixtures";

const { version_zero } = seed.projects;

const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const box = (page: Page, name: string) => page.getByRole("group", { name });
const lines = (page: Page) =>
	page.getByRole("table", { name: /^Lines in report / });
const headers = { Authorization: `Bearer ${seed.member.token}` };
const read = async <T>(request: APIRequestContext, path: string) =>
	(await (
		await request.get(`${seed.api_url}${path}`, { headers })
	).json()) as T;

const LOCKED = "Filtering by parameters needs a version 1 project.";

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

/** The newest run's report, reached from Runs as a reader would. */
const newestReport = async (page: Page) => {
	await page.goto(nextPath(version_zero.slug, "runs"));
	await expect(heading(page)).toHaveText("6 runs");
	await page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("rowgroup")
		.last()
		.getByRole("row")
		.filter({ hasText: seed.version_zero_hash })
		.getByRole("link")
		.first()
		.click();
	await expect(heading(page)).toHaveText(`main · ${seed.version_zero_hash}`);
	await page
		.getByRole("table", { name: "Reports in this run, by start" })
		.getByRole("link")
		.first()
		.click();
	await expect(heading(page)).toHaveText("main · ubuntu-latest");
};

// Kills any page of a version 0 project that still sends it back to the
// classic console, or fails to draw its data.
test("every page draws a version 0 project", async ({ page, request }) => {
	await newestReport(page);
	await expect(lines(page)).toContainText("blake3 Latency lower_value");

	const id = await dimensions(request, version_zero.slug);
	await page.goto(
		explore(version_zero.slug, {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("blake3")],
			measures: [id.measure("Latency")],
		}),
	);
	// Every metric: the value and both of its bounds.
	await expect(page.getByRole("figure")).toHaveAccessibleName("3 lines");

	await page.goto(nextPath(version_zero.slug, "plots"));
	await expect(heading(page)).toHaveText("1 pinned plot");

	await page.goto(nextPath(version_zero.slug, "alerts"));
	await expect(heading(page)).toHaveText("1 active");
	await page.getByRole("link", { name: /^blake3 .*, open the alert$/ }).click();
	await expect(heading(page)).toContainText("blake3");

	await page.goto(nextPath(version_zero.slug, "thresholds"));
	await expect(heading(page)).toHaveText("1 threshold");
	await page
		.getByRole("link", {
			name: /threshold on main, ubuntu-latest, Latency, value, every variant$/,
		})
		.click();
	await expect(heading(page)).toHaveText(
		"main · ubuntu-latest · Latency · value",
	);

	for (const [path, name] of [
		["branches", "Branches"],
		["branches/main", "main"],
		["testbeds/ubuntu-latest", "ubuntu-latest"],
		["benchmarks/blake3", "blake3"],
		["measures/latency", "Latency"],
		["settings", "General"],
	] as const) {
		await page.goto(nextPath(version_zero.slug, path));
		await expect(heading(page)).toHaveText(name);
	}
});

// Kills a bound line offered a threshold it cannot have, and a sheet that
// teaches flags a version 0 project refuses or names `json` for another adapter.
test("a report's lines no threshold checks teach version 0, and a bound line has none to declare", async ({
	page,
}) => {
	await newestReport(page);
	const table = lines(page);
	await expect(table).toContainText("blake3 Latency upper_value");
	for (const metric of ["lower_value", "upper_value"]) {
		await expect(
			table.getByRole("button", {
				name: new RegExp(`^No threshold checks blake3 Latency ${metric}\\.`),
			}),
		).toHaveCount(0);
	}
	await table
		.getByRole("button", {
			name: "No threshold checks blake3 Throughput value. Show the run snippet that declares one.",
		})
		.click();
	const sheet = page.getByRole("dialog", {
		name: "No threshold checks this line",
	});
	await expect(sheet).toContainText("--adapter json");
	await expect(sheet).toContainText("--threshold-measure throughput");
	await expect(sheet).toContainText("--threshold-lower-boundary 0.99");
	await expect(sheet).not.toContainText("--threshold-metric");
	await expect(sheet).not.toContainText("--threshold-parameters");
	await expect(
		sheet.getByRole("region", { name: "Exactly this line" }),
	).toHaveCount(0);
});

// Kills a threshold page that teaches a version 0 threshold the version 1
// flags, or an adapter no single report on the page names.
test("Change this threshold teaches version 0 with no adapter", async ({
	page,
	request,
}) => {
	const [threshold] = await read<JsonThreshold[]>(
		request,
		`/v0/projects/${version_zero.slug}/thresholds`,
	);
	await page.goto(
		nextPath(version_zero.slug, `thresholds/${threshold?.uuid ?? ""}`),
	);
	const change = page.getByRole("region", { name: "Change this threshold" });
	await expect(change).toContainText("--threshold-measure latency");
	await expect(change).toContainText("--threshold-test percentage");
	await expect(change).toContainText("--threshold-upper-boundary 0.1");
	for (const flag of [
		"--adapter",
		"--threshold-metric",
		"--threshold-parameters",
	]) {
		await expect(change).not.toContainText(flag);
	}
});

// Kills a parameters box a version 0 project can add to, one locked without
// saying why, and a link from a report row that carries its line's empty
// parameter set into a removable row.
test("Explore's parameters box is locked, and a row's link carries no set", async ({
	page,
}) => {
	await newestReport(page);
	await approach(lines(page));
	await lines(page)
		.getByRole("link", { name: "Open in Explore: blake3 Latency value" })
		.click();
	await expect(page).toHaveURL(/\/explore\?/);
	expect(new URL(page.url()).searchParams.has("parameters")).toBe(false);
	const parameters = box(page, "Parameters");
	await expect(parameters.getByText("every variant")).toBeVisible();
	await expect(
		parameters.getByRole("button", { name: /^Remove set/ }),
	).toHaveCount(0);
	const add = parameters.getByRole("button", { name: "Add set" });
	await expect(add).toHaveAttribute("aria-disabled", "true");
	await add.hover();
	await expect(page.getByRole("tooltip")).toHaveText(LOCKED);
	await add.click({ force: true });
	await expect(page.getByRole("menu")).toHaveCount(0);
});

// Kills a pin the classic console made, with no metrics, that the new console
// cannot draw or open: it draws every metric, the value and both bounds.
test("a classic pin draws every metric and opens in Explore", async ({
	page,
	request,
}) => {
	const [pin] = await read<JsonPlot[]>(
		request,
		`/v0/projects/${version_zero.slug}/plots`,
	);
	const title = pin?.title ?? "";
	expect(pin?.metrics).toBeUndefined();
	await page.goto(nextPath(version_zero.slug, "plots"));
	await expect(
		page.getByRole("region", { name: title, exact: true }).getByRole("figure"),
	).toHaveAccessibleName("3 lines over 4w");
	await page.getByRole("link", { name: `Open ${title} in Explore` }).click();
	await expect(page).toHaveURL(/\/explore\?/);
	await expect(page.getByRole("figure")).toHaveAccessibleName("3 lines");
	await expect(heading(page)).toHaveText(title);
});

/** A code block's own text, without its Copy button's. */
const codeText = (code: Locator) =>
	code.evaluate((element) =>
		[...element.childNodes]
			.filter((node) => node.nodeType === Node.TEXT_NODE)
			.map((node) => node.textContent)
			.join(""),
	);

// Kills a setup card that teaches a version 0 project the version 1 sample or
// threshold flags, which it refuses, and any flag of the card the CLI or the
// API does not take: the card's own run lands its first report and threshold.
test("an empty version 0 project's setup card teaches a run that lands", async ({
	page,
	request,
}) => {
	const project = await createProject(request, { version: 0 });
	await page.goto(nextPath(project.slug, "runs"));
	await expect(heading(page)).toHaveText("Send the first report");
	await expect(
		page.getByText(`${project.name} · BMF v0 · no reports yet`),
	).toBeVisible();
	await page.getByRole("button", { name: "Create key" }).click();
	const key = await page
		.getByRole("status", { name: "Your new project key" })
		.locator("span")
		.first()
		.textContent();
	const blocks = page.locator("pre.code");
	const sample = await codeText(blocks.nth(1));
	const command = await codeText(blocks.nth(2));
	expect(sample).not.toContain("parameters");
	expect(command).not.toContain("--threshold-metric");

	const cli = process.env.BENCHER_E2E_CLI;
	if (!cli) {
		throw new Error("Run the end-to-end suite with `cargo test-console e2e`");
	}
	const [bencher, ...args] = command
		.replaceAll("\\\n", " ")
		.trim()
		.split(/\s+/);
	expect(bencher).toBe("bencher");
	const dir = await mkdtemp(join(tmpdir(), "bencher-v0-"));
	try {
		await writeFile(join(dir, "results.json"), sample);
		await promisify(execFile)(cli, args, {
			cwd: dir,
			env: { ...process.env, BENCHER_API_KEY: key ?? "" },
		});
	} finally {
		await rm(dir, { recursive: true, force: true });
	}

	await expect(heading(page)).toHaveText("1 run", { timeout: 15_000 });
	const thresholds = await read<JsonThreshold[]>(
		request,
		`/v0/projects/${project.slug}/thresholds`,
	);
	expect(thresholds.map(({ measure }) => measure.slug)).toEqual(["latency"]);
});
