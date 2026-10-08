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
