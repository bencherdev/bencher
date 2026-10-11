import type { Page } from "@playwright/test";
import { decodeQuery } from "../src/next/query/query";
import type { JsonConsolePerf } from "../src/types/bencher";
import { createProject, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { dimensions } from "./explore";
import {
	VERSION_KEY,
	crumbs,
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { DAY, publicPath, twoLinesDrawn, twoRecentLines } from "./public";

const { hashbrown, version_zero } = seed.projects;

// The lines these tests draw reported near the real time (see `twoRecentLines`).
test.use({ freezeClock: false });

const signInWayBack = (path: string) =>
	`/auth/login?${new URLSearchParams({ back: Buffer.from(path).toString("base64") })}`;

const isPerf = (url: string) =>
	/\/v0\/console\/projects\/[^/]+\/perf$/.test(new URL(url).pathname);

const perfRequests = (page: Page) => {
	const asked: URL[] = [];
	page.on("request", (request) => {
		const url = new URL(request.url());
		if (/\/v0\/console\/projects\/[^/]+\/perf$/.test(url.pathname)) {
			asked.push(url);
		}
	});
	return asked;
};

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a public page that draws nothing without a sign in, offers an
	// edit, a pin, a tab, or a window it cannot draw, or names a member's
	// action to a reader who is not one.
	test("the public plot draws its query, read only, with nothing that changes the project", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const changes: string[] = [];
		page.on("request", (sent) => {
			if (sent.url().startsWith(seed.api_url) && sent.method() !== "GET") {
				changes.push(`${sent.method()} ${sent.url()}`);
			}
		});
		await page.goto(publicPath(project.slug, search));
		await twoLinesDrawn(page);
		await expect(
			page.getByRole("heading", { level: 1, name: project.name }),
		).toBeVisible();
		expect(
			JSON.parse(
				(await page.evaluate(
					(key) => localStorage.getItem(key),
					VERSION_KEY,
				)) ?? "{}",
			),
		).toMatchObject({ [project.slug]: 1 });

		const query = page.getByRole("region", { name: "Query, read only" });
		await expect(query.getByText("hash", { exact: true })).toBeVisible();
		await query.getByRole("button", { name: "Show query" }).click();
		for (const [box, value] of [
			["Branches", "main"],
			["Testbeds", "ubuntu-latest"],
			["Benchmarks", "hash"],
			["Parameters", "every variant"],
			["Measures", "Latency"],
			["Metrics", "every metric"],
		] as const) {
			await expect(
				query.getByRole("group", { name: box }).getByText(value),
			).toBeVisible();
		}
		await expect(
			page.getByRole("button", { name: /^(Add|Remove)/ }),
		).toHaveCount(0);
		expect(
			await page
				.getByRole("radiogroup", { name: "Window" })
				.getByRole("radio")
				.evaluateAll((radios) =>
					radios.map((radio) => radio.parentElement?.textContent),
				),
		).toEqual(["1w", "4w", "3m"]);
		for (const absent of ["Pin to Plots", "Save", "Save as new", "Discard"]) {
			await expect(page.getByRole("button", { name: absent })).toHaveCount(0);
		}
		await expect(page.getByRole("navigation", { name: "Project" })).toHaveCount(
			0,
		);
		await expect(page.getByRole("link", { name: /^Alerts/ })).toHaveCount(0);
		await expect(
			page.getByRole("link", { name: "Open in console" }),
		).toHaveCount(0);
		await expect(
			page.getByRole("banner").getByRole("link", { name: "Log in" }),
		).toHaveAttribute(
			"href",
			signInWayBack(new URL(page.url()).pathname + new URL(page.url()).search),
		);

		// The view's controls redraw in place and travel in the link, never in history.
		const entries = await page.evaluate(() => history.length);
		const asked = perfRequests(page);
		await page
			.getByRole("radiogroup", { name: "X axis" })
			.getByText("Version")
			.click();
		await expect(page).toHaveURL(/x_axis=version/);
		const windowAsked = page.waitForRequest((sent) =>
			/\/v0\/console\/projects\/[^/]+\/perf/.test(sent.url()),
		);
		await page
			.getByRole("radiogroup", { name: "Window" })
			.getByText("1w")
			.click();
		await windowAsked;
		await expect(page).toHaveURL(/window=1w/);
		await twoLinesDrawn(page);
		// Back to a window already drawn draws it again without asking.
		await page
			.getByRole("radiogroup", { name: "Window" })
			.getByText("4w")
			.click();
		await expect(page).not.toHaveURL(/window=/);
		await twoLinesDrawn(page);
		await settle(page);
		expect(asked).toHaveLength(1);
		expect(changes).toEqual([]);
		expect(await page.evaluate(() => history.length)).toBe(entries);

		await page
			.context()
			.grantPermissions(["clipboard-read", "clipboard-write"]);
		await page.getByRole("button", { name: "Share" }).click();
		await expect(
			page.getByRole("button", { name: "Link copied" }),
		).toBeVisible();
		expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
			page.url(),
		);
	});

	// Kills an old pull request link drawn short of its own window, and a public
	// Window that keeps the link's end, which never reaches the present.
	test("an old pull request link draws its whole window, and a preset reaches the present", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const reported = (daysAgo: number) =>
			postReport(request, project.slug, {
				start: new Date(Date.now() - daysAgo * DAY).toISOString(),
				results: {
					hash: [
						{
							parameters: { size: 1 },
							measures: { latency: { value: daysAgo } },
						},
					],
				},
			});
		await reported(95);
		const linked = await reported(70);
		await reported(1);
		const id = await dimensions(request, project.slug);
		// As `bencher run` writes it into a pull request comment.
		const link = new URLSearchParams({
			report: linked.uuid,
			branches: linked.branch.uuid,
			heads: linked.branch.head.uuid,
			testbeds: linked.testbed.uuid,
			benchmarks: id.benchmark("hash"),
			measures: id.measure("Latency"),
			start_time: String(Date.parse(linked.start_time) - 28 * DAY),
			end_time: String(Date.parse(linked.end_time)),
		});
		const opened = page.waitForResponse((response) => isPerf(response.url()));
		await page.goto(`/perf/${project.slug}?${link}`);
		const whole: JsonConsolePerf = await (await opened).json();
		expect(whole.window.clamped).toBe(false);
		expect(whole.points.x).toHaveLength(2);
		await expect(page.getByRole("figure")).toHaveAccessibleName("1 line");
		const windows = page.getByRole("radiogroup", { name: "Window" });
		await expect(windows.getByRole("radio", { checked: true })).toHaveCount(0);

		const now = Date.now();
		const present = page.waitForResponse((response) => isPerf(response.url()));
		await windows.getByText("4w").click();
		const answered = await present;
		const end = Number(
			new URL(answered.request().url()).searchParams.get("end_time"),
		);
		expect(Math.abs(end - now)).toBeLessThan(60_000);
		const recent: JsonConsolePerf = await answered.json();
		expect(recent.points.x).toHaveLength(1);
		expect(new URL(page.url()).searchParams.has("end_time")).toBe(false);
		await expect(windows.getByRole("radio", { name: "4w" })).toBeChecked();
	});

	// Kills an answer to an earlier window drawn over the window picked after it.
	test("an answer that arrives after a newer one is not drawn", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		// A third line that reported in the last four weeks, but not the last week.
		await postReport(request, project.slug, {
			start: new Date(Date.now() - 10 * DAY).toISOString(),
			results: {
				hash: [
					{ parameters: { size: 3 }, measures: { latency: { value: 3 } } },
				],
			},
		});
		let release = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		let first = true;
		await page.route(
			(url) => isPerf(url.href),
			async (route) => {
				if (first) {
					first = false;
					await held;
				}
				await route.continue();
			},
		);
		await page.goto(publicPath(project.slug, search));
		await page
			.getByRole("radiogroup", { name: "Window" })
			.getByText("1w")
			.click();
		await twoLinesDrawn(page);
		const late = page.waitForResponse((response) => isPerf(response.url()));
		release();
		await late;
		await settle(page);
		await twoLinesDrawn(page);
		await page
			.getByRole("radiogroup", { name: "Window" })
			.getByText("4w")
			.click();
		await expect(page.getByRole("figure")).toHaveAccessibleName("3 lines");
	});

	// Kills a link's own range rewritten to a preset before it is asked for,
	// and one past three months the API cut short with nothing on the page
	// saying so.
	test("a 120-day link asks for its own range and draws its last 92 days, saying so", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const end = Date.now();
		const start = end - 120 * DAY;
		const range = `${search}&start_time=${start}&end_time=${end}`;
		const opened = page.waitForResponse((response) => isPerf(response.url()));
		await page.goto(publicPath(project.slug, range));
		const answer = await opened;
		const asked = new URL(answer.request().url()).searchParams;
		expect(Number(asked.get("start_time"))).toBe(start);
		expect(Number(asked.get("end_time"))).toBe(end);
		const { window }: JsonConsolePerf = await answer.json();
		expect(window.clamped).toBe(true);
		// The API starts a window on a whole second.
		expect(Math.floor((window.end_time - window.start_time) / 1000)).toBe(
			(92 * DAY) / 1000,
		);
		await twoLinesDrawn(page);
		// The note takes the place of the window's reach, so nothing moves.
		const reach = page.locator(".muted.sm").filter({ hasText: /3 months/ });
		await expect(reach).toHaveText(/^cut to its last 3 months/);
		await expect(
			page
				.getByRole("radiogroup", { name: "Window" })
				.getByRole("radio", { checked: true }),
		).toHaveCount(0);
		expect(decodeQuery(new URL(page.url()).search).window).toEqual({
			start,
			end,
		});
	});

	// Kills a classic perf link left on the classic page for a version 1
	// project, or one that arrives without its lines or with the classic
	// page's own settings.
	test("a classic perf link of a version 1 project lands on the public plot with its query", async ({
		page,
		request,
	}) => {
		const id = await dimensions(request, hashbrown.slug);
		const branch = id.branch("main");
		const measure = id.measure("Latency");
		await page.goto(
			`/perf/${hashbrown.slug}?branches=${branch}&measures=${measure}&tab=benchmarks&key=true&x_axis=version`,
		);
		await expect(page).toHaveURL(
			publicPath(
				hashbrown.slug,
				`?branches=${branch}&measures=${measure}&x_axis=version`,
			),
		);
		await expect(
			page.getByRole("heading", { level: 1, name: hashbrown.name }),
		).toBeVisible();
	});

	// Kills a public plot that keeps a version 0 project, which it cannot draw.
	test("a version 0 project's public plot goes back to its perf page", async ({
		page,
	}) => {
		await page.goto(publicPath(version_zero.slug, "?x_axis=version"));
		await expect(page).toHaveURL(
			(url) =>
				url.pathname === `/perf/${version_zero.slug}` &&
				url.searchParams.get("x_axis") === "version",
		);
	});

	// Kills a private project drawn for nobody, a page that asks the API what
	// it already refused, and a sign in that forgets the way back.
	test("a private project asks the reader to log in, through a link that keeps the page", async ({
		page,
	}) => {
		const asked: string[] = [];
		page.on("request", (sent) => {
			if (sent.url().startsWith(seed.api_url)) {
				asked.push(sent.url());
			}
		});
		const path = publicPath(seed.projects.private.slug, "?x_axis=version");
		await page.goto(path);
		await expect(
			page.getByRole("heading", { level: 1, name: "Log in to see this page" }),
		).toBeVisible();
		await expect(
			page.getByRole("main").getByRole("link", { name: "Log in" }),
		).toHaveAttribute("href", signInWayBack(path));
		await expect(crumbs(page).getByRole("link")).toHaveCount(0);
		await page.waitForLoadState("networkidle");
		expect(asked).toEqual([]);
	});

	test("the public plot passes axe in both themes", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		await axeInBothThemes(page, publicPath(project.slug, search), () =>
			twoLinesDrawn(page),
		);
	});
});

