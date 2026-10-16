import type { APIRequestContext, Locator, Page } from "@playwright/test";
import { createProject, listPlots, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { dimensions, explore, plotAtEnd, twoLines } from "./explore";
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
	const blank = new Set(await named(nextPath(hashbrown.slug)));
	const [own] = (await named(link)).filter(
		(href) => !(blank.has(href) || PLOT_CHUNK.test(href)),
	);
	expect(own).toBeDefined();
	return (url: URL) => url.pathname === own;
};

// Kills a refused-query banner that wraps or grows past the plot's head row
// onto the lines it keeps, and a Retry under 44 px, at any width.
test("a refused edit's banner keeps to the plot's head row at every width", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(explore(hashbrown.slug, query));
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	await page.route(`${seed.api_url}/v0/console/projects/*/perf?*`, (route) =>
		route.fulfill({ status: 400, body: "{}" }),
	);
	await box(page, "Parameters")
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	const failed = page.getByRole("region", { name: "Plot" }).getByRole("alert");
	await expect(failed).toBeVisible();

	for (const [width, settled] of [
		[1280, box(page, "Branches")],
		[768, box(page, "Branches")],
		[390, page.getByRole("button", { name: "branches main" })],
	] as const) {
		await page.setViewportSize({ width, height: 844 });
		await expect(settled).toBeVisible();
		const banner = await failed.boundingBox();
		const drawing = await plot(page).locator(".pl-body").boundingBox();
		const retry = await failed
			.getByRole("button", { name: "Retry" })
			.boundingBox();
		const overlap =
			(banner?.y ?? 0) + (banner?.height ?? 0) - (drawing?.y ?? 0);
		expect
			.soft(Math.abs(overlap), `banner edge past the plot's top at ${width}`)
			.toBeLessThanOrEqual(4);
		expect.soft(retry?.height, `Retry at ${width}`).toBeGreaterThanOrEqual(44);
	}
	await expect(plot(page)).toHaveAccessibleName("2 lines");
});

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
	await page.getByRole("button", { name: /^Focus blake3 simd=avx2/ }).click();
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
		other.getByRole("button", { name: /^Focus blake3 simd=avx2/ }),
	).toHaveAttribute("aria-pressed", "true");
});

