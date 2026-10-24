import { createProject, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { dimensions } from "./explore";
import { expect, consolePath, seed, signedIn, tabRow, test } from "./fixtures";

const { empty, hashbrown } = seed.projects;
const DAY = 24 * 60 * 60 * 1000;

const card = (page: import("@playwright/test").Page) =>
	page.getByRole("region", { name: "Send the first report" });

test.describe("a project's Maintainer", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a project with no reports opened at its root on an empty Runs
	// list, or on "No runs" drawn before the card replaces it, a key shown
	// again or never, and a card that never notices the first report or does
	// not give way to the Runs list with that run first.
	test("an empty project's root opens on the setup card, a key shows once, and the first report gives way to Runs", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await page.addInitScript(() => {
			const holder = window as Window & { __noRuns?: boolean };
			new MutationObserver(() => {
				holder.__noRuns ||=
					[...document.querySelectorAll("h2")].some(
						(heading) => heading.textContent === "No runs",
					) ||
					[...document.querySelectorAll("h1")].some((heading) =>
						heading.textContent?.includes("0 runs"),
					);
			}).observe(document, { childList: true, subtree: true });
		});
		await page.goto(consolePath(project.slug));
		await expect(card(page)).toBeVisible();
		await expect(
			tabRow(page).getByRole("link", { name: "Runs" }),
		).toHaveAttribute("aria-current", "page");
		expect(
			await page.evaluate(
				() => (window as Window & { __noRuns?: boolean }).__noRuns,
			),
		).toBeFalsy();
		await expect(
			card(page).getByText(`--project ${project.slug}`),
		).toBeVisible();
		// The suite's API is not Bencher Cloud, as on a self-hosted server.
		await expect(card(page).getByText(`--host ${seed.api_url}`)).toBeVisible();

		await card(page).getByRole("button", { name: "Create key" }).click();
		const secret = card(page).getByRole("status", {
			name: "Your new project key",
		});
		const key = (await secret.innerText()).split("\n")[0]?.trim() ?? "";
		expect(key).toMatch(/^bencher_run_/);
		await expect(
			card(page).getByText(`export BENCHER_API_KEY=${key}`),
		).toBeVisible();
		await expect(
			card(page).getByRole("button", { name: "Create key" }),
		).toHaveCount(0);

		await page.reload();
		await expect(card(page)).toBeVisible();
		await expect(card(page).getByText(key)).toHaveCount(0);

		await postReport(request, project.slug, {
			start: "2026-09-10T00:00:00Z",
			results: {
				my_benchmark: [
					{ parameters: { size: 64 }, measures: { latency: { value: 88.2 } } },
				],
			},
		});
		await expect(card(page)).toHaveCount(0, { timeout: 15_000 });
		const rows = page
			.getByRole("table", { name: "Runs, newest first" })
			.getByRole("row")
			.filter({ has: page.getByRole("link") });
		await expect(rows).toHaveCount(1);
		await expect(rows.getByRole("link").first()).toHaveAttribute(
			"href",
			new RegExp(`^${consolePath(project.slug, "runs")}/[0-9a-f-]{36}$`),
		);
	});

	// Kills a card that keeps asking while its tab is hidden, and a poll that
	// rewrites the reader's whole stored cache each time it hears nothing new.
	test("the card asks only while its tab shows, and an unchanged answer writes nothing", async ({
		page,
		request,
	}) => {
		test.setTimeout(60_000);
		const project = await createProject(request);
		await page.addInitScript(() => {
			const holder = window as Window & { __puts?: number; __hidden?: boolean };
			holder.__puts = 0;
			const put = IDBObjectStore.prototype.put;
			IDBObjectStore.prototype.put = function (
				this: IDBObjectStore,
				...args: Parameters<IDBObjectStore["put"]>
			) {
				holder.__puts = (holder.__puts ?? 0) + 1;
				return put.apply(this, args);
			};
			Object.defineProperty(document, "visibilityState", {
				configurable: true,
				get: () => (holder.__hidden ? "hidden" : "visible"),
			});
		});
		let polls = 0;
		page.on("request", (sent) => {
			const url = new URL(sent.url());
			// Each poll asks for the Runs list's first batch.
			if (
				url.pathname === `/v0/console/projects/${project.slug}/runs` &&
				url.searchParams.get("page") === "1" &&
				url.searchParams.has("start_time")
			) {
				polls += 1;
			}
		});
		const puts = () =>
			page.evaluate(() => (window as Window & { __puts?: number }).__puts ?? 0);
		const hide = (hidden: boolean) =>
			page.evaluate((hidden) => {
				(window as Window & { __hidden?: boolean }).__hidden = hidden;
				document.dispatchEvent(new Event("visibilitychange"));
			}, hidden);

		await page.goto(consolePath(project.slug));
		await expect(card(page)).toBeVisible();
		await expect.poll(() => polls, { timeout: 8_000 }).toBe(2);
		const written = await puts();
		await expect.poll(() => polls, { timeout: 8_000 }).toBe(3);
		await page.waitForTimeout(1_500);
		expect(await puts()).toBe(written);

		await hide(true);
		const hidden = polls;
		await page.waitForTimeout(7_000);
		expect(polls).toBe(hidden);
		await hide(false);
		await expect.poll(() => polls, { timeout: 8_000 }).toBeGreaterThan(hidden);
	});

	// Kills blank Explore that unmounts the editor on screen for its skeleton
	// while it decides, which empties the page for an API round.
	test("from an Explore link, the Explore tab keeps the editor while blank Explore decides", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await postReport(request, project.slug, {
			start: new Date(Date.parse(seed.now) - DAY).toISOString(),
			results: {
				hash: [
					{ parameters: { size: 1 }, measures: { latency: { value: 1 } } },
				],
			},
		});
		const id = await dimensions(request, project.slug);
		const search = new URLSearchParams({
			branches: id.branch("main"),
			testbeds: id.testbed("ubuntu-latest"),
			benchmarks: id.benchmark("hash"),
			measures: id.measure("Latency"),
		});
		await page.goto(`${consolePath(project.slug, "explore")}?${search}`);
		const query = page.getByRole("region", { name: "Query" });
		await expect(query).toBeVisible();
		const panel = await query.elementHandle();
		await page.evaluate(() => {
			const holder = window as Window & { __skeleton?: boolean };
			new MutationObserver(() => {
				holder.__skeleton ||=
					document.querySelector('main[aria-busy="true"]') !== null;
			}).observe(document.body, { childList: true, subtree: true });
		});
		await page.route(`${seed.api_url}/**`, async (route) => {
			await new Promise((resolve) => setTimeout(resolve, 800));
			await route.continue();
		});
		await tabRow(page).getByRole("link", { name: "Explore" }).click();
		await expect(
			page.getByRole("region", { name: "Latest runs" }),
		).toBeVisible();
		expect(await panel?.evaluate((element) => element.isConnected)).toBe(true);
		expect(
			await page.evaluate(
				() => (window as Window & { __skeleton?: boolean }).__skeleton,
			),
		).toBeFalsy();
	});

	// Kills blank Explore held on its skeleton for good when its latest runs
	// do not load.
	test("blank Explore draws when its latest runs fail", async ({ page }) => {
		await page.route(
			(url) =>
				url.pathname === `/v0/console/projects/${hashbrown.slug}/runs` &&
				url.searchParams.get("per_page") === "4",
			(route) => route.fulfill({ status: 500, json: {} }),
		);
		await page.goto(consolePath(hashbrown.slug, "explore"));
		await expect(page.getByRole("region", { name: "Query" })).toBeVisible({
			timeout: 15_000,
		});
	});

	test("the setup card passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, consolePath(empty.slug), () =>
			expect(
				card(page).getByRole("button", { name: "Create key" }),
			).toBeEnabled(),
		);
	});
});

test.describe("a reader who is not a Maintainer", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills Create key offered to a reader the API refuses it to.
	test("sees the setup card without Create key", async ({ page }) => {
		await page.goto(consolePath(empty.slug));
		await expect(card(page)).toBeVisible();
		await expect(
			card(page).getByText("Read only", { exact: true }),
		).toBeVisible();
		await expect(
			card(page).getByRole("button", { name: "Create key" }),
		).toHaveCount(0);
	});
});

test.describe("narrow", () => {
	test.use({
		storageState: signedIn(seed.member),
		viewport: { width: 390, height: 844 },
	});

	// Kills a card wider than a phone, and a Create key under 44 px.
	test("the setup card fits a phone", async ({ page }) => {
		await page.goto(consolePath(empty.slug));
		const create = card(page).getByRole("button", { name: "Create key" });
		await expect(create).toBeVisible();
		expect((await create.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		expect(
			await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth,
			),
		).toBe(true);
	});
});
