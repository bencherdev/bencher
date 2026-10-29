import type { APIRequestContext, Page } from "@playwright/test";
import { postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, consolePath, seed, signedIn, test } from "./fixtures";

// The runner project's jobs are written as the runner protocol leaves them
// (no API takes a job through its states without a runner); everything else
// here, including the held run's new reports and the handover, goes through
// the API.

const { runner } = seed.projects;
/** A seeded project this repeat of the test alone changes. */
const own = (projects: { slug: string }[]) => {
	const project = projects[test.info().repeatEachIndex];
	if (!project) {
		throw new Error("The seed has fewer projects than the suite has repeats");
	}
	return project;
};

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

const latest = (page: Page) =>
	page.getByRole("region", { name: "Latest runs" });

/** Counts the API requests whose path is `path`, from now on. */
const counting = (page: Page, path: string) => {
	let asked = 0;
	page.on("request", (sent) => {
		if (new URL(sent.url()).pathname === path) {
			asked += 1;
		}
	});
	return () => asked;
};

// Kills a run's status read from anything but its jobs, counts drawn for a run
// nothing has landed for, and a landed report's counts left out of a running run.
test("the Runs list shows each run's jobs where its time lands", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "runs"));
	const table = page.getByRole("table", { name: "Runs, newest first" });
	const row = (hash: string) =>
		table.getByRole("row").filter({ hasText: hash.slice(0, 7) });
	const tail = async (hash: string) =>
		(await row(hash).getByRole("cell").allInnerTexts())
			.slice(6, 9)
			.map((text) => text.replace(/\s+/g, " ").trim());
	await expect(row(seed.jobs_hash)).toBeVisible();
	expect(await tail(seed.jobs_hash)).toEqual([
		"2",
		"1 active 1 total",
		"Running 48s",
	]);
	expect(await tail("9a7e3f2")).toEqual(["-", "-", "Queued"]);
	expect(await tail("e1f0b2d")).toEqual(["-", "-", "Failed 42s"]);
	expect(await tail("c4ce1ed")).toEqual(["-", "-", "Canceled"]);
	expect(await tail("5f3c9a0")).toEqual(["2", "0", "2m 00s"]);
});

// Kills a report's status read from anything but its job, a start shown for a
// job no runner took, a run heading without its status, and a page that stops
// asking while a job runs.
test("a run page shows each report's job and asks again while one runs", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "runs"));
	const asked = counting(page, `/v0/console/projects/${runner.slug}/runs`);
	await page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("link", { name: seed.jobs_hash, exact: true })
		.click();
	const reports = page.getByRole("table", {
		name: "Reports in this run, by start",
	});
	await expect(reports.getByText("Queued")).toBeVisible();
	const cells = async (row: number) =>
		(await reports.getByRole("row").nth(row).getByRole("cell").allInnerTexts())
			.slice(3, 7)
			.map((text) => text.replace(/\s+/g, " ").trim());
	expect(await cells(1)).toEqual([
		"23:50:00",
		"2",
		"1 active 1 total",
		"2m 00s",
	]);
	expect(await cells(2)).toEqual(["23:51:00", "-", "-", "Running 48s"]);
	expect(await cells(3)).toEqual(["", "-", "-", "Queued"]);
	await expect(page.locator(".rp-titlerow")).toContainText("Running 48s");

	const runPath = new URL(page.url()).pathname.split("/").at(-1);
	const runAsked = counting(
		page,
		`/v0/console/projects/${runner.slug}/runs/${runPath}`,
	);
	await expect.poll(runAsked, { timeout: 12_000 }).toBeGreaterThanOrEqual(1);
	expect(asked()).toBeLessThanOrEqual(1);
});

// Kills an opened run whose rows are drawn again on each poll while its job
// runs, which takes the focus from them, and one that stops asking.
test("an opened running run keeps its rows and the focus across polls", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "runs"));
	const table = page.getByRole("table", { name: "Runs, newest first" });
	const run = (
		(await table
			.getByRole("link", { name: seed.jobs_hash, exact: true })
			.getAttribute("href")) ?? ""
	)
		.split("/")
		.at(-1);
	const listAsked = counting(page, `/v0/console/projects/${runner.slug}/runs`);
	const runAsked = counting(
		page,
		`/v0/console/projects/${runner.slug}/runs/${run}`,
	);
	await table
		.getByRole("button", { name: new RegExp(`^main, run ${seed.jobs_hash}, `) })
		.click();
	const report = table.getByRole("link", {
		name: /^ubuntu-latest, report 1, /,
	});
	await report.focus();
	await report.evaluate((element) => {
		(element as HTMLElement).dataset.kept = "";
	});
	const [list, opened] = [listAsked(), runAsked()];
	await expect
		.poll(() => Math.min(listAsked() - list, runAsked() - opened), {
			timeout: 12_000,
		})
		.toBeGreaterThanOrEqual(1);
	expect(
		await page.evaluate(
			() => (document.activeElement as HTMLElement | null)?.dataset.kept,
		),
	).toBe("");
});

