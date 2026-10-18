import type { Page, Request } from "@playwright/test";
import { embedHeight } from "../src/next/embed/code";
import { blankQuery, decodeQuery, encodeQuery } from "../src/next/query/query";
import { createProject, postReport } from "./api";
import { axeInBothThemes } from "./axe";
import { dimensions, explore } from "./explore";
import { expect, seed, settle, signedIn, signedOut, test } from "./fixtures";
import { createPin } from "./plots";
import {
	DAY,
	embedLink,
	publicPath,
	twoLinesDrawn,
	twoRecentLines,
} from "./public";

// The lines these tests draw reported near the real time (see `twoRecentLines`).
test.use({ freezeClock: false });

const apiRequests = (page: Page) => {
	const asked: Request[] = [];
	page.on("request", (sent) => {
		if (sent.url().startsWith(seed.api_url)) {
			asked.push(sent);
		}
	});
	return asked;
};

const sent = (asked: Request[]) =>
	asked.map((request) => ({
		route: `${request.method()} ${new URL(request.url()).pathname}`,
		authorization: request.headers().authorization,
	}));

const linkedQuery = async (page: Page, name: string) => {
	const href = new URL(
		(await page
			.getByRole("link", { name, exact: true })
			.getAttribute("href")) ?? "",
		page.url(),
	);
	return { path: href.pathname, query: decodeQuery(href.search) };
};

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills an embed that draws the bar, the query panel, or an action, reads
	// the API as anything but no one or more than once a resource, and a line
	// whose links open another page or query, or open it in the frame.
	test("the embed draws a public project's plot and key under a line that opens its public plot", async ({
		page,
		context,
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const asked = apiRequests(page);
		await page.goto(embedLink(project.slug, search));
		await twoLinesDrawn(page);
		await expect(page).toHaveTitle(`${project.name} | Bencher`);
		for (const name of [project.name, "Bencher"]) {
			await expect(
				page.getByRole("link", { name, exact: true }),
			).toHaveAttribute("target", "_blank");
			expect(await linkedQuery(page, name)).toEqual({
				path: publicPath(project.slug),
				query: decodeQuery(search),
			});
		}
		await expect(page.getByRole("banner")).toHaveCount(0);
		await expect(page.getByRole("region", { name: /Query/ })).toHaveCount(0);
		for (const absent of ["Share", "Feedback", "Show query", "Retry"]) {
			await expect(page.getByRole("button", { name: absent })).toHaveCount(0);
		}
		await settle(page);
		expect(sent(asked).sort((a, b) => a.route.localeCompare(b.route))).toEqual([
			{
				route: `GET /v0/console/projects/${project.slug}/perf`,
				authorization: undefined,
			},
			{ route: `GET /v0/projects/${project.slug}`, authorization: undefined },
		]);

		const opened = context.waitForEvent("page");
		await page.getByRole("link", { name: project.name, exact: true }).click();
		const plot = await opened;
		await expect(plot).toHaveURL(
			(url) =>
				url.pathname === publicPath(project.slug) &&
				encodeQuery(decodeQuery(url.search)) ===
					encodeQuery(decodeQuery(search)),
		);
		await twoLinesDrawn(plot);
		expect(page.url()).toContain(embedLink(project.slug));
	});

	// Kills a server that reads the project before it answers, which names the
	// project in the document or refuses one that does not exist, and an embed
	// that search engines take for a page of its own.
	test("the server sends the embed without reading the API, naming the public plot canonical", async ({
		request,
	}) => {
		const { project, search } = await twoRecentLines(request);
		const served = await request.get(
			`${seed.console_url}${embedLink(project.slug, search)}`,
			{ maxRedirects: 0 },
		);
		expect(served.status()).toBe(200);
		const html = await served.text();
		expect(html).not.toContain(project.name);
		expect(html).toContain('aria-label="Loading the plot"');
		expect(/<link rel="canonical" href="([^"]*)"/.exec(html)?.[1]).toBe(
			`https://bencher.dev${publicPath(project.slug)}`,
		);
		const missing = await request.get(
			`${seed.console_url}${embedLink("no-such-project", search)}`,
			{ maxRedirects: 0 },
		);
		expect(missing.status()).toBe(200);
	});

	// Kills an embed that sends a version 0 project away or cannot draw it.
	test("a version 0 project's embed draws its plot", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const patched = await request.patch(
			`${seed.api_url}/v0/projects/${project.slug}`,
			{
				headers: { Authorization: `Bearer ${seed.admin.token}` },
				data: { bmf_version: 0 },
			},
		);
		expect(patched.ok(), await patched.text()).toBe(true);
		await postReport(request, project.slug, {
			start: new Date(Date.now() - DAY).toISOString(),
			results: { hash: { latency: { value: 1 } } },
		});
		const id = await dimensions(request, project.slug);
		const search = `?${new URLSearchParams({
			branches: id.branch("main"),
			testbeds: id.testbed("ubuntu-latest"),
			benchmarks: id.benchmark("hash"),
			measures: id.measure("Latency"),
		})}`;
		await page.goto(embedLink(project.slug, search));
		await expect(page.getByRole("figure")).toHaveAccessibleName("1 line");
		await expect(page).toHaveURL(embedLink(project.slug, search));
		await expect(
			page.getByRole("link", { name: project.name, exact: true }),
		).toBeVisible();
	});

	// Kills a snippet whose height cuts off a full key or a stacked panel, or
	// scrolls sideways, for any count of measures and either layout of two,
	// at a phone's, a tablet's, or a desktop's width.
	test("the snippet's height holds the plot and a full key for every count of measures", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const measured = (count: number, sizes: number) =>
			Array.from({ length: sizes }, (_, index) => ({
				parameters: { size: index + 1 },
				measures: Object.fromEntries(
					Array.from({ length: count }, (_, n) => [
						`m${n + 1}`,
						{ value: n + index + 1 },
					]),
				),
			}));
		for (const daysAgo of [2, 1]) {
			await postReport(request, project.slug, {
				start: new Date(Date.now() - daysAgo * DAY).toISOString(),
				// The most lines a key draws, and eight measures under the line cap.
				results: { hash: measured(4, 13), pair: measured(8, 2) },
			});
		}
		const id = await dimensions(request, project.slug);
		const measures = await request.get(
			`${seed.api_url}/v0/projects/${project.slug}/measures?per_page=64`,
			{ headers: { Authorization: `Bearer ${seed.member.token}` } },
		);
		const uuids = ((await measures.json()) as { uuid: string; name: string }[])
			.sort((a, b) => a.name.localeCompare(b.name, "en", { numeric: true }))
			.map(({ uuid }) => uuid);
		for (const [benchmark, count, layout, lines] of [
			["hash", 1, "dual", 13],
			["hash", 2, "dual", 26],
			["hash", 2, "stacked", 26],
			["hash", 3, "dual", 39],
			["hash", 4, "dual", 52],
			["pair", 8, "dual", 16],
		] as const) {
			const query = {
				...blankQuery(),
				branches: [{ uuid: id.branch("main") }],
				testbeds: [{ uuid: id.testbed("ubuntu-latest") }],
				benchmarks: [id.benchmark(benchmark)],
				measures: uuids.slice(0, count),
				layout,
			};
			const height = embedHeight(query);
			for (const width of [320, 390, 767, 768, 1280]) {
				const at = `${count} ${layout} at ${width}`;
				await page.setViewportSize({ width, height });
				await page.goto(embedLink(project.slug, encodeQuery(query)));
				await expect(page.getByRole("figure"), at).toHaveAccessibleName(
					`${lines} lines`,
				);
				const { scrolls, sideways } = await page.evaluate(() => ({
					scrolls: document.documentElement.scrollHeight > window.innerHeight,
					sideways: document.documentElement.scrollWidth > window.innerWidth,
				}));
				expect(scrolls, at).toBe(false);
				expect(sideways, at).toBe(false);
			}
		}
	});

	test("the embed passes axe in both themes", async ({ page, request }) => {
		const { project, search } = await twoRecentLines(request);
		await axeInBothThemes(page, embedLink(project.slug, search), () =>
			twoLinesDrawn(page),
		);
	});

	// Kills Copy embed code that copies a frame of another page or query, one
	// that does not load, or a title that is not the project's.
	test("on the public plot, Copy embed code copies a frame that draws the same query", async ({
		page,
		context,
		request,
	}) => {
		await context.grantPermissions(["clipboard-read", "clipboard-write"]);
		const { project, search } = await twoRecentLines(request);
		await page.goto(publicPath(project.slug, `${search}&x_axis=version`));
		await twoLinesDrawn(page);
		await page.getByRole("button", { name: "Share" }).click();
		await page.getByRole("menuitem", { name: "Copy embed code" }).click();
		await expect(
			page.getByRole("button", { name: "Embed code copied" }),
		).toBeVisible();
		const snippet = await page.evaluate(() => navigator.clipboard.readText());

		const host = await context.newPage();
		await host.setContent(snippet);
		const frame = host.locator("iframe");
		await expect(frame).toHaveAttribute("title", project.name);
		const src = new URL((await frame.getAttribute("src")) ?? "");
		expect(src.origin).toBe(new URL(page.url()).origin);
		expect(src.pathname).toBe(embedLink(project.slug));
		expect(decodeQuery(src.search)).toEqual(
			decodeQuery(new URL(page.url()).search),
		);
		await expect(
			host.frameLocator("iframe").getByRole("figure"),
		).toHaveAccessibleName("2 lines");
	});
});

