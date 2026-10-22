import type { APIRequestContext, Page } from "@playwright/test";
import { createProject, listPlots, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import {
	dimensions,
	expectNameOverUnits,
	explore,
	plotAtEnd,
	twoLines,
} from "./explore";
import {
	VERSION_KEY,
	expect,
	nextPath,
	seed,
	signedIn,
	tabRow,
	test,
} from "./fixtures";
import { DAY } from "./public";

const { hashbrown } = seed.projects;
const HOUR = DAY / 24;

const plot = (page: Page) => page.getByRole("figure");
const box = (page: Page, name: string) => page.getByRole("group", { name });
const keyEntries = (page: Page) =>
	page.getByRole("button", { name: /: (hide|show) its line$/ });
const params = (page: Page) => new URL(page.url()).searchParams;

test.use({ storageState: signedIn(seed.member), timezoneId: "UTC" });

// Kills a link that draws more than its lines (a parameters filter not sent),
// a key whose alerting line is not first, and box values left as UUIDs.
test("a link draws exactly its lines, the alerting one first", async ({
	page,
	request,
}) => {
	const { query, id } = await twoLines(request);
	// The API answers in box order, so the alerting line on main comes third.
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			branches: [
				{ uuid: id.branch("feature-simd") },
				{ uuid: id.branch("main") },
			],
		}),
	);

	await expect(plot(page)).toHaveAccessibleName("4 lines");
	await expect(keyEntries(page)).toHaveText([
		/blake3\s*simd=avx2\s*main/,
		/blake3\s*simd=avx2\s*feature-simd/,
		/blake3\s*simd=sse4\.2\s*feature-simd/,
		/blake3\s*simd=sse4\.2\s*main/,
	]);
	await expect(
		box(page, "Branches").getByRole("button", { name: "Remove branch main" }),
	).toBeVisible();
	await expect(
		box(page, "Measures").getByRole("button", {
			name: "Remove measure Latency",
		}),
	).toBeVisible();
});

// Kills values left as UUIDs when the query cannot draw yet, so no plot
// answer will ever name them.
test("a link that cannot draw yet still names its values", async ({
	page,
	request,
}) => {
	const { id } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			benchmarks: [id.benchmark("blake3")],
			measures: [id.measure("Latency")],
		}),
	);
	await expect(
		box(page, "Benchmarks").getByRole("button", {
			name: "Remove benchmark blake3",
		}),
	).toBeVisible();
	await expect(
		box(page, "Measures").getByRole("button", {
			name: "Remove measure Latency",
		}),
	).toBeVisible();
	await expect(plot(page)).toHaveCount(0);
});

// Kills an edit that leaves the URL or the lines as they were, and an edit
// written over the history entry, so Back would not restore the query.
test("editing each box changes the lines and the link, and Back restores", async ({
	page,
	request,
}) => {
	const { query, id } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");

	await box(page, "Branches")
		.getByRole("button", { name: "Add branch" })
		.click();
	await page.getByRole("menuitem", { name: "feature-simd" }).click();
	await expect(plot(page)).toHaveAccessibleName("4 lines");
	expect(params(page).get("branches")).toBe(
		`${id.branch("main")},${id.branch("feature-simd")}`,
	);

	await box(page, "Branches")
		.getByRole("button", { name: "Remove branch feature-simd" })
		.click();
	await expect(plot(page)).toHaveAccessibleName("2 lines");

	await box(page, "Testbeds")
		.getByRole("button", { name: "Add testbed" })
		.click();
	await page.getByRole("menuitem", { name: "macos-latest" }).click();
	await expect(plot(page)).toHaveAccessibleName("4 lines");
	await box(page, "Testbeds")
		.getByRole("button", { name: "Remove testbed macos-latest" })
		.click();
	await expect(plot(page)).toHaveAccessibleName("2 lines");

	await box(page, "Benchmarks")
		.getByRole("button", { name: "Add benchmark" })
		.click();
	await page.getByRole("menuitem", { name: "sha256" }).click();
	await expect(plot(page)).toHaveAccessibleName("3 lines");
	expect(params(page).get("benchmarks")).toBe(
		`${id.benchmark("blake3")},${id.benchmark("sha256")}`,
	);

	await box(page, "Parameters")
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await expect(plot(page)).toHaveAccessibleName("12 lines");
	expect(params(page).has("parameters")).toBe(false);

	await box(page, "Measures")
		.getByRole("button", { name: "Add measure" })
		.click();
	await page.getByRole("menuitem", { name: /^Throughput/ }).click();
	await expect(plot(page)).toHaveAccessibleName("24 lines");

	await box(page, "Metrics")
		.getByRole("button", { name: "Remove metric value" })
		.click();
	await expect(page).not.toHaveURL(/metrics=/);
	await expect(plot(page)).toHaveAccessibleName("24 lines");

	await page.goBack();
	await expect(page).toHaveURL(/metrics=value/);
	await page.goBack();
	await expect(plot(page)).toHaveAccessibleName("12 lines");
	await page.goBack();
	await expect(plot(page)).toHaveAccessibleName("3 lines");
	expect(params(page).getAll("parameters")).toEqual([
		'{"input_bytes":65536,"threads":1}',
	]);
});

// Kills a plot that empties or shows a spinner while an edit loads, and one
// that stays marked busy once the answer arrives.
test("the lines drawn stay, marked busy, until the edit's answer arrives", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");

	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	await page.route(
		`${seed.api_url}/v0/console/projects/*/perf?*`,
		async (route) => {
			await held;
			await route.continue();
		},
	);
	// When the plot was first marked busy, on the page's clock.
	await page.evaluate(() => {
		const holder = window as Window & { __busy?: number };
		const frame = document.querySelector('section[aria-label="Plot"]');
		if (frame) {
			new MutationObserver(() => {
				if (
					holder.__busy === undefined &&
					frame.getAttribute("aria-busy") === "true"
				) {
					holder.__busy = performance.now();
				}
			}).observe(frame, { attributeFilter: ["aria-busy"] });
		}
	});
	const asked = page.waitForRequest(/\/v0\/console\/projects\/[^/]+\/perf\?/);
	await box(page, "Parameters")
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await asked;
	const frame = page.getByRole("region", { name: "Plot" });
	await expect(frame).toHaveAttribute("aria-busy", "true");
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	release();
	await expect(plot(page)).toHaveAccessibleName("8 lines");
	await expect(frame).toHaveAttribute("aria-busy", "false");
	// Busy from the edit itself, not only once the request it waits for starts.
	const { busy, sent } = await page.evaluate(() => ({
		busy: (window as Window & { __busy?: number }).__busy ?? Number.NaN,
		sent:
			performance
				.getEntriesByType("resource")
				.filter(({ name }) =>
					/\/v0\/console\/projects\/[^/]+\/perf\?/.test(name),
				)
				.at(-1)?.startTime ?? Number.NaN,
	}));
	expect(sent - busy).toBeGreaterThan(100);
});

