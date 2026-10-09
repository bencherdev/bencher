import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { decodeQuery } from "../src/next/query/query";
import type { JsonBranch, JsonProject } from "../src/types/bencher";
import { createProject } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, nextPath, seed, settle, signedIn, test } from "./fixtures";

const { hashbrown } = seed.projects;

const READ_ONLY = "Read only. Ask a project Maintainer for access.";

const HOUR = 60 * 60 * 1_000;

/** The member's answer from the API, which refuses with its status. */
const call = async <T>(
	request: APIRequestContext,
	method: "GET" | "POST" | "PATCH",
	path: string,
	data?: unknown,
): Promise<T> => {
	const response = await request.fetch(`${seed.api_url}${path}`, {
		method,
		headers: { Authorization: `Bearer ${seed.member.token}` },
		...(data === undefined ? {} : { data }),
	});
	if (!response.ok()) {
		throw new Error(
			`${method} ${path} answered ${response.status()}: ${await response.text()}`,
		);
	}
	return (await response.json()) as T;
};

const LATENCY_THRESHOLD = {
	models: [
		{
			measure: "latency",
			metric: "value",
			model: {
				test: "t_test",
				min_sample_size: 4,
				max_sample_size: 64,
				upper_boundary: 0.99,
			},
		},
	],
};

/**
 * A project of its own, oldest report first: `main` and `feature-simd` each
 * declare a latency threshold on `ubuntu-latest`, `main` reports again on
 * `macos-latest`, and `devel` reports last with no threshold.
 */
const dimensionsProject = async (request: APIRequestContext) => {
	const project: JsonProject = await createProject(request);
	const reports = [
		{ branch: "main", testbed: "ubuntu-latest", thresholds: true },
		{ branch: "feature-simd", testbed: "ubuntu-latest", thresholds: true },
		{ branch: "main", testbed: "macos-latest", thresholds: false },
		{ branch: "devel", testbed: "ubuntu-latest", thresholds: false },
	];
	for (const [index, report] of reports.entries()) {
		const start = Date.parse(seed.now) - (reports.length - index) * HOUR;
		const results = {
			blake3: [1, 4].map((threads) => ({
				parameters: { input_bytes: 1024, threads },
				measures: { latency: { value: 100 + threads + index } },
			})),
		};
		await call(request, "POST", `/v0/projects/${project.slug}/reports`, {
			branch: report.branch,
			hash: `${index}e2e000000000000000000000000000000000000`.slice(0, 40),
			testbed: report.testbed,
			start_time: new Date(start).toISOString(),
			end_time: new Date(start + 60_000).toISOString(),
			results: [JSON.stringify(results)],
			settings: { adapter: "json" },
			...(report.thresholds ? { thresholds: LATENCY_THRESHOLD } : {}),
		});
	}
	return project;
};

/** The branches the project's thresholds sit on, archived or active. */
const thresholdBranches = async (
	request: APIRequestContext,
	slug: string,
	archived: boolean,
) =>
	(
		await call<{ branch: { name: string } }[]>(
			request,
			"GET",
			`/v0/projects/${slug}/thresholds?archived=${archived}`,
		)
	)
		.map(({ branch }) => branch.name)
		.sort();

const branch = (request: APIRequestContext, project: string, slug: string) =>
	call<JsonBranch>(request, "GET", `/v0/projects/${project}/branches/${slug}`);

const list = (page: Page, name: string) => page.getByRole("table", { name });

/** The names of the rows in a list, top to bottom. */
const rowNames = (table: Locator) =>
	table.getByRole("row").getByRole("link").allTextContents();

const status = (page: Page, name: RegExp) =>
	page.getByRole("radiogroup", { name: "Status" }).getByRole("radio", { name });