// Kills a blank Explore without its starting points, an alert that opens more
// than its line or names a source that does not link to the alerts, and a
// benchmark picked first that leaves the branch, the testbed, and the measures
// for the reader.
test("blank Explore starts from alerts, reports, and pins, and a first benchmark fills the rest", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug));
	const alerting = page.getByRole("region", { name: "Alerting now" });
	const alert = alerting.getByRole("link", { name: /^blake3 / });
	await expect(alert).toHaveCount(1);
	await expect(alert).toHaveAccessibleName(
		/^blake3 input_bytes=65536 simd=avx2 threads=1 Latency main · ubuntu-latest · \+\d+\.\d%$/,
	);
	await expect(
		page.getByRole("region", { name: "Latest reports" }).getByRole("link", {
			name: /main · macos-latest/,
		}),
	).toBeVisible();
	await expect(
		page.getByRole("region", { name: "Pinned plots" }),
	).toBeVisible();

	await box(page, "Benchmarks")
		.getByRole("button", { name: "Add benchmark" })
		.click();
	// The filled values are named before the plot answers.
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const perf = `${seed.api_url}/v0/console/projects/*/perf?*`;
	await page.route(perf, async (route) => {
		await held;
		await route.continue();
	});
	await page.getByRole("menuitem", { name: "sha256" }).click();
	// The newest report the API took with sha256 is the last run on feature-simd.
	await expect(
		box(page, "Branches").getByRole("button", {
			name: "Remove branch feature-simd",
		}),
	).toBeVisible();
	await expect(
		box(page, "Testbeds").getByRole("button", {
			name: "Remove testbed ubuntu-latest",
		}),
	).toBeVisible();
	await expect(
		box(page, "Measures").getByRole("button", {
			name: /^Remove measure (Latency|Throughput)$/,
		}),
	).toHaveCount(2);
	release();
	await page.unroute(perf);
	await expect(plot(page)).toHaveAccessibleName("8 lines");

	await page.goto(nextPath(hashbrown.slug));
	await alert.click();
	await expect(plot(page)).toHaveAccessibleName("1 line");
	await expect(page).toHaveURL(/focus=/);
	await expect(page.getByText(/on main · ubuntu-latest$/)).toBeVisible();
	await page.getByRole("link", { name: "alert", exact: true }).click();
	await expect(page).toHaveURL(
		new RegExp(`${nextPath(hashbrown.slug, "alerts")}$`),
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

	await page.goto(nextPath(project.slug));
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

	await tabRow(page).getByRole("link", { name: "Reports" }).click();
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

	await tabRow(page).getByRole("link", { name: "Reports" }).click();
	await dialog.getByRole("button", { name: "Leave without saving" }).click();
	await expect(page).toHaveURL(/\/reports$/);
	expect((await listPlots(request, project.slug))[0]?.x_axis).toBe("date_time");

	await page.goBack();
	await expect(page.getByText("Unsaved changes")).toBeVisible();
	await tabRow(page).getByRole("link", { name: "Reports" }).click();
	await dialog.getByRole("button", { name: "Save and leave" }).click();
	await expect(page).toHaveURL(/\/reports$/);
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

// Kills a Clear that leaves a box, drops the window or the y scale, adds
// more than one history entry, or drops the focus, and a Back that does not
// draw the plot again.
test("Clear empties every box for blank Explore, keeps the view, and Back draws the plot again", async ({
	page,
	request,
}) => {
	const { query } = await twoLines(request);
	await page.goto(
		explore(hashbrown.slug, {
			...query,
			window: { seconds: 7 * 24 * 60 * 60 },
			yScale: "log",
		}),
	);
	await expect(plot(page)).toHaveAccessibleName("2 lines");
	const entries = await page.evaluate(() => history.length);
	const clear = page.getByRole("button", { name: "Clear", exact: true });

	await clear.click();
	await expect(
		box(page, "Branches").getByRole("button", { name: "Add branch" }),
	).toBeFocused();
	await expect(
		page.getByRole("region", { name: "Pinned plots" }),
	).toBeVisible();
	expect([...params(page)]).toEqual([
		["window", "1w"],
		["y_axis", "log"],
	]);
	expect(await page.evaluate(() => history.length)).toBe(entries + 1);
	await expect(clear).toHaveCount(0);

	await page.goBack();
	await expect(plot(page)).toHaveAccessibleName("2 lines");
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

// Kills a readout that follows the mouse past the next point on its way to
// the readout's link, so the link reached is another report's or none.
test("a mouse reaches a readout's report link past a point close beside it", async ({
	page,
	request,
}) => {
	const project = await createProject(request);
	const now = Date.parse(seed.now);
	const results = (value: number) => ({
		hash: [{ parameters: { size: 1 }, measures: { latency: { value } } }],
	});
	// Two reports an hour apart among three weeks: their points sit a few pixels apart.
	const first = await postReport(request, project.slug, {
		start: new Date(now - 21 * DAY).toISOString(),
		results: results(1),
	});
	await postReport(request, project.slug, {
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
	const link = plot(page).getByRole("status").getByRole("link");
	await expect(link).toHaveAttribute("href", new RegExp(first.uuid));
	const goal = await link.boundingBox();
	if (!goal) {
		throw new Error("No report link");
	}
	await page.mouse.move(goal.x + goal.width / 2, goal.y + goal.height / 2, {
		steps: 8,
	});
	await page.mouse.down();
	await page.mouse.up();
	await expect(page).toHaveURL(new RegExp(`/reports/${first.uuid}`));
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

	// Kills a Clear missing from a phone's query panel or its sheet, a target
	// under 44 px there, and a focus that falls with it.
	test("Clear is a 44 px target beside the chips and in the sheet", async ({
		page,
		request,
	}) => {
		const { query } = await twoLines(request);
		await page.goto(explore(hashbrown.slug, query));
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		const target = async (button: Locator) => {
			const size = await button.boundingBox();
			expect(
				Math.min(size?.width ?? 0, size?.height ?? 0),
			).toBeGreaterThanOrEqual(44);
		};

		await page.getByRole("button", { name: "branches main" }).click();
		const sheet = page.getByRole("dialog", { name: "Query" });
		const inSheet = sheet.getByRole("button", { name: "Clear", exact: true });
		await target(inSheet);
		await inSheet.click();
		await expect(
			sheet.getByRole("button", { name: "Add branch" }),
		).toBeFocused();
		await sheet.getByRole("button", { name: "Done" }).click();

		await page.goBack();
		await expect(plot(page)).toHaveAccessibleName("2 lines");
		const inPanel = page.getByRole("button", { name: "Clear", exact: true });
		await target(inPanel);
		await inPanel.click();
		await expect(
			page.getByRole("button", { name: /^branches / }),
		).toBeFocused();
	});
});

// Kills a contrast, name, or role failure on blank Explore's starting points
// in either theme.
test("blank Explore passes axe in both themes", async ({ page }) => {
	await axeInBothThemes(page, nextPath(hashbrown.slug), async () => {
		await expect(
			page
				.getByRole("region", { name: "Alerting now" })
				.getByRole("link", { name: /^blake3 / }),
		).toHaveCount(1);
		await expect(
			page.getByRole("region", { name: "Latest reports" }).getByRole("link"),
		).not.toHaveCount(0);
	});
});

// Kills a source line, from an alert or from a report, whose link reads only
// by its color or fails a contrast or a name in either theme.
test("Explore opened from an alert or a report passes axe in both themes", async ({
	page,
	request,
}) => {
	await axeInBothThemes(
		page,
		nextPath(hashbrown.slug),
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