test.describe("a signed in member", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills an embed that reads as the reader, which would show another site's
	// visitors what only the reader may see.
	test("a private project's embed draws nothing, even for its member", async ({
		page,
	}) => {
		const asked = apiRequests(page);
		const id = "00000000-0000-4000-8000-000000000001";
		await page.goto(
			embedLink(
				seed.projects.private.slug,
				`?${new URLSearchParams({ branches: id, testbeds: id, benchmarks: id, measures: id })}`,
			),
		);
		await expect(page.getByText("Private or not found")).toBeVisible();
		await expect(page.getByRole("figure")).toHaveCount(0);
		await settle(page);
		expect(asked.length).toBeGreaterThan(0);
		expect(
			sent(asked).filter(({ authorization }) => authorization !== undefined),
		).toEqual([]);
	});

	// Kills Explore's Share without the embed on a public project, or one that
	// copies a frame of another query or of the pin the embed cannot read.
	test("in Explore on a public project, Share copies the link or the embed code", async ({
		page,
		context,
		request,
	}) => {
		await context.grantPermissions(["clipboard-read", "clipboard-write"]);
		const { project, search } = await twoRecentLines(request);
		await page.goto(explore(project.slug, decodeQuery(search)));
		await twoLinesDrawn(page);
		const share = page.getByRole("button", { name: "Share" });
		await expect(share).toHaveAttribute("aria-haspopup", "menu");
		await share.click();
		await page.getByRole("menuitem", { name: "Copy link" }).click();
		await expect(
			page.getByRole("button", { name: "Link copied" }),
		).toBeFocused();
		expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
			page.url(),
		);

		await page.keyboard.press("Enter");
		await expect(
			page.getByRole("menuitem", { name: "Copy link" }),
		).toBeFocused();
		await page.keyboard.press("ArrowDown");
		await page.keyboard.press("Enter");
		await expect(
			page.getByRole("button", { name: "Embed code copied" }),
		).toBeVisible();
		const snippet = await page.evaluate(() => navigator.clipboard.readText());
		const src = new URL(
			/src="([^"]*)"/.exec(snippet)?.[1]?.replaceAll("&amp;", "&") ?? "",
		);
		expect(src.pathname).toBe(embedLink(project.slug));
		expect(decodeQuery(src.search)).toEqual(
			decodeQuery(new URL(page.url()).search),
		);
		expect(snippet).toContain(`title="${project.name}"`);
	});

	// Kills a pin's embed code that loses the pin's title, or carries the pin
	// the embed cannot read.
	test("in Explore on a pin, the embed code is titled by the pin and its project", async ({
		page,
		context,
		request,
	}) => {
		await context.grantPermissions(["clipboard-read", "clipboard-write"]);
		const { project } = await twoRecentLines(request);
		const title = `Hash "sizes" <all> & more`;
		const pin = await createPin(request, project.slug, { title });
		await page.goto(explore(project.slug, { plot: pin.uuid }));
		await twoLinesDrawn(page);
		await page.getByRole("button", { name: "Share" }).click();
		await page.getByRole("menuitem", { name: "Copy embed code" }).click();
		await expect(
			page.getByRole("button", { name: "Embed code copied" }),
		).toBeVisible();
		const frame = await page.evaluate(async () => {
			const parsed = new DOMParser().parseFromString(
				await navigator.clipboard.readText(),
				"text/html",
			);
			const iframe = parsed.querySelector("iframe");
			return {
				title: iframe?.getAttribute("title"),
				src: iframe?.getAttribute("src") ?? "",
			};
		});
		expect(frame.title).toBe(`${title} | ${project.name}`);
		expect(decodeQuery(new URL(frame.src).search).plot).toBeUndefined();
	});

	// Kills a private project's Share that offers an embed no one can load, or
	// asks before it copies the link.
	test("in Explore on a private project, Share copies the link at once", async ({
		page,
		context,
		request,
	}) => {
		await context.grantPermissions(["clipboard-read", "clipboard-write"]);
		const { project, search } = await twoRecentLines(request);
		const patched = await request.patch(
			`${seed.api_url}/v0/projects/${project.slug}`,
			{
				headers: { Authorization: `Bearer ${seed.member.token}` },
				data: { visibility: "private" },
			},
		);
		expect(patched.ok(), await patched.text()).toBe(true);
		await page.goto(explore(project.slug, decodeQuery(search)));
		await twoLinesDrawn(page);
		const share = page.getByRole("button", { name: "Share" });
		await expect(share).not.toHaveAttribute("aria-haspopup");
		await share.click();
		await expect(
			page.getByRole("button", { name: "Link copied" }),
		).toBeVisible();
		await expect(page.getByRole("menu")).toHaveCount(0);
		expect(await page.evaluate(() => navigator.clipboard.readText())).toBe(
			page.url(),
		);
	});
});