test.describe("with the version memory", () => {
	const { slug } = seed.projects.private;
	test.use({
		storageState: {
			cookies: [],
			origins: [
				{
					origin: seed.console_url,
					localStorage: [
						{ name: VERSION_KEY, value: JSON.stringify({ [slug]: 1 }) },
					],
				},
			],
		},
	});

	// Kills a classic perf page the server cannot read that stays put for a
	// project the reader has seen on version 1.
	test("a classic perf link the server cannot read follows the version memory", async ({
		page,
	}) => {
		await page.goto(`/perf/${slug}?x_axis=version&tab=plots`);
		await expect(page).toHaveURL(publicPath(slug, "?x_axis=version"));
	});
});

test.describe("a signed in member", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills Open in console missing for a member, or opening Explore on
	// another query than the one drawn.
	test("Open in console opens Explore with the same lines", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const asked = page.waitForRequest((sent) => isPerf(sent.url()));
		await page.goto(publicPath(project.slug, `${search}&x_axis=version`));
		await twoLinesDrawn(page);
		// A public project's plot is the same for every reader.
		expect((await asked).headers().authorization).toBeUndefined();
		await expect(page.getByRole("link", { name: "Log in" })).toHaveCount(0);
		await page.getByRole("link", { name: "Open in console" }).click();
		await expect(page).toHaveURL(
			(url) =>
				url.pathname === nextPath(project.slug, "explore") &&
				url.searchParams.get("x_axis") === "version",
		);
		await twoLinesDrawn(page);
	});

	// Kills a private project's public plot refused to a reader who may read it,
	// or painted locked before the reader's own read answers.
	test("a private project's public plot opens for its member", async ({
		page,
	}) => {
		const { private: locked } = seed.projects;
		let release = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route(
			(url) =>
				url.pathname === `/v0/projects/${locked.slug}` ||
				(url.origin === new URL(seed.console_url).origin &&
					url.pathname.endsWith(".js")),
			async (route) => {
				await held;
				await route.continue();
			},
		);
		// The server's page, before any script: the head marks the reader.
		await page.goto(publicPath(locked.slug), { waitUntil: "commit" });
		await expect(page.locator('main[aria-busy="true"]')).toBeVisible();
		await expect(page.getByRole("heading", { level: 1 })).toHaveCount(0);
		release();
		await expect(
			page.getByRole("heading", { level: 1, name: locked.name }),
		).toBeVisible();
		await expect(
			page.getByRole("link", { name: "Open in console" }),
		).toBeVisible();
	});
});