// Kills Back to a query the browser has drawn waiting out the pause an edit
// takes, or asking the API again.
test("Back to a drawn query redraws at once, with no request", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await box(page, "Parameters")
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await expect(plot(page)).toHaveAccessibleName("8 lines");

	const asked: string[] = [];
	page.on("request", (sent) => {
		if (/\/v0\/console\/projects\/[^/]+\/perf\?/.test(sent.url())) {
			asked.push(sent.url());
		}
	});
	// Frames drawn between Back and the plot naming the lines it went back to.
	await page.evaluate(() => {
		const holder = window as Window & { __back?: Promise<number> };
		holder.__back = new Promise((resolve) => {
			addEventListener(
				"popstate",
				() => {
					const drawn = () =>
						document.querySelector("figure")?.getAttribute("aria-label") ===
						"2 lines";
					// The router may redraw before this listener runs.
					if (drawn()) {
						resolve(0);
						return;
					}
					let frames = 0;
					const count = () => {
						frames += 1;
						requestAnimationFrame(count);
					};
					requestAnimationFrame(count);
					const observer = new MutationObserver(() => {
						if (drawn()) {
							observer.disconnect();
							resolve(frames);
						}
					});
					observer.observe(document.body, {
						subtree: true,
						attributeFilter: ["aria-label"],
					});
				},
				{ once: true },
			);
		});
	});
	await page.goBack();
	const frames = await page.evaluate(
		() => (window as Window & { __back?: Promise<number> }).__back,
	);
	test.info().annotations.push({ type: "frames", description: String(frames) });
	expect(frames).toBeLessThanOrEqual(1);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	expect(asked).toEqual([]);
});

// Kills a plot query the API refuses or never answers leaving a frame that
// waits for good, a Retry that does not ask again, and a refused edit that
// takes away the lines already drawn or leaves them dimmed.
test("a refused plot query says so, keeps what was drawn, and Retry asks again", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	const perf = `${seed.api_url}/v0/console/projects/*/perf?*`;
	const failed = page.getByRole("region", { name: "Plot" }).getByRole("alert");
	const retry = failed.getByRole("button", { name: "Retry" });

	await page.route(perf, (route) => route.fulfill({ status: 500, body: "{}" }));
	await page.goto(explore(hashbrown.slug, query));
	// An API that did not answer is asked twice more before the page says so.
	await expect(failed).toContainText("did not load", { timeout: 15_000 });
	await page.unroute(perf);
	await retry.click();
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(failed).toHaveCount(0);

	await page.route(perf, (route) => route.fulfill({ status: 400, body: "{}" }));
	await box(page, "Parameters")
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await expect(failed).toContainText("refused");
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(page.getByRole("region", { name: "Plot" })).toHaveAttribute(
		"aria-busy",
		"false",
	);
	await page.unroute(perf);
	await retry.click();
	await expect(plot(page)).toHaveAccessibleName("8 lines");
	await expect(failed).toHaveCount(0);
});

// Kills a plot whose own code did not load leaving the frame loading for good
// with no banner, a Retry that does not load it again or reads the whole page
// again to do it, and one that throws.
test("a plot whose code did not load says so, and Retry loads it in place", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	const thrown: Error[] = [];
	page.on("pageerror", (error) => thrown.push(error));
	const failed = page.getByRole("region", { name: "Plot" }).getByRole("alert");
	await page.route(PLOT_CHUNK, (route) => route.abort());
	await page.goto(explore(hashbrown.slug, query));
	await expect(failed).toContainText("The plot did not load");
	await page.unroute(PLOT_CHUNK);
	await page.evaluate(() => {
		Object.assign(window, { stayed: true });
	});
	await failed.getByRole("button", { name: "Retry" }).click();
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(failed).toHaveCount(0);
	expect(await page.evaluate(() => "stayed" in window)).toBe(true);
	expect(thrown).toEqual([]);
});

// Kills a Retry that keeps asking for a chunk a newer deploy took away, which
// only reading the page again replaces.
test("Retry reads the page again when the server no longer has the plot's code", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	const thrown: Error[] = [];
	page.on("pageerror", (error) => thrown.push(error));
	const failed = page.getByRole("region", { name: "Plot" }).getByRole("alert");
	await page.route(PLOT_CHUNK, (route) => route.fulfill({ status: 404 }));
	await page.goto(explore(hashbrown.slug, query));
	await expect(failed).toContainText("The plot did not load");
	await page.evaluate(() => {
		Object.assign(window, { stayed: true });
	});
	await Promise.all([
		page.waitForEvent("load"),
		failed.getByRole("button", { name: "Retry" }).click(),
	]);
	expect(await page.evaluate(() => "stayed" in window)).toBe(false);
	await expect(failed).toContainText("The plot did not load");
	expect(thrown).toEqual([]);
});

// Kills a Retry that imports the plot again in place after one of the plot's
// own imports failed: that module stays failed for the page's life, so only
// reading the page again draws the plot.
test("Retry reads the page again when one of the plot's own imports did not load", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	const link = explore(hashbrown.slug, query);
	const plotImport = await plotOnlyImport(request, link);
	const thrown: Error[] = [];
	page.on("pageerror", (error) => thrown.push(error));
	const failed = page.getByRole("region", { name: "Plot" }).getByRole("alert");
	await page.route(plotImport, (route) => route.abort());
	await page.goto(link);
	await expect(failed).toContainText("The plot did not load");
	await page.unroute(plotImport);
	await page.evaluate(() => {
		Object.assign(window, { stayed: true });
	});
	await Promise.all([
		page.waitForEvent("load"),
		failed.getByRole("button", { name: "Retry" }).click(),
	]);
	expect(await page.evaluate(() => "stayed" in window)).toBe(false);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(failed).toHaveCount(0);
	expect(thrown).toEqual([]);
});

/** The plot's own chunk, under any later try's URL too. */
const PLOT_CHUNK = /\/assets\/Plot\.[^/]*\.js(\?.*)?$/;

/** A chunk only the plot's own code imports: one `link`'s document names and blank Explore's does not. */
const plotOnlyImport = async (request: APIRequestContext, link: string) => {
	const named = async (path: string) =>
		[
			...(await (await request.get(path)).text()).matchAll(
				/<link\b[^>]*\bhref="([^"]+\.js)"/g,
			),
		].map(([, href]) => href ?? "");
	const blank = new Set(await named(nextPath(hashbrown.slug, "explore")));
	const [own] = (await named(link)).filter(
		(href) => !(blank.has(href) || PLOT_CHUNK.test(href)),
	);
	expect(own).toBeDefined();
	return (url: URL) => url.pathname === own;
};

