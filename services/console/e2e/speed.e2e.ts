import type { Page } from "@playwright/test";
import {
	crumbs,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	tabRow,
	test,
} from "./fixtures";
import { explore, twoLines } from "./explore";
import ceilings from "./speed-ceilings.json" with { type: "json" };
import {
	type Cost,
	loadedAndIdle,
	mark,
	measure,
	observeLayoutShift,
} from "./speed";

const { hashbrown } = seed.projects;
// The shell alone: a project path no page draws, so no page data rides along.
const SHELL = nextPath(hashbrown.slug, "nowhere");
const REPORTS = nextPath(hashbrown.slug, "reports");

test.use({ storageState: signedIn(seed.member), freezeClock: false });

const report = (name: string, cost: Cost) => {
	test
		.info()
		.annotations.push({ type: name, description: JSON.stringify(cost) });
	console.log(`${name}: ${JSON.stringify(cost)}`);
};

/** The page is up, its deferred work has started, and the network is quiet. */
const ready = async (page: Page) => {
	await expect(
		tabRow(page).getByRole("link", { name: "Alerts 1 active" }),
	).toBeVisible();
	await loadedAndIdle(page);
	await settle(page);
};

// Kills JavaScript, document, and inline script growth, modules found only after
// their parent arrives, API requests that block the first paint or chain into a
// second round, and layout that shifts as data arrives.
test("a cold load holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	// An API slower than the first paint, so data never seen always arrives
	// after it and the shift it causes is the same on every run.
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(SHELL);
	await expect(
		crumbs(page).getByRole("link", { name: seed.organization.name }),
	).toBeVisible();
	await ready(page);

	const cost = await measure(page, seed.api_url);
	report("cold load", cost);
	const ceiling = ceilings.coldLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.deferredJsBytes).toBeLessThanOrEqual(ceiling.deferredJsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a returning reader's first paint that differs from what the app
// draws over it, and a reload that waits on the cache to ask the API.
test("a warm reload holds the speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	await page.goto(SHELL);
	await ready(page);
	await settle(page, 1_500);

	await page.reload();
	await ready(page);

	const cost = await measure(page, seed.api_url);
	report("warm reload", cost);
	const ceiling = ceilings.warmReload;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a tab switch that refetches the shell or pulls in more than its page.
test("a tab switch holds the speed ceilings", async ({ page }) => {
	await page.goto(SHELL);
	await ready(page);

	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Thresholds" }).click();
	await expect(
		page.getByRole("heading", { level: 1, name: "Thresholds" }),
	).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("tab switch", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceilings.tabSwitch.apiRequests);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceilings.tabSwitch.jsBytes);
});

const reportsReady = async (page: Page) => {
	await expect(
		page
			.getByRole("table", { name: "Reports, newest first" })
			.getByRole("link")
			.first(),
	).toBeVisible();
	await ready(page);
};

// Kills a Reports page whose list waits on the shell's request or on its own
// code arriving late, JavaScript that grows, and rows that move as they arrive.
test("a cold load of Reports holds its speed ceilings", async ({ page }) => {
	await observeLayoutShift(page);
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(REPORTS);
	await reportsReady(page);

	const cost = await measure(page, seed.api_url);
	report("reports cold load", cost);
	const ceiling = ceilings.reportsColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
	expect(cost.inlineScriptBytes).toBeLessThanOrEqual(ceiling.inlineScriptBytes);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a switch into Reports that refetches the shell, asks for the list in
// more than one request, or pulls in more than its page.
test("a tab switch into Reports holds its speed ceilings", async ({ page }) => {
	await page.goto(SHELL);
	await ready(page);

	const since = await mark(page);
	await tabRow(page).getByRole("link", { name: "Reports" }).click();
	await reportsReady(page);

	const cost = await measure(page, seed.api_url, since);
	report("reports tab switch", cost);
	const ceiling = ceilings.reportsTabSwitch;
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
});

/** Load `path` cold, the API slower than the first paint, and measure it once `shown` resolves. */
const coldLoad = async (
	page: Page,
	path: string,
	shown: () => Promise<void>,
): Promise<Cost> => {
	await observeLayoutShift(page);
	await page.route(`${seed.api_url}/**`, async (route) => {
		await new Promise((resolve) => setTimeout(resolve, 500));
		await route.continue();
	});
	await page.goto(path);
	await shown();
	await ready(page);
	return measure(page, seed.api_url);
};

// Kills a General section that asks the API for what the shell's request
// already holds, and one that shifts as the project arrives.
test("a cold load of General holds the speed ceilings", async ({ page }) => {
	const cost = await coldLoad(page, nextPath(hashbrown.slug, "settings"), () =>
		expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			hashbrown.name,
		),
	);
	report("settings cold load", cost);
	const ceiling = ceilings.settingsColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills key lists that wait on the shell's request, a third request for the
// other status, and a table that shifts as the keys arrive.
test("a cold load of Keys holds the speed ceilings", async ({ page }) => {
	const cost = await coldLoad(
		page,
		nextPath(hashbrown.slug, "settings/keys"),
		() =>
			expect(page.getByRole("table", { name: "Active keys" })).toBeVisible(),
	);
	report("keys cold load", cost);
	const ceiling = ceilings.keysColdLoad;
	expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
		ceiling.apiAnswersBeforePaint,
	);
	expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
	expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
	expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
	expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
});

// Kills a section switch that loads more code or refetches the shell.
test("moving from General to Keys holds the speed ceilings", async ({
	page,
}) => {
	await page.goto(nextPath(hashbrown.slug, "settings"));
	await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
		hashbrown.name,
	);
	await ready(page);

	const since = await mark(page);
	await page
		.getByRole("navigation", { name: "Settings", exact: true })
		.getByRole("link", { name: "Keys" })
		.click();
	await expect(page.getByRole("table", { name: "Active keys" })).toBeVisible();
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("settings section switch", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.apiRequests,
	);
	expect(cost.apiRounds).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.apiRounds,
	);
	expect(cost.jsBytes).toBeLessThanOrEqual(
		ceilings.settingsSectionSwitch.jsBytes,
	);
});

const twoLinesShown = (page: Page) =>
	expect(page.getByRole("figure")).toHaveAccessibleName("2 lines");

/**
 * Layout shift with a source outside the shell: what the page itself moves.
 * On a cold load the shell's own alert badge and names arrive after its first
 * paint, and at narrow widths that alone passes the ceiling.
 */
const observePageShift = (page: Page) =>
	page.addInitScript(() => {
		const holder = window as Window & { __pageShift?: number };
		holder.__pageShift = 0;
		new PerformanceObserver((list) => {
			for (const entry of list.getEntries()) {
				const shift = entry as PerformanceEntry & {
					value: number;
					hadRecentInput: boolean;
					sources?: { node?: Node | null }[];
				};
				const outside = (shift.sources ?? []).some(
					({ node }) => !(node instanceof Element && node.closest(".shell")),
				);
				if (!shift.hadRecentInput && outside) {
					holder.__pageShift = (holder.__pageShift ?? 0) + shift.value;
				}
			}
		}).observe({ type: "layout-shift", buffered: true });
	});

const pageShift = (page: Page) =>
	page.evaluate(
		() => (window as Window & { __pageShift?: number }).__pageShift ?? 0,
	);

// The cold loads of a report, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills a report whose lines wait on the shell's request or on its own
		// code arriving late, a first batch asked for twice, the full plot's code
		// loaded before a row expands, a page stylesheet of its own, and a page
		// that shifts as the report arrives.
		test(`a cold load of a report holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const response = await request.get(
				`${seed.api_url}/v0/projects/${hashbrown.slug}/reports?branch=main&testbed=ubuntu-latest&per_page=1`,
				{ headers: { Authorization: `Bearer ${seed.member.token}` } },
			);
			const [newest] = (await response.json()) as { uuid: string }[];
			await observePageShift(page);
			const cost = await coldLoad(page, `${REPORTS}/${newest?.uuid}`, () =>
				expect(
					page
						.getByRole("table", { name: /^Lines in report / })
						.getByRole("checkbox")
						.first(),
				).toBeVisible(),
			);
			const moved = await pageShift(page);
			report(`report cold load ${width}`, {
				...cost,
				pageShift: moved,
			} as Cost);
			const ceiling = ceilings.reportColdLoad;
			expect(moved).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			if (!viewport) {
				expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			}
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.stylesheets).toBeLessThanOrEqual(ceiling.stylesheets);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});
	});
}

// The cold loads that draw Explore, at each width the layout changes at.
for (const [width, viewport] of [
	["desktop", undefined],
	["768", { width: 768, height: 1024 }],
	["390", { width: 390, height: 844 }],
] as const) {
	test.describe(`at ${width}`, () => {
		if (viewport) {
			test.use({ viewport });
		}

		// Kills an Explore link whose plot query waits on the shell's request, a
		// second data request, code found late, a page stylesheet of its own, and
		// a page that shifts as the answer arrives.
		test(`a cold Explore link holds its speed ceilings at ${width}`, async ({
			page,
			request,
		}) => {
			const link = explore(hashbrown.slug, (await twoLines(request)).query);
			await observePageShift(page);
			const cost = await coldLoad(page, link, () => twoLinesShown(page));
			const moved = await pageShift(page);
			report(`explore cold load ${width}`, {
				...cost,
				pageShift: moved,
			} as Cost);
			const ceiling = ceilings.exploreColdLoad;
			expect(moved).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			if (!viewport) {
				expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			}
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.htmlBytes).toBeLessThanOrEqual(ceiling.htmlBytes);
			expect(cost.inlineScriptBytes).toBeLessThanOrEqual(
				ceiling.inlineScriptBytes,
			);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
			const stylesheets = await page.evaluate(
				() =>
					performance
						.getEntriesByType("resource")
						.filter((entry) => new URL(entry.name).pathname.endsWith(".css"))
						.length,
			);
			expect(stylesheets).toBe(1);
		});

		// Kills starting points asked for one after another or after the shell's
		// request, and lists that shift as they arrive.
		test(`a cold blank Explore holds its speed ceilings at ${width}`, async ({
			page,
		}) => {
			await observePageShift(page);
			const cost = await coldLoad(page, nextPath(hashbrown.slug), () =>
				expect(
					page
						.getByRole("region", { name: "Alerting now" })
						.getByRole("link", { name: /open in Explore$/ }),
				).toBeVisible(),
			);
			const moved = await pageShift(page);
			report(`blank explore cold load ${width}`, {
				...cost,
				pageShift: moved,
			} as Cost);
			const ceiling = ceilings.blankExploreColdLoad;
			expect(moved).toBeLessThanOrEqual(ceiling.cls);
			expect(cost.apiAnswersBeforePaint).toBeLessThanOrEqual(
				ceiling.apiAnswersBeforePaint,
			);
			expect(cost.apiRequests).toBeLessThanOrEqual(ceiling.apiRequests);
			expect(cost.apiRounds).toBeLessThanOrEqual(ceiling.apiRounds);
			if (!viewport) {
				expect(cost.cls).toBeLessThanOrEqual(ceiling.cls);
			}
			expect(cost.jsBytes).toBeLessThanOrEqual(ceiling.jsBytes);
			expect(cost.lateModules).toBeLessThanOrEqual(ceiling.lateModules);
		});
	});
}

// Kills a link whose plot code is found only once the page's own code runs, a
// round later, blank Explore loading the plot it does not draw, and a reader
// reaching for a benchmark who waits on the plot's code after the pick.
test("an Explore link names its plot's code in its document; blank Explore loads it only once a benchmark is near", async ({
	browser,
	page,
	request,
}) => {
	const scripts = (target: Page) =>
		target.evaluate(() =>
			performance
				.getEntriesByType("resource")
				.map((entry) => new URL(entry.name))
				.filter(
					(url) =>
						url.origin === location.origin && url.pathname.endsWith(".js"),
				)
				.map((url) => url.pathname),
		);
	await page.goto(nextPath(hashbrown.slug));
	await expect(
		page
			.getByRole("region", { name: "Alerting now" })
			.getByRole("link", { name: /open in Explore$/ }),
	).toBeVisible();
	await ready(page);
	const blank = new Set(await scripts(page));

	const context = await browser.newContext({
		storageState: signedIn(seed.member),
	});
	const linked = await context.newPage();
	const link = explore(hashbrown.slug, (await twoLines(request)).query);
	await linked.goto(link);
	await twoLinesShown(linked);
	await ready(linked);
	const served = await (await linked.request.get(linked.url())).text();
	const named = new Set(
		[...served.matchAll(/<link\b[^>]*\bhref="([^"]+\.js)"/g)].map(
			([, href]) => new URL(href ?? "", linked.url()).pathname,
		),
	);
	const plot = (await scripts(linked)).filter((path) => !blank.has(path));
	expect(plot.length).toBeGreaterThan(0);
	expect(plot.filter((path) => !named.has(path))).toEqual([]);
	await context.close();

	await page
		.getByRole("group", { name: "Benchmarks" })
		.getByRole("button", { name: "Add benchmark" })
		.click();
	await expect
		.poll(async () => {
			const loaded = new Set(await scripts(page));
			return plot.filter((path) => !loaded.has(path));
		})
		.toEqual([]);
});

// Kills a box edit that asks the API more than once or loads more code.
test("an Explore box edit makes one request", async ({ page, request }) => {
	await page.goto(explore(hashbrown.slug, (await twoLines(request)).query));
	await twoLinesShown(page);
	await ready(page);

	const since = await mark(page);
	await page
		.getByRole("group", { name: "Parameters" })
		.getByRole("button", { name: "Remove set input_bytes=65536 threads=1" })
		.click();
	await expect(page.getByRole("figure")).toHaveAccessibleName("8 lines");
	await settle(page);

	const cost = await measure(page, seed.api_url, since);
	report("explore box edit", cost);
	expect(cost.apiRequests).toBeLessThanOrEqual(
		ceilings.exploreBoxEdit.apiRequests,
	);
	expect(cost.apiRounds).toBeLessThanOrEqual(ceilings.exploreBoxEdit.apiRounds);
	expect(cost.jsBytes).toBeLessThanOrEqual(ceilings.exploreBoxEdit.jsBytes);
});

// Kills a key toggle that refetches, rebuilds the plot, or does more than one
// 120 Hz frame of work through the link it writes.
test("an Explore key toggle fits in a frame", async ({ page, request }) => {
	await page.goto(explore(hashbrown.slug, (await twoLines(request)).query));
	await twoLinesShown(page);
	await ready(page);

	const since = await mark(page);
	const times = await page.evaluate(async () => {
		const toggle = /^(Hide|Show) blake3 simd=sse4\.2/;
		const button = () =>
			[...document.querySelectorAll("button")].find((each) =>
				toggle.test(each.getAttribute("aria-label") ?? ""),
			);
		// A message is a task of its own that no timer clamp delays, so a wait
		// for the redraw measures the work and not the browser's timers.
		const nextTask = () =>
			new Promise<void>((resolve) => {
				const channel = new MessageChannel();
				channel.port1.onmessage = () => resolve();
				channel.port2.postMessage(null);
			});
		const nextFrame = () =>
			new Promise<void>((resolve) =>
				requestAnimationFrame(() => requestAnimationFrame(() => resolve())),
			);
		const canvas = document.querySelector("figure canvas");
		const spent: number[] = [];
		for (let round = 0; round < 9; round++) {
			const before = button()?.getAttribute("aria-label");
			const start = performance.now();
			button()?.click();
			while (button()?.getAttribute("aria-label") === before) {
				await nextTask();
			}
			spent.push(performance.now() - start);
			await nextFrame();
		}
		return {
			times: spent.sort((a, b) => a - b),
			kept: canvas?.isConnected === true,
		};
	});
	const median =
		times.times[Math.floor(times.times.length / 2)] ?? Number.POSITIVE_INFINITY;
	report("explore key toggle", { median, ...times } as unknown as Cost);
	// The plot redraws in place rather than building itself again.
	expect(times.kept).toBe(true);
	expect(median).toBeLessThanOrEqual(ceilings.exploreKeyToggle.ms);
	const cost = await measure(page, seed.api_url, since);
	expect(cost.apiRequests).toBe(0);
});