test.describe("Dimensions, as the project's Maintainer", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills rows without their last report or threshold count, a toggle that
	// counts the rows on screen instead of the totals, a list in another order,
	// and a crumb that leads a list to itself.
	test("each list shows its rows with their thresholds and the totals", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await page.goto(nextPath(project.slug, "branches"));
		await expect(
			page.getByRole("heading", { level: 1, name: "Branches" }),
		).toBeVisible();
		const branches = list(page, "Active branches");
		await expect(branches.getByRole("row")).toHaveCount(4);
		expect(await rowNames(branches)).toEqual(["devel", "feature-simd", "main"]);
		const main = branches.getByRole("row").filter({ hasText: "main" });
		await expect(main.getByRole("cell").nth(4)).toHaveText("1", {
			useInnerText: true,
		});
		await expect(
			branches
				.getByRole("row")
				.filter({ hasText: "devel" })
				.getByRole("cell")
				.nth(4),
		).toHaveText("0", { useInnerText: true });
		await expect(status(page, /^Active/)).toHaveAccessibleName("Active 3");
		await expect(status(page, /^Archived/)).toHaveAccessibleName("Archived 0");

		await page
			.getByRole("navigation", { name: "Dimension lists" })
			.getByRole("link", { name: "Testbeds" })
			.click();
		const testbeds = list(page, "Active testbeds");
		expect(await rowNames(testbeds)).toEqual(["macos-latest", "ubuntu-latest"]);
		// The crumb and the rail both lead to the Dimensions page, not this list.
		const dimensions = page
			.getByRole("main")
			.getByRole("link", { name: "Dimensions", exact: true });
		const hrefs = await dimensions.evaluateAll((links) =>
			links.map((link) => link.getAttribute("href")),
		);
		expect(hrefs.length).toBeGreaterThan(1);
		expect(new Set(hrefs)).toEqual(
			new Set([nextPath(project.slug, "branches")]),
		);
		await expect(
			testbeds
				.getByRole("row")
				.filter({ hasText: "ubuntu-latest" })
				.getByRole("cell")
				.nth(4),
		).toHaveText("2", { useInnerText: true });

		await page
			.getByRole("navigation", { name: "Dimension lists" })
			.getByRole("link", { name: "Measures" })
			.click();
		const measures = list(page, "Active measures");
		await expect(
			measures
				.getByRole("row")
				.filter({ hasText: "Latency" })
				.getByRole("cell")
				.nth(4),
		).toHaveText("2", { useInnerText: true });
	});

	// Kills a sort that the API never hears, a direction that does not flip,
	// and a picked sort that keeps the last one's direction.
	test("sorting by last used reads newest first, then oldest first", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await page.goto(nextPath(project.slug, "branches"));
		const branches = list(page, "Active branches");
		await expect(branches.getByRole("row")).toHaveCount(4);

		await page
			.getByRole("radiogroup", { name: "Sort" })
			.getByRole("radio", { name: "Last used" })
			.check();
		await expect
			.poll(() => rowNames(branches))
			.toEqual(["devel", "main", "feature-simd"]);
		await page
			.getByRole("button", { name: "Sort direction: Newest first" })
			.click();
		await expect
			.poll(() => rowNames(branches))
			.toEqual(["feature-simd", "main", "devel"]);
		await expect(
			page.getByRole("button", { name: "Sort direction: Oldest first" }),
		).toBeVisible();

		// The sort is in the link, so a reload keeps it.
		await page.reload();
		await expect
			.poll(() => rowNames(list(page, "Active branches")))
			.toEqual(["feature-simd", "main", "devel"]);
	});

	// Kills an archive without its impact line first, one that leaves its
	// thresholds active, a row that leaves the list instead of dimming in place,
	// and an Undo that brings back the branch without its threshold.
	test("archiving feature-simd says what it does, archives it and its threshold, and Undo restores both", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await page.goto(nextPath(project.slug, "branches"));
		const branches = list(page, "Active branches");
		const row = branches.getByRole("row").filter({ hasText: "feature-simd" });
		await row.getByRole("button", { name: "Archive feature-simd" }).click();
		const impact = page.getByRole("group", { name: "Archive feature-simd" });
		await expect(impact).toContainText(
			"Archiving feature-simd archives 1 threshold and hides its lines. A run that reports it brings it back.",
		);
		await impact.getByRole("button", { name: "Archive feature-simd" }).click();

		await expect(row).toContainText("Archived just now");
		await expect(status(page, /^Active/)).toHaveAccessibleName("Active 2");
		await expect
			.poll(
				async () =>
					(await branch(request, project.slug, "feature-simd")).archived,
			)
			.toBeTruthy();
		expect(await thresholdBranches(request, project.slug, false)).toEqual([
			"main",
		]);
		expect(await thresholdBranches(request, project.slug, true)).toEqual([
			"feature-simd",
		]);

		await row.getByRole("button", { name: "Undo, feature-simd" }).click();
		await expect(
			row.getByRole("button", { name: "Archive feature-simd" }),
		).toBeVisible();
		await expect
			.poll(
				async () =>
					(await branch(request, project.slug, "feature-simd")).archived,
			)
			.toBeFalsy();
		expect(await thresholdBranches(request, project.slug, false)).toEqual([
			"feature-simd",
			"main",
		]);
		await expect(status(page, /^Active/)).toHaveAccessibleName("Active 3");
	});

	// Kills an Archived toggle that lists the active rows, and an Unarchive that
	// does not say which thresholds come back.
	test("Archived lists an archived branch, and Unarchive brings back its threshold", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await call(
			request,
			"PATCH",
			`/v0/projects/${project.slug}/branches/feature-simd`,
			{
				archived: true,
			},
		);
		await page.goto(nextPath(project.slug, "branches"));
		await expect(list(page, "Active branches").getByRole("row")).toHaveCount(3);
		await status(page, /^Archived/).check();
		const archived = list(page, "Archived branches");
		expect(await rowNames(archived)).toEqual(["feature-simd"]);

		await archived
			.getByRole("button", { name: "Unarchive feature-simd" })
			.click();
		await expect(page.getByRole("main").getByRole("status")).toContainText(
			"Unarchived feature-simd and 1 threshold.",
		);
		await expect
			.poll(
				async () =>
					(await branch(request, project.slug, "feature-simd")).archived,
			)
			.toBeFalsy();
		expect(await thresholdBranches(request, project.slug, false)).toEqual([
			"feature-simd",
			"main",
		]);
	});

	// Kills a refused archive left showing as archived.
	test("a refused archive puts the row back and says so", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await page.route(
			`${seed.api_url}/v0/projects/${project.slug}/branches/*`,
			async (route) => {
				if (route.request().method() === "PATCH") {
					await route.fulfill({ status: 500, body: "{}" });
					return;
				}
				await route.fallback();
			},
		);
		await page.goto(nextPath(project.slug, "branches"));
		const row = list(page, "Active branches")
			.getByRole("row")
			.filter({ hasText: "devel" });
		await row.getByRole("button", { name: "Archive devel" }).click();
		await page
			.getByRole("group", { name: "Archive devel" })
			.getByRole("button", { name: "Archive devel" })
			.click();
		await expect(page.getByRole("alert")).toContainText(
			"Bencher did not archive devel",
		);
		await expect(
			row.getByRole("button", { name: "Archive devel" }),
		).toBeVisible();
		await expect(status(page, /^Active/)).toHaveAccessibleName("Active 3");
	});

	// Kills a variant archive that the API never hears, and a variant row that
	// leaves instead of dimming with its Undo.
	test("a variant archives from its benchmark's page", async ({
		page,
		request,
	}) => {
		const project = await dimensionsProject(request);
		await page.goto(nextPath(project.slug, "benchmarks/blake3"));
		await expect(
			page.getByRole("heading", { level: 1, name: "blake3" }),
		).toBeVisible();
		const variants = page.getByRole("table", { name: "Active variants" });
		await expect(variants.getByRole("row")).toHaveCount(3);
		const label = "Archive blake3 input_bytes=1024 threads=4";
		await variants.getByRole("button", { name: label }).click();
		await page
			.getByRole("group", { name: label })
			.getByRole("button", { name: "Archive variant" })
			.click();
		await expect(
			variants.getByRole("row").filter({ hasText: "threads=4" }),
		).toContainText("Archived just now");
		await expect
			.poll(async () =>
				(
					await call<{ parameters: { threads?: number } }[]>(
						request,
						"GET",
						`/v0/projects/${project.slug}/benchmarks/blake3/variants?archived=true`,
					)
				).map(({ parameters }) => parameters.threads),
			)
			.toEqual([4]);
	});

	// Kills an inspect page that drops the head, its start point, the recent
	// reports, or the thresholds, and an Explore link for another branch.
	test("a branch's page shows its head, recent reports, and thresholds", async ({
		page,
		request,
	}) => {
		const main = await branch(request, hashbrown.slug, "main");
		await page.goto(nextPath(hashbrown.slug, "branches/main"));
		await expect(
			page.getByRole("heading", { level: 1, name: "main" }),
		).toBeVisible();
		const crumb = page
			.getByRole("main")
			.getByRole("link", { name: "Branches" });
		await expect(crumb).toHaveAttribute(
			"href",
			nextPath(hashbrown.slug, "branches"),
		);
		await expect(page.getByRole("region", { name: "Head" })).toContainText(
			seed.last_main_hash,
		);
		const reports = page.getByRole("table", { name: "Recent reports" });
		await expect(reports.getByRole("row").nth(1)).toContainText("macos-latest");
		await expect(reports.getByRole("row").nth(2)).toContainText(
			"ubuntu-latest",
		);
		const thresholds = page.getByRole("table", { name: "Thresholds" });
		await expect(thresholds.getByRole("row")).toHaveCount(2);
		await expect(thresholds.getByRole("row").nth(1)).toContainText(
			"ubuntu-latest",
		);
		await expect(thresholds.getByRole("row").nth(1)).toContainText("Latency");

		const explore = page.getByRole("link", { name: "Open in Explore" });
		const href = new URL(
			(await explore.getAttribute("href")) ?? "",
			seed.console_url,
		);
		expect(href.pathname).toBe(nextPath(hashbrown.slug, "explore"));
		expect(decodeQuery(href.search).branches).toEqual([{ uuid: main.uuid }]);
	});

	// Kills a testbed or measure page without its thresholds, and a measure
	// page without its units.
	test("a testbed's and a measure's pages show their thresholds", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "testbeds/ubuntu-latest"));
		await expect(
			page.getByRole("heading", { level: 1, name: "ubuntu-latest" }),
		).toBeVisible();
		await expect(
			page.getByRole("table", { name: "Thresholds" }).getByRole("row").nth(1),
		).toContainText("main");

		await page.goto(nextPath(hashbrown.slug, "measures/latency"));
		await expect(
			page.getByRole("heading", { level: 1, name: "Latency" }),
		).toBeVisible();
		await expect(page.getByRole("region", { name: "Units" })).toContainText(
			"nanoseconds (ns)",
		);
		await expect(
			page.getByRole("table", { name: "Thresholds" }).getByRole("row").nth(1),
		).toContainText("ubuntu-latest");
	});

	// Kills a benchmark page that lists the never reported empty variant or
	// miscounts the values in use.
	test("a benchmark's page shows its parameters in use and its variants", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "benchmarks/blake3"));
		await expect(
			page.getByRole("heading", { level: 1, name: "blake3" }),
		).toBeVisible();
		const params = page.getByRole("region", { name: "Parameters in use" });
		await expect(params).toContainText("input_bytes");
		await expect(params).toContainText("simd");
		await expect(
			page.getByRole("table", { name: "Active variants" }).getByRole("row"),
		).toHaveCount(9);
	});

	// Kills contrast, names, and roles that fail axe in either theme.
	test("a list, a branch's page, and a benchmark's page pass axe in both themes", async ({
		page,
	}) => {
		await axeInBothThemes(
			page,
			nextPath(hashbrown.slug, "branches"),
			async () => {
				await expect(
					list(page, "Active branches").getByRole("link", { name: "main" }),
				).toBeVisible();
				await settle(page);
			},
		);
		await axeInBothThemes(
			page,
			nextPath(hashbrown.slug, "branches/main"),
			async () => {
				await expect(
					page.getByRole("table", { name: "Thresholds" }).getByRole("row"),
				).toHaveCount(2);
				await settle(page);
			},
		);
		await axeInBothThemes(
			page,
			nextPath(hashbrown.slug, "benchmarks/blake3"),
			async () => {
				await expect(
					page.getByRole("table", { name: "Active variants" }).getByRole("row"),
				).toHaveCount(9);
				await settle(page);
			},
		);
	});
});