// Kills a refused-query banner that wraps or grows past the plot's first axes
// rows onto the lines it keeps, or stops short of the two rows a two-axis plot
// heads with on a phone, and a Retry under 44 px, at any width.
for (const measures of [["Latency"], ["Latency", "Throughput"]]) {
	test(`a refused edit's banner covers the plot's axes rows at every width with ${measures.length} measures`, async ({
		page,
		request,
	}) => {
		const { id, query } = await twoLines(request);
		await page.goto(
			explore(hashbrown.slug, { ...query, measures: measures.map(id.measure) }),
		);
		const drawn = `${2 * measures.length} lines`;
		await expect(plot(page)).toHaveAccessibleName(drawn);
		await page.route(`${seed.api_url}/v0/console/projects/*/perf?*`, (route) =>
			route.fulfill({ status: 400, body: "{}" }),
		);
		await box(page, "Parameters")
			.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
			.click();
		const failed = page
			.getByRole("region", { name: "Plot" })
			.getByRole("alert");
		await expect(failed).toBeVisible();

		for (const [width, settled] of [
			[1280, box(page, "Branches")],
			[768, box(page, "Branches")],
			[390, page.getByRole("button", { name: "branches main" })],
		] as const) {
			await page.setViewportSize({ width, height: 844 });
			await expect(settled).toBeVisible();
			const banner = await failed.boundingBox();
			const drawing = await plot(page)
				.locator(".pl-area")
				.first()
				.boundingBox();
			const retry = await failed
				.getByRole("button", { name: "Retry" })
				.boundingBox();
			const overlap =
				(banner?.y ?? 0) + (banner?.height ?? 0) - (drawing?.y ?? 0);
			expect
				.soft(Math.abs(overlap), `banner edge off the lines' top at ${width}`)
				.toBeLessThanOrEqual(4);
			expect
				.soft(retry?.height, `Retry at ${width}`)
				.toBeGreaterThanOrEqual(44);
		}
		await expect(plot(page)).toHaveAccessibleName(drawn);
	});
}

// Kills a covered set merged away or counted as adding variants, and a row
// without its own count.
test("the parameters box counts each set and flags one a wider set covers", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			sets: [{ simd: "avx2" }, { simd: "avx2", threads: 1 }],
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("4 lines");
	const parameters = box(page, "Parameters");
	await expect(parameters).toContainText("4 variants");
	await expect(parameters).toContainText("adds 0 variants");
	await expect(
		parameters.getByRole("button", { name: "Remove set simd=avx2 threads=1" }),
	).toBeVisible();
});

