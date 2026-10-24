import { accountPath, shortcutPage } from "../src/next/account/paths";
import { classicRedirect } from "../src/next/classic";
import { CLASSIC_ROUTES, classicPath } from "../src/next/classic-routes";
import {
	consolePath,
	expect,
	logInTo,
	seed,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { finishGitHub, startGitHub } from "./github";

const { hashbrown } = seed.projects;
const UUID = "0e5a0c32-8d3f-4b52-9a7e-3f8e0f1c2d4b";
const PARAMS = {
	project: hashbrown.slug,
	organization: seed.organization.slug,
	user: seed.member.user.slug,
	alert: UUID,
	report: UUID,
	threshold: UUID,
	plot: UUID,
	key: UUID,
	metric: UUID,
	member: UUID,
	token: UUID,
	branch: "main",
	testbed: "ubuntu-latest",
	benchmark: "blake3",
	measure: "latency",
};
// What a released CLI adds to a link, and what only the classic page read.
const SEARCH = "?utm_source=github&utm_campaign=pr%2Bcomments&tab=branches";

/** Where the server lands `path`, before any page script runs, and whether a reader must sign in there. */
const landing = (path: string, search: string) => {
	const to = classicRedirect({ pathname: path, search });
	if (to) {
		return { to, public: to === "/explore" };
	}
	return {
		to: `${path}${search}`,
		public: /^\/perf\/[^/]+(\/embed)?$/.test(path),
	};
};

test.describe("every classic route", () => {
	test.describe("signed in", () => {
		test.use({ storageState: signedIn(seed.member) });

		// Kills a classic route with no page and no redirect, a redirect the
		// server does not make or makes to another page, and a shortcut that
		// does not find the reader.
		for (const route of CLASSIC_ROUTES) {
			test(route, async ({ page }) => {
				const path = classicPath(route, PARAMS);
				const response = await page.goto(`${path}${SEARCH}`);
				expect(response?.status()).toBe(200);
				const shortcut = shortcutPage(path);
				if (shortcut) {
					await expect(page).toHaveURL(
						accountPath(seed.member.user.slug, shortcut),
					);
					return;
				}
				const { to } = landing(path, SEARCH);
				expect(response?.url()).toBe(new URL(to, seed.console_url).href);
			});
		}
	});

	test.describe("signed out", () => {
		test.use({ storageState: signedOut });

		// Kills a classic route that a reader who is not signed in reaches with
		// no way back to the page it now lands on, and a public page sent to
		// sign in.
		for (const route of CLASSIC_ROUTES) {
			test(route, async ({ page }) => {
				const path = classicPath(route, PARAMS);
				await page.goto(`${path}${SEARCH}`);
				const { to, public: open } = shortcutPage(path)
					? { to: `${path}${SEARCH}`, public: false }
					: landing(path, SEARCH);
				// The console's root stays classic, and its sign in lands on the main project.
				if (to === "/console") {
					await expect(page).toHaveURL((url) => url.pathname === "/auth/login");
				} else if (open) {
					await expect(page).toHaveURL(
						(url) => url.pathname === new URL(to, seed.console_url).pathname,
					);
					await expect(page).not.toHaveURL(/\/auth\/login/);
				} else {
					await expect(page).toHaveURL(logInTo(to));
				}
			});
		}
	});
});

test.describe("a released CLI's pull request comment", () => {
	const head = "11111111-1111-4111-8111-111111111111";
	const spec = "22222222-2222-4222-8222-222222222222";
	const model = "33333333-3333-4333-8333-333333333333";
	const utm = new URLSearchParams({
		utm_medium: "referral",
		utm_source: "github",
		utm_content: "comment",
		utm_campaign: "pr+comments",
		utm_term: hashbrown.slug,
	});
	const perf = new URLSearchParams({
		branches: UUID,
		heads: head,
		testbeds: UUID,
		specs: spec,
		benchmarks: UUID,
		measures: UUID,
		start_time: "1757000000000",
		end_time: "1757800000000",
		report: UUID,
		upper_boundary: "true",
	});
	const explore = `?branches=${UUID}&heads=${head}&testbeds=${UUID}&specs=${spec}&benchmarks=${UUID}&measures=${UUID}&start_time=1757000000000&end_time=1757800000000&report=${UUID}`;

	/** Each link as the CLI writes it, under `base`, with where it lands. */
	const links = (base: string, plot: string): [string, string][] => {
		const resource = (rest: string, query = "") =>
			`${base}/${rest}?${query}${query ? "&" : ""}${utm}`;
		const project = consolePath(hashbrown.slug).replace(/\/$/, "");
		return [
			[`${base}?${utm}`, base.startsWith("/perf") ? base : project],
			[
				`${plot}?${perf}`,
				base.startsWith("/perf")
					? `${base}${explore}`
					: `${project}/explore${explore}`,
			],
			[resource(`reports/${UUID}`), `${project}/reports/${UUID}`],
			[resource("branches/main", `head=${head}`), `${project}/branches/main`],
			[
				resource("testbeds/ubuntu-latest", `spec=${spec}`),
				`${project}/testbeds/ubuntu-latest`,
			],
			[resource("benchmarks/blake3"), `${project}/benchmarks/blake3`],
			[resource("measures/latency"), `${project}/measures/latency`],
			[
				resource(`thresholds/${UUID}`, `model=${model}`),
				`${project}/thresholds/${UUID}`,
			],
			[resource(`alerts/${UUID}`), `${project}/alerts/${UUID}`],
			[`${project}/thresholds/add?${utm}`, `${project}/thresholds`],
		];
	};
	const comments = {
		public: links(`/perf/${hashbrown.slug}`, `/perf/${hashbrown.slug}`),
		private: links(
			`/console/projects/${hashbrown.slug}`,
			`/console/projects/${hashbrown.slug}/perf`,
		),
	};

	for (const [kind, shapes] of Object.entries(comments)) {
		test.describe(`for a ${kind} project, signed in`, () => {
			test.use({ storageState: signedIn(seed.member) });

			// Kills a released CLI's link that lands anywhere but the page that
			// replaced its classic page, a plot link that keeps the classic query
			// or loses its lines, and tracking that changes the page it lands on.
			test("each link lands on its page", async ({ page }) => {
				for (const [link, to] of shapes) {
					const response = await page.goto(link);
					const expected = new URL(to, seed.console_url);
					if (expected.search) {
						// A plot link's query is Explore's, written once the page reads it.
						if (to.startsWith("/perf/")) {
							await expect(page, link).toHaveURL(expected.href);
						} else {
							expect(response?.url(), link).toBe(expected.href);
						}
					} else {
						expect(new URL(response?.url() ?? "").pathname, link).toBe(
							expected.pathname,
						);
					}
				}
			});
		});

		test.describe(`for a ${kind} project, signed out`, () => {
			test.use({ storageState: signedOut });

			// Kills a link a reader who is not signed in follows to sign in with
			// no way back to the page it lands on, and the public plot sent there.
			test("each link lands on its page or on sign in for it", async ({
				page,
			}) => {
				for (const [link, to] of shapes) {
					await page.goto(link);
					if (to.startsWith("/perf/")) {
						await expect(page, link).toHaveURL(
							(url) => url.pathname === `/perf/${hashbrown.slug}`,
						);
					} else {
						await expect(page, link).toHaveURL(/\/auth\/login\?back=/);
						const back = Buffer.from(
							new URL(page.url()).searchParams.get("back") ?? "",
							"base64",
						).toString();
						expect(new URL(back, seed.console_url).pathname, link).toBe(
							new URL(to, seed.console_url).pathname,
						);
					}
				}
			});
		});
	}
});

test.describe("a classic public page, signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a classic public page that strands a reader who is not signed in,
	// and a sign in that forgets the console page it now lands on.
	test("lands on sign in and returns to the console page", async ({ page }) => {
		const to = consolePath(hashbrown.slug, "branches/main");
		await page.goto(`/perf/${hashbrown.slug}/branches/main?head=${UUID}`);
		await expect(page).toHaveURL(logInTo(to));
		const { state } = await startGitHub(page, logInTo(to));
		await finishGitHub(page, state);

		await expect(page).toHaveURL(to);
		await expect(page.getByRole("heading", { level: 1 })).toContainText("main");
	});
});