test.describe("a signed in member of a version 0 project", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a private version 0 project's public plot sent to the classic public
	// page, which cannot show it, rather than to its console.
	test("a private version 0 project goes to its console perf page", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		for (const [token, data] of [
			[seed.member.token, { visibility: "private" }],
			[seed.admin.token, { bmf_version: 0 }],
		] as const) {
			const response = await request.patch(
				`${seed.api_url}/v0/projects/${project.slug}`,
				{ headers: { Authorization: `Bearer ${token}` }, data },
			);
			expect(response.ok(), await response.text()).toBe(true);
		}
		await page.goto(publicPath(project.slug));
		await expect(page).toHaveURL(
			(url) => url.pathname === `/console/projects/${project.slug}/perf`,
		);
	});
});

test.describe("a signed in reader who is not a member", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills a reader the project refuses who is shown a plot, or told to log in
	// or sign up while signed in.
	test("a private project tells them it is private or missing", async ({
		page,
	}) => {
		await page.goto(publicPath(seed.projects.private.slug));
		await expect(
			page.getByRole("heading", {
				level: 1,
				name: "This project is private, or it does not exist",
			}),
		).toBeVisible();
		for (const name of ["Log in", "Sign up"]) {
			await expect(page.getByRole("link", { name })).toHaveCount(0);
		}
		await expect(page.getByRole("figure")).toHaveCount(0);
	});
});