// Kills a third measure drawn on a dual axis, a layout control offered when
// it cannot apply, a layout choice that does not reach the link, and a
// Measures box that names the other layout.
test("two measures share a dual axis by default, three always stack", async ({
	page,
	request,
}) => {
	const { query, id } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			measures: [id.measure("Latency"), id.measure("Throughput")],
		}),
	);
	const layout = page.getByRole("radiogroup", { name: "Measures layout" });
	await expect(layout.getByRole("radio", { name: "Dual axis" })).toBeChecked();
	await expect(page.locator("figure")).toHaveAttribute("data-layout", "dual");
	await expect(box(page, "Measures")).toContainText("two y axes");
	await layout.getByText("Stacked").click();
	await expect(page.locator("figure")).toHaveAttribute(
		"data-layout",
		"stacked",
	);
	await expect(box(page, "Measures")).not.toContainText("two y axes");
	await expect(box(page, "Measures")).toContainText("stacked");
	expect(params(page).get("layout")).toBe("stacked");

	const project = await createProject(request);
	await postReport(request, project.slug, {
		start: "2026-09-10T00:00:00Z",
		results: {
			bake: [
				{
					measures: {
						latency: { value: 1 },
						throughput: { value: 2 },
						instructions: { value: 3 },
					},
				},
			],
		},
	});
	const own = await dimensions(request, project.slug);
	await page.goto(
		explore(project.slug, {
			branches: [{ uuid: own.branch("main") }],
			testbeds: [{ uuid: own.testbed("ubuntu-latest") }],
			benchmarks: [own.benchmark("bake")],
			measures: ["Latency", "Throughput", "Instructions"].map(own.measure),
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("3 lines");
	await expect(
		page.getByRole("radiogroup", { name: "Measures layout" }),
	).toHaveCount(0);
	await expect(page.locator("figure")).toHaveAttribute(
		"data-layout",
		"stacked",
	);
});

// Kills hidden or focused lines kept only in the page, so a shared link loses them.
test("hidden and focused lines travel in the link", async ({
	page,
	request,
	context,
}) => {
	const { query } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await page
		.getByRole("button", { name: /^blake3 simd=sse4\.2.*: hide its line$/ })
		.click();
	await page
		.getByRole("button", { name: /^Focus line of blake3 simd=avx2/ })
		.click();
	await expect(page).toHaveURL(/hide=/);
	await expect(page).toHaveURL(/focus=/);

	const other = await context.newPage();
	await other.goto(page.url());
	await expect(
		other.getByRole("button", {
			name: /^blake3 simd=sse4\.2.*: show its line$/,
		}),
	).toBeVisible();
	await expect(
		other.getByRole("button", { name: /^Clear focus of blake3 simd=avx2/ }),
	).toBeVisible();
});

// Kills an Explore link from when Explore was the project root drawn as Runs,
// which drops its query, or moved to Explore without its query.
test("an Explore link at the project root opens Explore with its query", async ({
	page,
	request,
}) => {
	const link = explore(hashbrown.slug, (await twoLines(request)).query);
	const { search } = new URL(link, seed.console_url);
	await page.goto(`${nextPath(hashbrown.slug)}${search}`);
	await expect(page).toHaveURL(link);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
});

// A released CLI's pull request comment tracks every link it writes.
const COMMENT_UTM =
	"utm_medium=referral&utm_source=github&utm_content=comment&utm_campaign=pr+comments&utm_term=hashbrown";

// Kills a comment's project link that its tracking sends from Runs to Explore.
test("a comment's project link with its tracking opens Runs", async ({
	page,
}) => {
	const link = `${nextPath(hashbrown.slug)}?${COMMENT_UTM}`;
	await page.goto(link);
	await expect(page).toHaveURL(link);
	await expect(
		page
			.getByRole("navigation", { name: "Project" })
			.getByRole("link", { name: "Runs" }),
	).toHaveAttribute("aria-current", "page");
	await expect(
		page.getByRole("table", { name: "Runs, newest first" }),
	).toBeVisible();
});

// Kills a comment's plot link that its tracking keeps from Explore.
test("a comment's plot link with its tracking at the project root opens Explore", async ({
	page,
	request,
}) => {
	const link = explore(hashbrown.slug, (await twoLines(request)).query);
	const { search } = new URL(link, seed.console_url);
	await page.goto(`${nextPath(hashbrown.slug)}${search}&${COMMENT_UTM}`);
	await expect(page).toHaveURL(link);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
});

// Kills a blank Explore without its starting points, and an alert that opens
// more than its line or names a source that does not link to that alert.
test("blank Explore starts from alerts, runs, and pins", async ({ page }) => {
	await page.goto(nextPath(hashbrown.slug, "explore"));
	const alerting = page.getByRole("region", { name: "Alerting now" });
	const alert = alerting.getByRole("link", { name: /^blake3 / });
	await expect(alert).toHaveCount(1);
	await expect(alert).toHaveAccessibleName(
		/^blake3 input_bytes=65536 simd=avx2 threads=1 Latency main · ubuntu-latest · \+\d+\.\d%$/,
	);
	await expect(
		page.getByRole("region", { name: "Latest runs" }).getByRole("button", {
			name: new RegExp(`^main, run ${seed.last_main_hash}, `),
		}),
	).toBeVisible();
	await expect(
		page.getByRole("region", { name: "Pinned plots" }),
	).toBeVisible();

	await alert.click();
	await expect(plot(page)).toHaveAccessibleName("1 line");
	await expect(page).toHaveURL(/focus=/);
	await expect(page.getByText(/on main · ubuntu-latest$/)).toBeVisible();
	await page.getByRole("link", { name: "alert", exact: true }).click();
	await expect(page).toHaveURL(
		new RegExp(`${nextPath(hashbrown.slug, "alerts")}/[0-9a-f-]{36}$`),
	);
	await expect(page.getByRole("heading", { level: 1 })).toContainText("blake3");
});

const latestRuns = (page: Page) =>
	page.getByRole("region", { name: "Latest runs" });

// Kills a latest run that leaves blank Explore or lists its reports out of
// start order or without their testbeds, a report search over more than that
// report ran, a pick that draws another testbed or asks again, a source line
// that does not name the report or link to it, and a hash that does not open
// the run.
test("a latest run opens to its reports, and a report to the benchmarks it ran; a pick draws one from that report", async ({
	page,
}) => {
	const asked: string[] = [];
	page.on("request", (sent) => {
		if (
			/\/v0\/console\/projects\/[^/]+\/(runs|reports)\/[^/?]+/.test(sent.url())
		) {
			asked.push(new URL(sent.url()).pathname.split("/")[5] ?? "");
		}
	});
	await page.goto(nextPath(hashbrown.slug, "explore"));
	// The newest run on main sent its reports from both testbeds.
	const row = latestRuns(page).getByRole("button", {
		name: new RegExp(`^main, run ${seed.last_main_hash}, `),
	});
	await row.click();
	await expect(row).toHaveAttribute("aria-expanded", "true");
	await expect(page).toHaveURL(
		new RegExp(`${nextPath(hashbrown.slug, "explore")}$`),
	);
	const reports = page.getByRole("region", {
		name: `Reports in run ${seed.last_main_hash}`,
	});
	await expect(reports.getByRole("button")).toHaveText([
		/^1\s*main\s*· ubuntu-latest$/,
		/^2\s*main\s*· macos-latest$/,
	]);
	await reports.getByRole("button", { name: /^2 main · macos-latest/ }).click();
	const search = page.getByRole("searchbox", {
		name: "Search benchmarks in report 2 on macos-latest",
		exact: true,
	});
	await expect(search).toBeFocused();
	await expect(page.getByRole("menuitem")).toHaveText(["blake3", "xxh3"]);
	await search.pressSequentially("bla");
	await expect(page.getByRole("menuitem")).toHaveCount(1);
	await page.keyboard.press("ArrowDown");
	await page.keyboard.press("Enter");

	// The key report ran one variant of blake3 in both measures.
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(
		box(page, "Testbeds").getByRole("button", { name: /^Remove testbed / }),
	).toHaveAccessibleName("Remove testbed macos-latest");
	const query = page.getByRole("region", { name: "Query" });
	await expect(query).toContainText(`from report ${seed.last_main_hash}`);
	const report = params(page).get("report");
	await expect(
		query.getByRole("link", { name: seed.last_main_hash, exact: true }),
	).toHaveAttribute("href", `${nextPath(hashbrown.slug, "reports")}/${report}`);
	expect(asked).toEqual(["runs", "reports"]);

	await page.goBack();
	await latestRuns(page)
		.getByRole("link", { name: seed.last_main_hash, exact: true })
		.click();
	await expect(page).toHaveURL(
		new RegExp(`${nextPath(hashbrown.slug, "runs")}/[0-9a-f-]{36}$`),
	);
	await expect(page.getByRole("heading", { level: 1 })).toContainText(
		seed.last_main_hash,
	);
});

/** A project of its own whose one report ran `alpha` in two measures and `beta` in one, `days` before the seed's now. */
const reported = async (request: APIRequestContext, days = 1) => {
	const project = await createProject(request);
	const report = await postReport(request, project.slug, {
		hash: "5ab1e50000000000000000000000000000000000",
		start: new Date(Date.parse(seed.now) - days * DAY).toISOString(),
		results: {
			alpha: [
				{
					parameters: { size: 1 },
					measures: { latency: { value: 1 }, throughput: { value: 2 } },
				},
			],
			beta: [
				{ parameters: { size: 1 }, measures: { latency: { value: 3 } } },
				{ parameters: { size: 2 }, measures: { latency: { value: 4 } } },
			],
		},
	});
	return { project, report };
};

// Kills a project whose only run predates the latest runs' window listed as
// one that never ran, and a report older than the default window picked over
// a window that holds none of its points, which draws nothing.
test("a latest run older than the window still lists, and its report draws over the 4 weeks ending at it", async ({
	page,
	request,
}) => {
	const { project, report } = await reported(request, 35);
	await page.goto(nextPath(project.slug, "explore"));
	await latestRuns(page)
		.getByRole("button", { name: /^main, run 5ab1e50, / })
		.click();
	await page
		.getByRole("region", { name: "Reports in run 5ab1e50" })
		.getByRole("button", { name: /^1 main · ubuntu-latest/ })
		.click();
	await page.getByRole("menuitem", { name: "beta" }).click();
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(page.getByRole("region", { name: "Query" })).toContainText(
		"from report 5ab1e50",
	);
	expect(params(page).get("end_time")).toBe(
		String(Date.parse(report.end_time)),
	);
});

/** A project of its own, with one benchmark's two variants run on `main`. */
const pinnable = async (request: APIRequestContext) => {
	const project = await createProject(request);
	await postReport(request, project.slug, {
		start: "2026-09-10T00:00:00Z",
		results: {
			hash: [
				{ parameters: { size: 1 }, measures: { latency: { value: 1 } } },
				{ parameters: { size: 2 }, measures: { latency: { value: 2 } } },
			],
		},
	});
	const id = await dimensions(request, project.slug);
	return {
		project,
		query: {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("hash")],
			sets: [{ size: 1 }],
			measures: [id.measure("Latency")],
		},
	};
};