// Kills a run page that asks again with nothing unfinished.
test("a finished run's page does not ask again", async ({ page }) => {
	await page.goto(consolePath(runner.slug, "runs"));
	const link = page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("link", { name: "5f3c9a0", exact: true });
	const href = (await link.getAttribute("href")) ?? "";
	const asked = counting(
		page,
		`/v0/console/projects/${runner.slug}/runs/${href.split("/").at(-1)}`,
	);
	await link.click();
	await expect(page.locator(".rp-titlerow")).toContainText("5f3c9a0");
	await page.waitForTimeout(7_000);
	expect(asked()).toBeLessThanOrEqual(1);
});

/** The query keys the browser's store holds. */
const storedKeys = (page: Page) =>
	page.evaluate(
		() =>
			new Promise<string[]>((resolve, reject) => {
				const open = indexedDB.open("bencher-console");
				open.onerror = () => reject(open.error);
				open.onsuccess = () => {
					const db = open.result;
					const names = [...db.objectStoreNames];
					const all: string[] = [];
					const tx = db.transaction(names, "readonly");
					for (const name of names) {
						const read = tx.objectStore(name).getAll();
						read.onsuccess = () => {
							for (const entry of read.result) {
								for (const query of entry?.state?.queries ?? []) {
									all.push(JSON.stringify(query.queryKey));
								}
							}
						};
					}
					tx.oncomplete = () => resolve(all);
				};
			}),
	);

// Kills a poll's answer, which shows a job still running, written to the
// browser's store, where a return would paint its stale status.
test("a page that shows a running job never stores what it hears", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "runs"));
	await expect(page.getByText("Running 48s")).toBeVisible();
	await expect
		.poll(async () => (await storedKeys(page)).length, { timeout: 10_000 })
		.toBeGreaterThan(0);
	await page.waitForTimeout(6_000);
	const keys = await storedKeys(page);
	expect(keys.some((key) => key.includes('"runs"'))).toBe(false);
	expect(keys.some((key) => key.includes('"project"'))).toBe(true);
});

// Kills a runner report's page without its status or with a "No lines" card
// while it runs, and a spec missing from its sub line or what it ran on.
test("a running report says so in its heading and names its spec", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "runs"));
	await page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("link", { name: seed.jobs_hash, exact: true })
		.click();
	await page
		.getByRole("table", { name: "Reports in this run, by start" })
		.getByRole("link", { name: /report 2, .*Running 48s$/ })
		.click();
	await expect(page.locator(".rp-titlerow")).toContainText("Running 48s");
	await expect(page.locator(".rp-sub")).toContainText(
		/Standard x86 \(\w+, x86_64, \d+, 16 GiB\)/,
	);
	await expect(page.getByRole("heading", { name: "No lines" })).toHaveCount(0);
});

// Kills blank Explore's latest runs drawn without their statuses, and an
// opened run that offers a benchmark search for a report with nothing landed.
test("blank Explore's latest runs show their jobs, and an opened run's unfinished reports show theirs", async ({
	page,
}) => {
	await page.goto(consolePath(runner.slug, "explore"));
	const run = latest(page).getByRole("button", {
		name: new RegExp(`^main, run ${seed.jobs_hash}, .*Running 48s$`),
	});
	await expect(run).toBeVisible();
	await expect(latest(page)).toContainText("Queued");
	await expect(latest(page)).toContainText("Failed 42s");
	await expect(latest(page)).toContainText("Canceled");
	await run.click();
	const reports = page.getByRole("region", {
		name: `Reports in run ${seed.jobs_hash}`,
	});
	await expect(reports.getByRole("button", { name: /^1 main/ })).toBeVisible();
	await expect(
		reports.getByRole("button", { name: /^(2|3) main/ }),
	).toHaveCount(0);
	await expect(reports).toContainText("Running 48s");
	await expect(reports).toContainText("Queued");
});

const heldReport = (request: APIRequestContext, slug: string) =>
	postReport(request, slug, {
		hash: `${seed.held_hash}${"0".repeat(33)}`,
		start: new Date(Date.parse(seed.now) - 60_000).toISOString(),
		results: {
			blake3: [
				{
					parameters: { input_bytes: 65536 },
					measures: { latency: { value: 20 } },
				},
			],
		},
	});