test.describe("a reader whose session ended", () => {
	test.use({
		storageState: signedIn({
			...seed.member,
			expiration: new Date(Date.now() - DAY).toISOString(),
		}),
	});

	// Kills an ended session taken for a reader, which hides Log in for good.
	test("is asked to log in, with no Open in console", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		await page.goto(publicPath(project.slug, search));
		await twoLinesDrawn(page);
		await expect(
			page.getByRole("banner").getByRole("link", { name: "Log in" }),
		).toBeVisible();
		await expect(
			page.getByRole("link", { name: "Open in console" }),
		).toHaveCount(0);
	});
});

test.describe("narrow", () => {
	test.use({ storageState: signedOut, viewport: { width: 390, height: 844 } });

	// Kills a narrow query that cannot be read or offers an edit, and touch
	// targets under 44 px.
	test("the query shows read only, and its controls are 44 px", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		await page.goto(publicPath(project.slug, search));
		await twoLinesDrawn(page);
		const show = page.getByRole("button", { name: "Show query" });
		for (const target of [
			show,
			page.getByRole("banner").getByRole("link", { name: "Log in" }),
			page.getByRole("radiogroup", { name: "Window" }).getByText("1w"),
		]) {
			expect((await target.boundingBox())?.height).toBeGreaterThanOrEqual(44);
		}
		await show.click();
		await expect(
			page.getByRole("group", { name: "Benchmarks" }).getByText("hash"),
		).toBeVisible();
		await expect(
			page.getByRole("button", { name: /^(Add|Remove)/ }),
		).toHaveCount(0);
		expect(
			await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth,
			),
		).toBe(true);
	});

	// Kills a custom window's From and To held on one line past the edge of a
	// phone, which scrolls the page sideways.
	test("a custom window scrolls nothing sideways", async ({
		page,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const end = Date.now();
		await page.goto(
			publicPath(
				project.slug,
				`${search}&start_time=${end - 30 * DAY}&end_time=${end}`,
			),
		);
		await twoLinesDrawn(page);
		const range = page.getByRole("group", { name: "Custom window" });
		await expect(range.getByLabel("To")).toBeVisible();
		expect(
			await page.evaluate(
				() => document.documentElement.scrollWidth <= window.innerWidth,
			),
		).toBe(true);
		const box = await range.boundingBox();
		expect((box?.x ?? 0) + (box?.width ?? 0)).toBeLessThanOrEqual(390);
	});
});