// Kills a pin titled without what tells the line apart from its benchmark's
// other variants or after a line the key hides, a pin not at the top of
// Plots, and an Undo that leaves it.
test("Pin to Plots pins the shown line by its full name, and Undo removes it", async ({
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, { ...query, sets: [] }));
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await page
		.getByRole("button", { name: /^hash size=2.*: hide its line$/ })
		.click();

	await page.getByRole("button", { name: "Pin to Plots" }).click();
	const confirmation = page.getByRole("status").filter({ hasText: "Pinned" });
	await expect(confirmation).toContainText(
		'Pinned "hash size=1, Latency on main" · 4w',
	);
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: "hash size=1, Latency on main",
		}),
	).toBeVisible();
	const [pin] = await listPlots(request, project.slug);
	expect(pin).toMatchObject({
		title: "hash size=1, Latency on main",
		window: 28 * 24 * 60 * 60,
	});
	expect(pin?.hidden).toHaveLength(1);

	await page.getByRole("button", { name: "Undo" }).click();
	await expect(page.getByText("Unsaved plot")).toBeVisible();
	await expect.poll(() => listPlots(request, project.slug)).toEqual([]);
});

// Kills a pin missing from blank Explore or opened as anything but itself, an
// edit to it that never marks it, a Save that does not write the pin or leaves
// the link unlike what the pin keeps, and a reload that brings back the old pin
// or the mark.
test("an edited pin shows the unsaved mark until Save, which writes it", async ({
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, query));
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	await expect(page.getByRole("button", { name: "Save as new" })).toBeVisible();

	await page.goto(nextPath(project.slug, "explore"));
	const pins = page.getByRole("region", { name: "Pinned plots" });
	const pin = pins.getByRole("link").filter({ hasText: "Latency on main" });
	await expect(pin).toHaveText([/hash size=1, Latency on main/]);
	await pin.click();
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: "hash size=1, Latency on main",
		}),
	).toBeVisible();
	const mark = page.getByText("Unsaved changes");
	await expect(mark).toHaveCount(0);

	await page
		.getByRole("radiogroup", { name: "Y scale" })
		.getByText("Log")
		.click();
	await expect(mark).toBeVisible();
	// A pin keeps a rolling window, so a custom range saves as its length.
	const window = page.getByRole("radiogroup", { name: "Window" });
	await window.getByText("Custom").click();
	await expect(
		page.getByRole("group", { name: "Custom window" }),
	).toBeVisible();
	await page.getByRole("button", { name: "Save", exact: true }).click();
	await expect(mark).toHaveCount(0);
	await expect(window.getByRole("radio", { name: "4w" })).toBeChecked();
	await expect
		.poll(async () => (await listPlots(request, project.slug))[0]?.y_axis)
		.toBe("log");

	await page.reload();
	await expect(
		page.getByRole("radiogroup", { name: "Y scale" }).getByRole("radio", {
			name: "Log",
		}),
	).toBeChecked();
	await expect(mark).toHaveCount(0);
	// The saved link took the place of the edit it saved, so Back skips it.
	await page.goBack();
	await expect(window.getByRole("radio", { name: "4w" })).toBeChecked();
});

// Kills leaving a pin's unsaved changes behind without asking, and a prompt
// that keeps the reader from leaving once they choose to.
test("leaving a pin with unsaved changes asks first", async ({
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, query));
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	// An edit before the pin lands edits the unsaved plot, not the pin.
	await expect(
		page.getByRole("status").filter({ hasText: "Pinned" }),
	).toBeVisible();
	await page
		.getByRole("radiogroup", { name: "X axis" })
		.getByText("Version")
		.click();
	await expect(page.getByText("Unsaved changes")).toBeVisible();

	await tabRow(page).getByRole("link", { name: "Runs" }).click();
	const dialog = page.getByRole("alertdialog", {
		name: "Leave without saving?",
	});
	await expect(dialog).toBeVisible();
	await dialog.getByRole("button", { name: "Keep editing" }).first().click();
	await expect(dialog).toHaveCount(0);
	await expect(page.getByText("Unsaved changes")).toBeVisible();

	// Closing the tab gets the browser's own prompt.
	const asked = new Promise<string>((resolve) => {
		page.once("dialog", async (prompt) => {
			resolve(prompt.type());
			await prompt.dismiss();
		});
	});
	await page.close({ runBeforeUnload: true });
	expect(await asked).toBe("beforeunload");
	await expect(page.getByText("Unsaved changes")).toBeVisible();

	await tabRow(page).getByRole("link", { name: "Runs" }).click();
	await dialog.getByRole("button", { name: "Leave without saving" }).click();
	await expect(page).toHaveURL(new RegExp(`${nextPath(project.slug)}$`));
	expect((await listPlots(request, project.slug))[0]?.x_axis).toBe("date_time");

	await page.goBack();
	await expect(page.getByText("Unsaved changes")).toBeVisible();
	await tabRow(page).getByRole("link", { name: "Runs" }).click();
	await dialog.getByRole("button", { name: "Save and leave" }).click();
	await expect(page).toHaveURL(new RegExp(`${nextPath(project.slug)}$`));
	await expect
		.poll(async () => (await listPlots(request, project.slug))[0]?.x_axis)
		.toBe("version");
});

// Kills a Discard that keeps the edit, and a Save as new that writes over the
// pin or pins nothing.
test("Discard returns a pin to what it saved, and Save as new pins the edit beside it", async ({
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, query));
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	await expect(
		page.getByRole("status").filter({ hasText: "Pinned" }),
	).toBeVisible();
	const mark = page.getByText("Unsaved changes");
	const xAxis = page.getByRole("radiogroup", { name: "X axis" });
	await xAxis.getByText("Version").click();
	await expect(mark).toBeVisible();
	await page.getByRole("button", { name: "Discard" }).click();
	await expect(mark).toHaveCount(0);
	await expect(xAxis.getByRole("radio", { name: "Report date" })).toBeChecked();

	await xAxis.getByText("Version").click();
	await page.getByRole("button", { name: "Save as new" }).click();
	await expect(
		page.getByRole("status").filter({ hasText: "Saved as new" }),
	).toBeVisible();
	await expect(mark).toHaveCount(0);
	await expect
		.poll(async () =>
			(await listPlots(request, project.slug)).map(({ x_axis }) => x_axis),
		)
		.toEqual(["version", "date_time"]);
});

