import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import { createProject, postReport } from "./api";
import { expect, consolePath, seed, signedIn, tabRow, test } from "./fixtures";

test.use({ storageState: signedIn(seed.member) });

const explore = (page: Page) =>
	tabRow(page).getByRole("link", { name: "Explore" });

// Kills a metric's landing that opens Explore on another metric of its
// measure, on the branch's current head, without its report marked, or with
// the reader left on the landing.
test("a metric's link opens Explore on exactly its line at its report", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	const report = await postReport(request, project.slug, {
		start: new Date(Date.parse(seed.now) - 60 * 60 * 1_000).toISOString(),
		results: {
			blake3: [
				{
					parameters: { threads: 1 },
					measures: { latency: { value: 100, p99: 140 } },
				},
			],
		},
	});
	const metric = report.results?.[0]?.[0]?.measures[0]?.metrics.find(
		({ name }) => name === "p99",
	);
	if (!metric) {
		throw new Error("The report has no p99");
	}

	await page.goto(consolePath(project.slug, `metrics/${metric.uuid}`));

	await expect(page).toHaveURL(
		(url) => url.pathname === consolePath(project.slug, "explore"),
	);
	const params = new URL(page.url()).searchParams;
	expect(params.getAll("metrics")).toEqual(["p99"]);
	expect(params.get("heads")).toBe(report.branch.head.uuid);
	expect(params.get("report")).toBe(report.uuid);
	expect(params.get("only")?.split(",")).toHaveLength(1);
	await expect(page.getByRole("figure")).toHaveAccessibleName("1 line");
	await expect(explore(page)).toHaveAttribute("aria-current", "page");
});

// Kills a landing that stays in the history, so Back opens it again and it
// sends the reader forward to Explore once more.
test("Back from a metric's Explore returns to the page before it", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	await page.goto(consolePath(project.slug, "runs"));
	await page.goto(consolePath(project.slug, `metrics/${randomUUID()}`));
	await expect(page).toHaveURL(consolePath(project.slug, "explore"));

	await page.goBack();

	await expect(page).toHaveURL(consolePath(project.slug, "runs"));
});

// Kills a metric that cannot be read leaving the reader on the landing, or
// opening Explore with something chosen.
test("a metric that cannot be read opens Explore with nothing chosen", async ({
	page,
	request,
}) => {
	const project = await createProject(request);

	await page.goto(consolePath(project.slug, `metrics/${randomUUID()}`));

	await expect(page).toHaveURL(consolePath(project.slug, "explore"));
});