test.describe("the public surface", () => {
	// Kills a robots.txt that hides the public plot or its embed, with or
	// without a query, from crawlers, or opens the console and the classic
	// public pages to them.
	test("robots.txt allows the public plot and its embed and disallows the console", async ({
		request,
	}) => {
		const allowed = crawlable(await (await request.get("/robots.txt")).text());
		const embed = `/perf/${hashbrown.slug}/embed`;
		expect(allowed(`/perf/${hashbrown.slug}`)).toBe(true);
		expect(
			allowed(`/perf/${hashbrown.slug}?branches=${UUID}&x_axis=version`),
		).toBe(true);
		expect(allowed(embed)).toBe(true);
		expect(allowed(`${embed}?branches=${UUID}&measures=${UUID}`)).toBe(true);
		expect(allowed(`/perf/${hashbrown.slug}/reports/${UUID}`)).toBe(false);
		expect(allowed(`/perf/${hashbrown.slug}/plots`)).toBe(false);
		expect(allowed("/console")).toBe(false);
		expect(allowed(consolePath(hashbrown.slug, "explore"))).toBe(false);
		expect(allowed("/explore")).toBe(true);
	});

	// Kills a sitemap that lists a console page, a classic public page, or the
	// public projects list.
	test("the sitemap lists no console page", async ({ request }) => {
		const index = await (await request.get("/sitemap-index.xml")).text();
		const sitemaps = [...index.matchAll(/<loc>([^<]+)<\/loc>/g)].map(
			([, loc]) => new URL(loc ?? "").pathname,
		);
		expect(sitemaps.length).toBeGreaterThan(0);
		const pages = (
			await Promise.all(
				sitemaps.map(async (sitemap) =>
					[
						...(
							await (await request.get(sitemap)).text()
						).matchAll(/<loc>([^<]+)<\/loc>/g),
					].map(([, loc]) => new URL(loc ?? "").pathname),
				),
			)
		).flat();
		expect(pages.length).toBeGreaterThan(0);
		expect(
			pages.filter((path) => /^\/(console|perf)(\/|$)/.test(path)),
		).toEqual([]);
	});
});

/**
 * Whether a crawler may fetch a path and query under the `User-agent: *`
 * rules: the longest matching pattern wins, Allow on a tie, `*` matches any
 * run and a trailing `$` ends the match.
 */
const crawlable = (robots: string) => {
	const rules = robots
		.split("\n")
		.map((line) => /^(Allow|Disallow):\s*(\S+)/.exec(line.trim()))
		.flatMap((match) =>
			match?.[1] && match[2]
				? [{ allow: match[1] === "Allow", pattern: match[2] }]
				: [],
		);
	const matches = (pattern: string, path: string) =>
		new RegExp(
			`^${pattern
				.replace(/\$$/, "")
				.split("*")
				.map((part) => part.replace(/[.*+?^${}()|[\]\\]/g, "\\$&"))
				.join(".*")}${pattern.endsWith("$") ? "$" : ""}`,
		).test(path);
	return (path: string) => {
		const [best] = rules
			.filter(({ pattern }) => matches(pattern, path))
			.sort(
				(a, b) =>
					b.pattern.length - a.pattern.length ||
					Number(b.allow) - Number(a.allow),
			);
		return best?.allow ?? true;
	};
};