// Kills a bare link to a pin that never opens the pin's query, and a link to a
// pin deleted since that still opens as that pin.
test("a link to a pin opens its query, and one to a deleted pin is an unsaved plot", async ({
	newContext,
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, query));
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	await expect(page.getByRole("button", { name: "Save as new" })).toBeVisible();
	const pinned = page.url();
	const [pin] = await listPlots(request, project.slug);

	await page.goto(explore(project.slug, { plot: pin?.uuid ?? "" }));
	await expect(plot(page)).toHaveAccessibleName("1 line");
	await expect(
		page.getByRole("heading", {
			level: 1,
			name: "hash size=1, Latency on main",
		}),
	).toBeVisible();
	expect(params(page).get("benchmarks")).toBe(query.benchmarks[0]);

	await request.delete(
		`${seed.api_url}/v0/projects/${project.slug}/plots/${pin?.uuid}`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	expect(await listPlots(request, project.slug)).toEqual([]);
	// A fresh browser, since this one still holds the pin it read.
	const context = await newContext({
		storageState: signedIn(seed.member),
	});
	const fresh = await context.newPage();
	await fresh.goto(pinned);
	await expect(fresh.getByText("Unsaved plot")).toBeVisible();
	expect(params(fresh).has("plot")).toBe(false);
	await context.close();
});

// Kills a Clear that leaves a box or a view control off its default, adds
// more than one history entry, or drops the focus, and a Back that does not
// restore the query and its view.
test("Clear returns Explore to its defaults in one step, and Back restores the query and its view", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			xAxis: "version",
			yScale: "log",
			window: { seconds: 7 * 24 * 60 * 60 },
			layout: "stacked",
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	const drawn = page.url();
	const entries = await page.evaluate(() => history.length);
	const clear = page.getByRole("button", { name: "Clear", exact: true });

	await clear.click();
	await expect(
		box(page, "Branches").getByRole("button", { name: "Add branch" }),
	).toBeFocused();
	await expect(
		page.getByRole("region", { name: "Pinned plots" }),
	).toBeVisible();
	expect([...params(page)]).toEqual([]);
	expect(await page.evaluate(() => history.length)).toBe(entries + 1);
	await expect(clear).toHaveCount(0);

	await page.goBack();
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	expect(page.url()).toBe(drawn);
	await expect(
		page
			.getByRole("radiogroup", { name: "Y scale" })
			.getByRole("radio", { name: "Log" }),
	).toBeChecked();
});

// Kills a Clear whose arrival moves the boxes, the chips, the view controls,
// or what sits under the query panel: at the end of the boxes' row, as it
// was, or without its held place where it takes a line of its own.
for (const [width, ownLine] of [
	[1280, false],
	[390, false],
	[320, false],
	[350, true],
	[800, true],
] as const) {
	test(`Clear appearing moves nothing at ${width}`, async ({ page }) => {
		await page.setViewportSize({ width, height: 900 });
		await page.goto(nextPath(hashbrown.slug, "explore"));
		const query = page.getByRole("region", { name: "Query" });
		const boxes =
			width < 768
				? query.getByRole("button", {
						name: /^(branches|testbeds|benchmarks|parameters|measures|metrics) /,
					})
				: query.getByRole("group");
		await expect(boxes).toHaveCount(6);
		// A segmented control's own width follows its choice, so only its place counts.
		const placed = async () => [
			...(await boxes.evaluateAll((all) =>
				all.map((each) => JSON.stringify(each.getBoundingClientRect())),
			)),
			...(await query.getByRole("radiogroup").evaluateAll((all) =>
				all.map((each) => {
					const { left, top } = each.getBoundingClientRect();
					return `${left},${top}`;
				}),
			)),
			JSON.stringify(await query.boundingBox()),
			JSON.stringify(
				await page.getByRole("region", { name: "Start from" }).boundingBox(),
			),
		];
		const clear = page.getByRole("button", { name: "Clear", exact: true });
		await expect(clear).toHaveCount(0);
		const before = await placed();

		await page
			.getByRole("radiogroup", { name: "Y scale" })
			.getByText("Log")
			.click();
		await expect(clear).toBeVisible();
		if (ownLine) {
			// The controls' last line has no room for it here, so only its held place keeps the page still.
			expect(
				await clear.evaluate((button) => {
					const others = [...(button.parentElement?.children ?? [])].filter(
						(each) => each !== button,
					);
					return (
						button.getBoundingClientRect().top >=
						Math.max(
							...others.map((each) => each.getBoundingClientRect().bottom),
						) -
							1
					);
				}),
			).toBe(true);
		}
		expect(await placed()).toEqual(before);
	});
}

// Kills a Clear that stops short of the view controls row's right edge, as it
// did when it wrapped onto a line of its own.
for (const [width, wrapped] of [
	[320, false],
	[1440, true],
] as const) {
	test(`Clear ends the view controls row at ${width}`, async ({
		page,
		request,
	}) => {
		await page.setViewportSize({ width, height: 900 });
		if (wrapped) {
			const { id, query } = await twoLines(request);
			const end = Date.parse(seed.now);
			await page.goto(
				explore(hashbrown.slug, {
					...query,
					measures: [id.measure("Latency"), id.measure("Throughput")],
					window: { start: end - 14 * DAY, end },
				}),
			);
			await expect(
				page.getByRole("radiogroup", { name: "Measures layout" }),
			).toBeVisible();
		} else {
			await page.goto(nextPath(hashbrown.slug, "explore"));
			await page
				.getByRole("radiogroup", { name: "Y scale" })
				.getByText("Log")
				.click();
		}
		const clear = page.getByRole("button", { name: "Clear", exact: true });
		await expect(clear).toBeVisible();
		const [right, edge] = await clear.evaluate((button) => {
			const row = button.parentElement as HTMLElement;
			const style = getComputedStyle(row);
			return [
				button.getBoundingClientRect().right,
				row.getBoundingClientRect().right -
					Number.parseFloat(style.paddingRight) -
					Number.parseFloat(style.borderRightWidth),
			];
		});
		expect(Math.abs((right ?? 0) - (edge ?? 0))).toBeLessThan(1);
	});
}

// Kills a measure's units crunched beside its name, or either cut short, on a
// wide screen.
test("a measure shows its name over its units at 1280", async ({
	page,
	request,
}) => {
	await page.setViewportSize({ width: 1280, height: 900 });
	const { query } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expectNameOverUnits(
		box(page, "Measures"),
		"Latency",
		"nanoseconds (ns)",
	);
});