test.describe("as a reader who is a member of nothing", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills Archive drawn for a reader the API would refuse.
	test("the lists and the pages draw no Archive", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "branches"));
		await expect(page.getByText(READ_ONLY)).toBeVisible();
		await expect(
			list(page, "Active branches").getByRole("link", { name: "main" }),
		).toBeVisible();
		await expect(page.getByRole("button", { name: /^Archive/ })).toHaveCount(0);

		await page.goto(nextPath(hashbrown.slug, "branches/main"));
		await expect(
			page.getByRole("heading", { level: 1, name: "main" }),
		).toBeVisible();
		await expect(page.getByText(READ_ONLY)).toBeVisible();
		await expect(page.getByRole("button", { name: /Archive/ })).toHaveCount(0);
	});
});

test.describe("narrow", () => {
	test.use({
		storageState: signedIn(seed.member),
		viewport: { width: 390, height: 844 },
	});

	// Kills a list that keeps its desktop columns on a phone, targets under
	// 44 px, and an inspect page that puts its counts below the cards.
	test("rows fold to two lines, targets are 44 px, and a page leads with its counts", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "branches"));
		const row = list(page, "Active branches")
			.getByRole("row")
			.filter({ hasText: "feature-simd" });
		await expect(row).toBeVisible();
		await expect(
			list(page, "Active branches").getByRole("columnheader", {
				name: "Created",
			}),
		).toBeHidden();
		const targets = [
			...(await page
				.getByRole("navigation", { name: "Dimension lists" })
				.getByRole("link")
				.all()),
			page.getByRole("searchbox", { name: "Filter branches" }),
			page.getByRole("radio", { name: "Name" }).locator(".."),
			page.getByRole("button", { name: /^Sort direction/ }),
			status(page, /^Active/).locator(".."),
			row.getByRole("button", { name: "Archive feature-simd" }),
		];
		for (const target of targets) {
			const box = await target.boundingBox();
			expect(box?.height).toBeGreaterThanOrEqual(44);
		}

		await page.goto(nextPath(hashbrown.slug, "branches/main"));
		const counts = page.getByRole("list", { name: "On this branch" });
		const head = page.getByRole("region", { name: "Head" });
		await expect(head).toBeVisible();
		const countsBox = await counts.boundingBox();
		const headBox = await head.boundingBox();
		expect(countsBox?.y ?? 0).toBeLessThan(headBox?.y ?? 0);
		expect(headBox?.width).toBeGreaterThan(300);
	});
});