// Kills an alerting line that lands in a table already showing lines and pushes
// its rows about, and a Show that leaves it held.
test("a running run holds alerting lines that land behind Show", async ({
	page,
	request,
}) => {
	const held = own(seed.projects.held.slice(0, -1));
	await page.goto(consolePath(held.slug, "runs"));
	await page
		.getByRole("table", { name: "Runs, newest first" })
		.getByRole("link", { name: seed.held_hash, exact: true })
		.click();
	const alerts = page.getByRole("table", { name: /^Alerts in run / });
	await expect(alerts).toBeVisible();
	const lines = alerts.locator("tbody tr.lr");
	const before = await lines.count();
	expect(before).toBeGreaterThan(0);
	await heldReport(request, held.slug);
	const status = page.getByRole("status").filter({ hasText: "new line" });
	await expect(status).toContainText("1 new line", { timeout: 15_000 });
	expect(await lines.count()).toBe(before);
	await status.getByRole("button", { name: "Show" }).click();
	await expect(lines).toHaveCount(before + 1);
	await expect(status).toHaveCount(0);
});

const unarchive = (request: APIRequestContext, slug: string) =>
	request.fetch(`${seed.api_url}/v0/projects/${slug}/branches/main`, {
		method: "PATCH",
		headers: { Authorization: `Bearer ${seed.member.token}` },
		data: { archived: false },
	});

// Kills a setup card that does not give way to a job's report, or a Runs
// list that shows that run without its status.
test("the setup card gives way to a queued job's run, which shows its status", async ({
	page,
	request,
}) => {
	const handover = own(seed.projects.handover);
	await page.goto(consolePath(handover.slug));
	const card = page.getByRole("region", { name: "Send the first report" });
	await expect(card).toBeVisible();
	expect((await unarchive(request, handover.slug)).ok()).toBe(true);
	await expect(card).toHaveCount(0, { timeout: 15_000 });
	await expect(
		page
			.getByRole("table", { name: "Runs, newest first" })
			.getByRole("row")
			.filter({ hasText: "9a7e3f2" }),
	).toContainText("Queued");
});

test.describe("with motion reduced, so the running pulse rests", () => {
	test.use({ reducedMotion: "reduce" });

	const runs = (page: Page) =>
		page.getByRole("table", { name: "Runs, newest first" });
	/** Opens the running run from the Runs list, the pointer then off its rows. */
	const openRun = async (page: Page) => {
		await runs(page)
			.getByRole("link", { name: seed.jobs_hash, exact: true })
			.click();
		await expect(
			page.getByRole("table", { name: "Reports in this run, by start" }),
		).toBeVisible();
		await page.mouse.move(0, 0);
	};

	// Kills a Runs list whose pills, dashes, or rows fail axe in either theme.
	test("the Runs list of pending runs passes axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(page, consolePath(runner.slug, "runs"), async () => {
			await expect(runs(page).getByText("Queued").first()).toBeVisible();
		});
	});

	// Kills a running run's page that fails axe in either theme.
	test("a running run's page passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(
			page,
			consolePath(runner.slug, "runs"),
			() => expect(page.locator(".rp-titlerow")).toContainText("Running"),
			() => openRun(page),
		);
	});

	// Kills a running report's page, its pill or its spec, that fails axe in
	// either theme.
	test("a running report's page passes axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			consolePath(runner.slug, "runs"),
			() => expect(page.locator(".rp-titlerow")).toContainText("Running"),
			async () => {
				await openRun(page);
				await page
					.getByRole("table", { name: "Reports in this run, by start" })
					.getByRole("link", { name: /report 2, / })
					.click();
				await page.waitForURL(/\/reports\//);
				await page.mouse.move(0, 0);
			},
		);
	});

	// Kills blank Explore's opened running run that fails axe in either theme.
	test("blank Explore with a running run opened passes axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			consolePath(runner.slug, "explore"),
			() =>
				expect(
					page.getByRole("region", {
						name: `Reports in run ${seed.jobs_hash}`,
					}),
				).toContainText("Queued"),
			async () => {
				await latest(page)
					.getByRole("button", { name: new RegExp(`run ${seed.jobs_hash}`) })
					.click();
				await page.mouse.move(0, 0);
			},
		);
	});

	// Kills held lines, their count or their Show, that fail axe in either theme.
	test("a run's held lines pass axe in both themes", async ({
		page,
		request,
	}) => {
		// The project no other test changes, as each scan lands a line of its own.
		const held = seed.projects.held.at(-1);
		if (!held) {
			throw new Error("The seed has no held project");
		}
		await axeInBothThemes(
			page,
			consolePath(held.slug, "runs"),
			() =>
				expect(
					page.getByRole("status").filter({ hasText: "new line" }),
				).toBeVisible({ timeout: 15_000 }),
			async () => {
				await runs(page)
					.getByRole("link", { name: seed.held_hash, exact: true })
					.click();
				await expect(
					page.getByRole("table", { name: /^Alerts in run / }),
				).toBeVisible();
				await page.mouse.move(0, 0);
				await heldReport(request, held.slug);
			},
		);
	});
});