// Kills a Clear drawn only for some boxes, such as one that needs a benchmark.
test("a link whose only value is a measure draws Clear, which empties the box", async ({
	page,
	request,
}) => {
	const { id } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, { measures: [id.measure("Latency")] }),
	);
	const remove = box(page, "Measures").getByRole("button", {
		name: /^Remove measure /,
	});
	await expect(remove).toBeVisible();

	await page.getByRole("button", { name: "Clear", exact: true }).click();
	await expect(remove).toHaveCount(0);
	expect(params(page).has("measures")).toBe(false);
});

// Kills a Clear on a pin that keeps the pin, which then fills the query back
// from it, and a Back that loses the pin's unsaved edit.
test("Clear on a pin with unsaved changes leaves the pin, and Back brings the edit back", async ({
	page,
	request,
}) => {
	const { project, query } = await pinnable(request);
	await page.goto(explore(project.slug, query));
	await page.getByRole("button", { name: "Pin to Plots" }).click();
	await expect(page.getByRole("button", { name: "Save as new" })).toBeVisible();
	const scale = page.getByRole("radiogroup", { name: "Y scale" });
	await scale.getByText("Log").click();
	const mark = page.getByText("Unsaved changes");
	await expect(mark).toBeVisible();

	await page.getByRole("button", { name: "Clear", exact: true }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "Unsaved plot" }),
	).toBeVisible();
	await expect(page.getByRole("button", { name: "Add branch" })).toBeFocused();
	expect(params(page).has("plot")).toBe(false);

	await page.goBack();
	await expect(mark).toBeVisible();
	await expect(scale.getByRole("radio", { name: "Log" })).toBeChecked();
	expect(params(page).has("plot")).toBe(true);
});

// Kills a query past the line cap drawn without saying how many lines it left out.
test("a query past 64 lines says so", async ({ page, request }) => {
	const id = await dimensions(request, hashbrown.slug);
	await page.goto(
		explore(hashbrown.slug, {
			branches: [
				{ uuid: id.branch("main") },
				{ uuid: id.branch("feature-simd") },
			],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: ["blake3", "sha256", "xxh3", "crc32c"].map(id.benchmark),
			measures: [id.measure("Latency"), id.measure("Throughput")],
		}),
	);
	await expect(
		page.getByRole("status").filter({ hasText: "of 64 lines" }),
	).toContainText("72 of 64 lines");
	await expect(plot(page)).toHaveAccessibleName("64 lines");
});

