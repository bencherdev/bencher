import { createHash, randomUUID } from "node:crypto";
import type { APIRequestContext, BrowserContext, Page } from "@playwright/test";
import { embedPath, publicPlotPath } from "../src/next/public/query";
import {
	type ExploreQuery,
	blankQuery,
	encodeQuery,
} from "../src/next/query/query";
import type {
	JsonAlert,
	JsonAuthUser,
	JsonConsoleRuns,
	JsonOrganization,
	JsonProject,
	JsonThreshold,
} from "../src/types/bencher";
import { createMember, send } from "./api";
import {
	consolePath,
	crumb,
	expect,
	seed,
	settle,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { signUp } from "./mail";

// Every page at the caps the API accepts: 64-byte names (CJK and RTL among
// them), 256-byte slash branches, 1024-byte Rust paths, 8 parameter keys of 64
// bytes each, 1e300 and 1e-300 values, and 64 lines on one plot.

const WIDTHS = [
	{ width: 1440, height: 900 },
	{ width: 768, height: 1024 },
	{ width: 320, height: 568 },
];

const HOUR = 60 * 60 * 1_000;
const DAY = 24 * HOUR;
const NOW = Date.parse(seed.now);

const fill = (start: string, length: number, filler = "x") =>
	start.padEnd(length, filler).slice(0, length);

/** A 256-byte branch of slash segments; each differs at its start, since a slug keeps only the first 64. */
const longBranch = (index: number) => {
	let name = `release/v${index}/2026/q4/team-platform`;
	for (let segment = 0; name.length < 256; segment++) {
		name += `/subsystem-${segment}`;
	}
	const leaf = `/leaf-${index}`;
	return `${name.slice(0, 256 - leaf.length).replace(/\/$/, "a")}${leaf}`;
};

/** A 1024-byte Rust path that differs from its siblings at both ends. */
const rustPath = (index: number) => {
	const tail = `::bench_fn_${index}`;
	let path = `crate_${index}`;
	for (let module = 0; path.length < 1024; module++) {
		path += `::module_${String(module).padStart(3, "0")}`;
	}
	const head = path.slice(0, 1024 - tail.length);
	return `${head.replace(/:+$/, (colons) => "m".repeat(colons.length))}${tail}`;
};

const hashOf = (value: string) =>
	createHash("sha1").update(value).digest("hex");

const PARAMETERS = Object.fromEntries(
	Array.from({ length: 8 }, (_, key) => [
		fill(`k${key}_`, 64, "k"),
		fill(`v${key}_`, 64, "v"),
	]),
);

const [LAST = ""] = Object.keys(PARAMETERS).slice(-1);
const SHARED = Object.fromEntries(
	Object.entries(PARAMETERS).filter(([key]) => key !== LAST),
);

const read = async <T>(
	request: APIRequestContext,
	path: string,
	token: string,
): Promise<T> => {
	const response = await request.get(`${seed.api_url}${path}`, {
		headers: { Authorization: `Bearer ${token}` },
	});
	if (!response.ok()) {
		throw new Error(`GET ${path} answered ${response.status()}`);
	}
	return (await response.json()) as T;
};

type Seeded = Awaited<ReturnType<typeof seedAtCaps>>;

const seedAtCaps = async (request: APIRequestContext) => {
	const tag = randomUUID().slice(0, 8);
	const spaced = fill(
		`${tag} Quarterly Platform Performance Regression Group`,
		64,
	);
	const email = (local: string) =>
		`${fill(`${local}.${tag}.`, 64, "e")}@a-rather-long-subdomain.long-organization-domain.example`;
	const reader = await signUp(request, {
		name: spaced,
		email: email("reader"),
	});
	const token = reader.token;
	const post = <T>(path: string, data: unknown) =>
		send<T>(request, "POST", path, token, data);

	const organization = await post<JsonOrganization>("/v0/organizations", {
		name: fill(tag, 64, "o"),
	});
	const created = await post<JsonProject>(
		`/v0/organizations/${organization.slug}/projects`,
		{
			name: fill(tag, 64, "p"),
			url: `https://github.com/${"o".repeat(39)}/${"r".repeat(100)}/tree/${"b".repeat(60)}`,
			visibility: "public",
		},
	);
	const project = await send<JsonProject>(
		request,
		"PATCH",
		`/v0/projects/${created.slug}`,
		seed.admin.token,
		{ bmf_version: 1 },
	);
	const slug = project.slug;
	const at = (path: string) => `/v0/projects/${slug}/${path}`;

	const testbeds: string[] = [];
	for (const name of [
		fill(tag, 64, "t"),
		"性能测试环境".repeat(4).slice(0, 21),
		"بيئةاختبارالأداء".repeat(2),
	]) {
		testbeds.push(
			(await post<{ slug: string }>(at("testbeds"), { name })).slug,
		);
	}
	const measure = await post<{ slug: string }>(at("measures"), {
		name: fill(`${tag} measure`, 64, "m"),
		units: fill("custom-unit", 64, "u"),
	});
	const branches = [0, 1, 2].map(longBranch);
	for (const name of branches) {
		await post(at("branches"), { name });
	}
	const report = (
		branch: string,
		testbed: string,
		hash: string,
		start: number,
		results: unknown,
		extra: Record<string, unknown> = {},
	) =>
		post<{ uuid: string }>(at("reports"), {
			branch,
			testbed,
			hash: hashOf(hash),
			start_time: new Date(start).toISOString(),
			end_time: new Date(start + 2 * 60_000).toISOString(),
			results: [JSON.stringify(results)],
			settings: { adapter: "json" },
			...extra,
		});

	// Four runs of extreme values, the last over a percentage threshold.
	const extremes = (scale: number) =>
		Object.fromEntries(
			[1e300 * scale, 1e-300, 0, -123456789.123, 1234567890123].map(
				(value, index) => [
					rustPath(index),
					[
						{
							parameters: index === 4 ? {} : PARAMETERS,
							measures: {
								[measure.slug]: { value },
								latency: { value },
							},
						},
					],
				],
			),
		);
	const percentage = {
		models: [
			{
				measure: "latency",
				metric: "value",
				model: { test: "percentage", upper_boundary: 0.1 },
			},
		],
	};
	let longReport: { uuid: string } | undefined;
	for (let index = 0; index < 4; index++) {
		longReport = await report(
			branches[0] ?? "",
			testbeds[0] ?? "",
			`long ${index}`,
			NOW - (10 - index) * DAY,
			extremes(index === 3 ? 1.5 : 1),
			{ thresholds: percentage },
		);
	}

	// One benchmark of 64 variants, which share every tag but the last: 64 lines.
	const lines = rustPath(64);
	const many = (shift: number) => ({
		[lines]: Array.from({ length: 64 }, (_, index) => ({
			parameters: {
				...SHARED,
				[LAST]: fill(`${index}-value-`, 64, "v"),
			},
			measures: { latency: { value: 100 + index * 3 + shift } },
		})),
	});
	let manyReport: { uuid: string } | undefined;
	for (const shift of [0, 1]) {
		manyReport = await report(
			"main",
			"localhost",
			`many ${shift}`,
			NOW - (3 - shift) * DAY,
			many(shift),
		);
	}

	// One run across every testbed.
	for (const [index, testbed] of [...testbeds, "localhost"].entries()) {
		await report(
			branches[1] ?? "",
			testbed,
			"across",
			NOW - 2 * HOUR + index * 60_000,
			{
				[rustPath(1)]: [
					{ parameters: PARAMETERS, measures: { latency: { value: 5 } } },
				],
			},
		);
	}

	const list = <T>(path: string) => read<T>(request, at(path), token);
	type Named = { uuid: string; name: string; slug: string };
	const id = async (dimension: string, names: string[]) => {
		const found = await list<Named[]>(`${dimension}?per_page=255`);
		return names.map((name) => {
			const entry = found.find(
				(each) => each.name === name || each.slug === name,
			);
			if (!entry) {
				throw new Error(`No ${dimension} named ${name}`);
			}
			return entry;
		});
	};
	const [main, ...long] = await id("branches", ["main", ...branches]);
	const [localhost, ...beds] = await id("testbeds", ["localhost", ...testbeds]);
	const benches = await id("benchmarks", [0, 1, 3, 64].map(rustPath));
	const [latency, wide] = await id("measures", ["latency", measure.slug]);
	if (!(main && localhost && latency && wide)) {
		throw new Error("A dimension is missing");
	}
	const plot = (title: string) =>
		post(at("plots"), {
			title,
			lower_value: false,
			upper_value: false,
			lower_boundary: false,
			upper_boundary: false,
			x_axis: "date_time",
			window: 30 * 24 * 60 * 60,
			branches: [main.uuid],
			testbeds: [localhost.uuid],
			benchmarks: [benches[3]?.uuid],
			measures: [latency.uuid],
			metrics: ["value"],
		});
	await plot(fill(`${tag} pinned plot of sixty four lines`, 64, "t"));

	await post(at("keys"), { name: fill(`${tag} project key`, 64, "k") });
	await post(`/v0/users/${reader.user.slug}/keys`, {
		name: fill(`${tag} user key`, 64, "k"),
	});
	await createMember(
		request,
		organization,
		await signUp(request, {
			name: fill(`${tag} Member`, 64, "n"),
			email: email("member"),
		}),
	);

	const alerts = await list<JsonAlert[]>("alerts?per_page=255");
	const alert = alerts[0];
	const thresholds = await list<JsonThreshold[]>("thresholds?per_page=255");
	const threshold = thresholds[0];
	const { runs } = await read<JsonConsoleRuns>(
		request,
		`/v0/console/projects/${slug}/runs?per_page=1`,
		token,
	);
	const run = runs[0];
	if (!(alert?.metric && threshold && run && longReport && manyReport)) {
		throw new Error("The seed raised no alert, threshold, or run");
	}

	const query = (picks: Partial<ExploreQuery>) =>
		encodeQuery({ ...blankQuery(), ...picks });
	const linesQuery = query({
		branches: [{ uuid: main.uuid }],
		testbeds: [{ uuid: localhost.uuid }],
		benchmarks: [benches[3]?.uuid ?? ""],
		sets: [SHARED],
		measures: [latency.uuid],
	});
	// 54 permutations, under the cap of 64.
	const namesQuery = query({
		branches: long.map(({ uuid }) => ({ uuid })),
		testbeds: beds.map(({ uuid }) => ({ uuid })),
		benchmarks: benches.slice(0, 3).map(({ uuid }) => uuid),
		sets: [PARAMETERS],
		measures: [wide.uuid, latency.uuid],
	});

	return {
		reader,
		organization: organization.slug,
		slug,
		run: run.uuid,
		longReport: longReport.uuid,
		manyReport: manyReport.uuid,
		alert: alert.uuid,
		metric: alert.metric.uuid,
		threshold: threshold.uuid,
		branch: long[0]?.slug ?? "",
		testbed: beds[1]?.slug ?? "",
		benchmark: benches[0]?.slug ?? "",
		measure: wide.slug,
		linesQuery,
		namesQuery,
	};
};

type State = readonly [
	name: string,
	path: string,
	then?: (page: Page) => Promise<void>,
];

// Each group is a test of its own, so the states spread across workers.
const GROUPS = [
	"project tabs",
	"project pages",
	"settings and account",
	"public pages",
] as const;

/** Each page state, by group, with its path and what to do once it has drawn. */
const groups = (
	s: Seeded,
): Record<(typeof GROUPS)[number], readonly State[]> => {
	const project = (tab = "") => consolePath(s.slug, tab);
	const user = (page: string) => `/console/users/${s.reader.user.slug}/${page}`;
	const organization = (page: string) =>
		`/console/organizations/${s.organization}/${page}`;
	return {
		"project tabs": [
			["Runs", project()],
			["Runs, a run open", project(), openRun],
			["Explore, 64 lines", `${project("explore")}${s.linesQuery}`],
			["Explore, long names", `${project("explore")}${s.namesQuery}`],
			["Explore, blank", project("explore")],
			["Plots", project("plots")],
			["Alerts", project("alerts")],
			["Thresholds", project("thresholds")],
			["Branches", project("branches")],
			["Testbeds", project("testbeds")],
			["Benchmarks", project("benchmarks")],
			["Measures", project("measures")],
			["Switcher", project(), openSwitcher],
		],
		"project pages": [
			["Run", project(`runs/${s.run}`)],
			["Report", project(`reports/${s.longReport}`)],
			["Report, a line open", project(`reports/${s.longReport}`), openLine],
			["Report, 64 lines", project(`reports/${s.manyReport}`)],
			["Metric", project(`metrics/${s.metric}`)],
			["Alert", project(`alerts/${s.alert}`)],
			["Threshold", project(`thresholds/${s.threshold}`)],
			["Branch", project(`branches/${s.branch}`)],
			["Testbed", project(`testbeds/${s.testbed}`)],
			["Benchmark", project(`benchmarks/${s.benchmark}`)],
			["Measure", project(`measures/${s.measure}`)],
		],
		"settings and account": [
			["Settings", project("settings")],
			["Keys", project("keys")],
			["Connections", project("connections"), disconnected],
			["Not found", project("q".repeat(64))],
			["Unknown project", consolePath("q".repeat(64))],
			["Profile", user("settings")],
			["User keys", user("keys")],
			["Connected apps", user("apps")],
			["Legacy tokens", user("tokens")],
			["Organizations", "/console/organizations"],
			["Organization", organization("settings")],
			["Members", organization("members")],
			["Billing", organization("billing")],
			["New project", organization("projects/add")],
		],
		"public pages": [
			["Public plot", `${publicPlotPath(s.slug)}${s.linesQuery}`],
			[
				"Public plot, query shown",
				`${publicPlotPath(s.slug)}${s.namesQuery}`,
				showQuery,
			],
			["Embed", `${embedPath(s.slug)}${s.namesQuery}`],
		],
	};
};

const openRun = async (page: Page) => {
	const toggle = page
		.getByRole("table", { name: "Runs, newest first" })
		.locator("button[aria-expanded]")
		.first();
	await toggle.click();
	await expect(toggle).toHaveAttribute("aria-expanded", "true");
};

const openLine = async (page: Page) => {
	await page
		.getByRole("button", { name: /^Expand / })
		.first()
		.click();
	await expect(
		page.getByRole("button", { name: /^Collapse / }).first(),
	).toBeVisible();
};

const disconnected = (page: Page) =>
	expect(page.getByRole("region", { name: "GitHub" })).toContainText(
		"Removed from the installation",
	);

const showQuery = async (page: Page) => {
	await page.getByRole("button", { name: "Show query" }).click();
	await expect(page.getByRole("button", { name: "Hide query" })).toBeVisible();
};

// The crumb opens the switcher from 768 px; under it, the account menu does.
const openSwitcher = async (page: Page) => {
	if ((page.viewportSize()?.width ?? 0) >= 768) {
		await crumb(page).click();
	} else {
		await page
			.getByRole("banner")
			.getByRole("button", { name: "Account" })
			.click();
		await page.getByRole("menuitem", { name: "Switch project" }).click();
	}
	await expect(
		page.getByRole("dialog", { name: "Switch project" }),
	).toBeVisible();
};

/** What widens the page or an open dialog past its own width, or nothing. */
const sideways = (page: Page) =>
	page.evaluate(() => {
		const describe = (element: Element) =>
			`<${element.tagName.toLowerCase()} class="${element.getAttribute("class") ?? ""}"> ${(element.textContent ?? "").slice(0, 40)}`;
		/** The innermost elements under `root` that reach past `edge`. */
		const culprits = (root: Element, edge: number) => {
			const reach = (element: Element) => {
				const box = element.getBoundingClientRect();
				return getComputedStyle(element).overflowX === "visible"
					? Math.max(box.right, box.left + element.scrollWidth)
					: box.right;
			};
			const held = (element: Element) => {
				for (
					let parent = element.parentElement;
					parent && parent !== root;
					parent = parent.parentElement
				) {
					if (getComputedStyle(parent).overflowX !== "visible") {
						return true;
					}
				}
				return false;
			};
			const over = Array.from(root.querySelectorAll("*")).filter(
				(element) => reach(element) > edge + 0.5,
			);
			// A box that scrolls or clips holds what it contains, except an
			// absolutely positioned element whose containing block is outside it.
			const loose = over.filter((element) => !held(element));
			const picked = new Set(loose.length > 0 ? loose : over);
			return Array.from(picked)
				.filter((element) =>
					Array.from(element.children).every((child) => !picked.has(child)),
				)
				.slice(0, 3)
				.map(describe)
				.join(" | ");
		};
		const problems: string[] = [];
		const { scrollWidth, clientWidth } = document.documentElement;
		if (scrollWidth > clientWidth) {
			problems.push(
				`page ${scrollWidth}px in ${clientWidth}px: ${culprits(document.body, clientWidth)}`,
			);
		}
		for (const dialog of document.querySelectorAll(
			"dialog[open], [role=dialog]",
		)) {
			if (dialog.scrollWidth > dialog.clientWidth) {
				const edge =
					dialog.getBoundingClientRect().left +
					dialog.clientLeft +
					dialog.clientWidth;
				problems.push(
					`dialog ${dialog.scrollWidth}px in ${dialog.clientWidth}px: ${culprits(dialog, edge)}`,
				);
			}
		}
		return problems.length > 0 ? problems.join("; ") : undefined;
	});

/** Each visible page title that has no width left. */
const flatTitles = (page: Page) =>
	page.evaluate(() =>
		Array.from(document.querySelectorAll("h1"))
			.filter(
				(title) =>
					title.checkVisibility({ visibilityProperty: true }) &&
					title.getBoundingClientRect().width <= 0,
			)
			.map(
				(title) =>
					`<h1 class="${title.getAttribute("class") ?? ""}"> ${(title.textContent ?? "").slice(0, 40)}`,
			),
	);

let seeded: Seeded;

test.beforeAll(async ({ playwright }) => {
	test.setTimeout(120_000);
	const request = await playwright.request.newContext();
	seeded = await seedAtCaps(request);
	await request.dispose();
});

// Kills any page or dialog that scrolls sideways once its names and values reach the caps.
for (const viewport of WIDTHS) {
	for (const group of GROUPS) {
		test(`no ${group} scroll sideways at the caps at ${viewport.width} px`, async ({
			newContext,
		}) => {
			test.setTimeout(120_000);
			const s = seeded;
			const reader = group === "public pages" ? undefined : s.reader;
			const context = await newContext({
				viewport,
				storageState: reader ? signedIn(reader) : signedOut,
			});
			if (reader) {
				await legacyTokens(context, reader);
				await linkedRepository(context, s);
			}
			const page = await context.newPage();
			for (const [name, path, then] of groups(s)[group]) {
				await page.goto(path);
				await expect(
					page.getByRole("heading", { level: 1 }).first(),
				).toBeAttached();
				await then?.(page);
				await settle(page);
				expect
					.soft(await sideways(page), `${name} at ${viewport.width} px`)
					.toBeUndefined();
				expect
					.soft(await flatTitles(page), `${name} at ${viewport.width} px`)
					.toEqual([]);
			}
		});
	}
}

/** Two legacy tokens with 64-byte names, since no API makes one any more. */
const legacyTokens = (context: BrowserContext, reader: JsonAuthUser) =>
	context.route(
		`${seed.api_url}/v0/users/${reader.user.slug}/tokens?*`,
		async (route) => {
			if (route.request().method() !== "GET") {
				return route.fallback();
			}
			const response = await route.fetch();
			const { "content-length": _, ...headers } = response.headers();
			const list = new URL(route.request().url()).searchParams.has("revoked");
			await route.fulfill({
				response,
				json: list
					? [0, 1].map((index) => ({
							uuid: `00000000-0000-4000-8000-00000000000${index}`,
							user: reader.user.uuid,
							name: index ? fill("legacy token ", 64, "n") : "t".repeat(64),
							token: `legacy-secret-${index}`,
							creation: "2023-11-20T00:00:00Z",
							expiration: "2028-11-20T00:00:00Z",
						}))
					: await response.json(),
				headers: { ...headers, "x-total-count": "2" },
			});
		},
	);

/** A disconnected repository at GitHub's longest owner and name; the test server has no GitHub App to link one. */
const linkedRepository = (context: BrowserContext, s: Seeded) =>
	context.route(`${seed.api_url}/v0/projects/${s.slug}/github`, (route) =>
		route.request().method() === "GET"
			? route.fulfill({
					headers: { "Access-Control-Allow-Origin": "*" },
					json: {
						linked_repository: {
							full_name: `${"o".repeat(39)}/${"r".repeat(100)}`,
							status: "disconnected",
							reason: "removed",
							user: s.reader.user,
							linked: seed.now,
						},
					},
				})
			: route.fallback(),
	);