// Kills a readout that takes the pointer before it is pinned, so a click on
// the plot beneath it pins nothing, or a pinned readout whose report link a
// mouse cannot reach.
test("a click pins the readout, and only a pinned readout's report link opens the report", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	const now = Date.parse(seed.now);
	const results = (value: number) => ({
		hash: [{ parameters: { size: 1 }, measures: { latency: { value } } }],
	});
	// Two reports an hour apart among three weeks: their points sit a few pixels apart.
	await postReport(request, project.slug, {
		start: new Date(now - 21 * DAY).toISOString(),
		results: results(1),
	});
	const second = await postReport(request, project.slug, {
		start: new Date(now - 21 * DAY + HOUR).toISOString(),
		results: results(2),
	});
	await postReport(request, project.slug, {
		start: new Date(now - DAY).toISOString(),
		results: results(3),
	});
	const id = await dimensions(request, project.slug);
	await page.goto(
		explore(project.slug, {
			branches: [{ uuid: id.branch("main") }],
			testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
			benchmarks: [id.benchmark("hash")],
			measures: [id.measure("Latency")],
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("1 line");
	const area = await page
		.getByRole("group", { name: /^1 line,/ })
		.boundingBox();
	if (!area) {
		throw new Error("No plot area");
	}
	await page.mouse.move(area.x + 0.5, area.y + area.height - 10);
	const status = plot(page).getByRole("status");
	const hint = status.getByText("Click to pin");
	const link = status.getByRole("link");
	await expect(hint).toBeVisible();
	await expect(link).toHaveCount(0);
	// The hint sits nearer the second report's point than the first's, and a
	// click on it reaches the plot beneath.
	const over = await hint.boundingBox();
	if (!over) {
		throw new Error("No hint");
	}
	await page.mouse.move(over.x + over.width / 2, over.y + over.height / 2, {
		steps: 4,
	});
	await page.mouse.down();
	await page.mouse.up();
	await expect(link).toHaveAttribute("href", new RegExp(second.uuid));
	await expect(hint).toHaveCount(0);
	const goal = await link.boundingBox();
	if (!goal) {
		throw new Error("No report link");
	}
	await page.mouse.move(goal.x + goal.width / 2, goal.y + goal.height / 2, {
		steps: 8,
	});
	await page.mouse.down();
	await page.mouse.up();
	await expect(page).toHaveURL(new RegExp(`/reports/${second.uuid}`));
});

// Kills scroll room that leaves the plot's top short of the top of the screen
// or lets it scroll past (a full screen of room under the page's padding),
// whatever the key's height, on a link and on a pin.
for (const viewport of [
	{ width: 1280, height: 800 },
	{ width: 390, height: 844 },
]) {
	test(`the plot scrolls exactly to the top of the screen at ${viewport.width}`, async ({
		page,
		request,
	}) => {
		await page.setViewportSize(viewport);
		const { id, query } = await twoLines(request);
		const pin = (await listPlots(request, hashbrown.slug)).find(
			({ title }) =>
				title === "blake3 input_bytes=65536 threads=1, Latency on main",
		);
		for (const [link, lines] of [
			[explore(hashbrown.slug, query), "2 lines"],
			[
				explore(hashbrown.slug, {
					...query,
					benchmarks: [id.benchmark("blake3")],
					sets: [],
				}),
				"8 lines",
			],
			[explore(hashbrown.slug, { plot: pin?.uuid ?? "" }), "2 lines"],
		] as const) {
			await page.goto(link);
			await expect(plot(page)).toHaveAccessibleName(lines);
			const { top } = await plotAtEnd(page);
			expect(Math.abs(top), link).toBeLessThanOrEqual(1);
		}
	});
}

// Kills room added below a plot already taller than the screen.
test("a plot taller than the screen ends the page at its key", async ({
	page,
	request,
}) => {
	await page.setViewportSize({ width: 1280, height: 480 });
	const { id, query } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			benchmarks: [id.benchmark("blake3")],
			sets: [],
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("8 lines");
	const { top, below } = await plotAtEnd(page);
	expect(top).toBeLessThan(-1);
	expect(below).toBeLessThanOrEqual(1);
});

// Kills a classic perf link that lands in Explore without its query, through
// the version the browser remembers or the one the classic navbar reads.
test("a classic perf link opens the same query in Explore", async ({
	newContext,
	request,
}) => {
	const { query } = await twoLines(request);
	const classic = new URLSearchParams({
		branches: query.branches.map(({ uuid }) => uuid).join(","),
		testbeds: query.testbeds.map(({ uuid }) => uuid).join(","),
		benchmarks: query.benchmarks.join(","),
		measures: query.measures.join(","),
		x_axis: "version",
		clear: "true",
		tab: "benchmarks",
	});
	for (const memory of [
		{ [VERSION_KEY]: JSON.stringify({ [hashbrown.slug]: 1 }) },
		{},
	]) {
		const context = await newContext({
			storageState: signedIn(seed.member, memory),
		});
		const page = await context.newPage();
		await page.goto(`/console/projects/${hashbrown.slug}/perf?${classic}`);
		await expect(page).toHaveURL(/\/next\/console\/projects\/.*\/explore\?/);
		await expect(page.getByRole("figure")).toHaveAccessibleName("8 lines");
		expect(params(page).get("x_axis")).toBe("version");
		expect(params(page).has("clear")).toBe(false);
		await context.close();
	}
});

// Kills pin and save controls drawn for a reader who cannot use them, and a
// Share that copies anything but the link.
test("a viewer can share but not pin", async ({ newContext, request }) => {
	const { query } = await twoLines(request);
	const context = await newContext({
		storageState: signedIn(seed.outsider),
		permissions: ["clipboard-read", "clipboard-write"],
	});
	const page = await context.newPage();
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await expect(page.getByRole("button", { name: "Pin to Plots" })).toHaveCount(
		0,
	);
	await page.getByRole("button", { name: "Share" }).click();
	await page.getByRole("menuitem", { name: "Copy link" }).click();
	await expect(page.getByRole("button", { name: "Link copied" })).toBeVisible();
	expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
		page.url(),
	);
	await context.close();
});

test.describe("narrow", () => {
	test.use({ viewport: { width: 390, height: 844 } });

	// Kills a narrow query panel that keeps the boxes on the page, chips that do
	// not name their dimension, and targets under 44 px.
	test("the query folds into chips that open a sheet", async ({
		page,
		request,
	}) => {
		const { query } = await twoLines(request);
		await page.goto(explore(hashbrown.slug, query));
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		const chip = page.getByRole("button", { name: "branches main" });
		await expect(chip).toBeVisible();
		await expect(box(page, "Branches")).toBeHidden();
		const size = await chip.boundingBox();
		expect(size?.height).toBeGreaterThanOrEqual(44);

		await chip.click();
		const sheet = page.getByRole("dialog", { name: "Query" });
		await expect(sheet.getByRole("group", { name: "Branches" })).toBeVisible();
		const remove = sheet.getByRole("button", { name: "Remove branch main" });
		expect((await remove.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		await sheet.getByRole("button", { name: "Done" }).click();
		await expect(sheet).toBeHidden();
	});

	// Kills a Clear missing from a phone's query panel, a target under 44 px
	// there, and a focus that falls with it.
	test("Clear is a 44 px target in the view controls", async ({
		page,
		request,
	}) => {
		const { query } = await twoLines(request);
		await page.goto(explore(hashbrown.slug, query));
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		const clear = page.getByRole("button", { name: "Clear", exact: true });
		const size = await clear.boundingBox();
		expect(
			Math.min(size?.width ?? 0, size?.height ?? 0),
		).toBeGreaterThanOrEqual(44);
		await clear.click();
		await expect(
			page.getByRole("button", { name: /^branches / }),
		).toBeFocused();
	});

	// Kills a measure's units crunched beside its name, or either cut short,
	// in a phone's query sheet.
	test("a measure shows its name over its units in the sheet", async ({
		page,
		request,
	}) => {
		const { query } = await twoLines(request);
		await page.goto(explore(hashbrown.slug, query));
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		await page.getByRole("button", { name: /^measures / }).click();
		await expectNameOverUnits(
			page
				.getByRole("dialog", { name: "Query" })
				.getByRole("group", { name: "Measures" }),
			"Latency",
			"nanoseconds (ns)",
		);
	});
});

// Kills a contrast, name, or role failure on blank Explore's starting points,
// or on a latest run's report opened to its benchmarks, in either theme.
test("blank Explore passes axe in both themes", async ({ page }) => {
	const runs = latestRuns(page);
	await axeInBothThemes(page, nextPath(hashbrown.slug, "explore"), async () => {
		await expect(
			page
				.getByRole("region", { name: "Alerting now" })
				.getByRole("link", { name: /^blake3 / }),
		).toHaveCount(1);
		await expect(runs.getByRole("link")).not.toHaveCount(0);
	});
	await axeInBothThemes(
		page,
		nextPath(hashbrown.slug, "explore"),
		async () => {
			await expect(page.getByRole("menuitem").first()).toBeVisible();
		},
		async () => {
			await runs.getByRole("button", { expanded: false }).first().click();
			await page
				.getByRole("region", { name: /^Reports in run / })
				.getByRole("button")
				.first()
				.click();
		},
	);
});

// Kills a source line, from an alert or from a report, whose link reads only
// by its color or fails a contrast or a name in either theme.
test("Explore opened from an alert or a report passes axe in both themes", async ({
	page,
	request,
}) => {
	await axeInBothThemes(
		page,
		nextPath(hashbrown.slug, "explore"),
		async () => {
			await expect(
				page.getByRole("link", { name: "alert", exact: true }),
			).toBeVisible();
			await expect(plot(page)).toHaveAccessibleName("1 line");
		},
		() =>
			page
				.getByRole("region", { name: "Alerting now" })
				.getByRole("link", { name: /^blake3 / })
				.click(),
	);

	const response = await request.get(
		`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
		{ headers: { Authorization: `Bearer ${seed.member.token}` } },
	);
	const [newest] = (await response.json()) as { uuid: string }[];
	const { query } = await twoLines(request);
	await axeInBothThemes(
		page,
		explore(hashbrown.slug, { ...query, report: newest?.uuid ?? "" }),
		async () => {
			await expect(
				page
					.getByRole("region", { name: "Query" })
					.getByRole("link", { name: /^[0-9a-f]{7}$/ }),
			).toBeVisible();
			await expect(plot(page)).toHaveAccessibleName("2 lines");
		},
	);
});

// Kills a contrast, name, or role failure on Explore in either theme.
test("Explore passes axe in both themes", async ({ page, request }) => {
	const { query } = await twoLines(request);
	await axeInBothThemes(page, explore(hashbrown.slug, query), async () => {
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		// Pin turns primary once the plot answers.
		await expect(
			page.getByRole("button", { name: "Pin to Plots" }),
		).toBeEnabled();
	});
});
