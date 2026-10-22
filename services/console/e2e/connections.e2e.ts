import type { Page } from "@playwright/test";
import type {
	JsonGitHubRepositories,
	JsonLinkedRepository,
	JsonProject,
} from "../src/types/bencher";
import { createProject } from "./api";
import { axeInBothThemes } from "./axe";
import {
	expect,
	nextPath,
	seed,
	settle,
	signedIn,
	signedOut,
	test,
	USER_KEY,
} from "./fixtures";
import { finishGitHub, startGitHub } from "./github";

const { hashbrown } = seed.projects;

const CORS = { "Access-Control-Allow-Origin": "*" };
const GITHUB = "https://github.example";
const RETURN = "/next/console/github";
const GRANT = "eyJhbGciOiJIUzI1NiJ9.eyJyZXBvc2l0b3J5IjoxfQ.Z3JhbnQtc2VjcmV0";
const CODE = "github-code-secret";

const card = (page: Page) => page.getByRole("region", { name: "GitHub" });

/** A link state as the API signs one, naming `project`; the faked API never checks it. */
const stateFor = (project: JsonProject) =>
	`eyJhbGciOiJIUzI1NiJ9.${Buffer.from(JSON.stringify({ project: project.uuid })).toString("base64url")}.c2ln`;

const returnUrl = (project: JsonProject) =>
	`${RETURN}?${new URLSearchParams({ code: CODE, state: stateFor(project) })}`;

/** Sign in, then come back to the return page with no code or state. */
const LOG_IN = `/auth/login?${new URLSearchParams({ back: Buffer.from(RETURN).toString("base64") })}`;

/**
 * Lands on the sign-in page with a return page's way back, and checks the
 * way back it keeps, for a magic link opened in another tab, holds neither
 * GitHub's code nor the state.
 */
const signsIn = async (page: Page, project: JsonProject) => {
	await expect(page).toHaveURL(LOG_IN);
	const kept = () =>
		page.evaluate(() => localStorage.getItem("BENCHER_RETURN") ?? "");
	await expect
		.poll(async () => JSON.parse((await kept()) || "null")?.href)
		.toBe(new URL(RETURN, seed.console_url).href);
	expect(await kept()).not.toContain(CODE);
	expect(await kept()).not.toContain(stateFor(project));
};

/**
 * The API as if it had a GitHub App, for `project`: its link, the trip to
 * GitHub, the code exchange, and the link a grant makes. Every request it
 * answers is recorded; preflights go on to the API, which allows them.
 */
const fakeGitHub = async (page: Page, project: JsonProject) => {
	const path = `${seed.api_url}/v0/projects/${project.slug}/github`;
	const sent: { method: string; url: string; body: unknown }[] = [];
	let link: { linked_repository?: JsonLinkedRepository } = {};
	const answer = (status: number, json: unknown) => ({
		status,
		headers: CORS,
		json,
	});
	await page.route(path, async (route) => {
		const request = route.request();
		const method = request.method();
		if (method === "OPTIONS") {
			return route.fallback();
		}
		sent.push({ method, url: request.url(), body: request.postDataJSON() });
		if (method === "PUT") {
			link = {
				linked_repository: {
					full_name: "pompeii-llc/rosti",
					status: "connected",
					user: seed.member.user,
					linked: seed.now,
				} as JsonLinkedRepository,
			};
			return route.fulfill(answer(200, link.linked_repository));
		}
		return route.fulfill(answer(200, link));
	});
	await page.route(`${path}/urls`, (route) =>
		route.request().method() === "GET"
			? route.fulfill(
					answer(200, {
						authorize: `${GITHUB}/login/oauth/authorize?state=s`,
						install: `${GITHUB}/apps/bencher/installations/new?state=s`,
					}),
				)
			: route.fallback(),
	);
	await page.route(`${seed.api_url}/v0/github/exchange`, (route) => {
		const request = route.request();
		if (request.method() !== "POST") {
			return route.fallback();
		}
		sent.push({
			method: "POST",
			url: request.url(),
			body: request.postDataJSON(),
		});
		const repositories: JsonGitHubRepositories = {
			project: project.slug,
			repositories: [
				{
					full_name: "pompeii-llc/rosti",
					installation: 1,
					installation_url: `${GITHUB}/organizations/pompeii-llc/settings/installations/1`,
					permission: "granted",
					grant: GRANT,
				},
				{
					full_name: "bagge-farm/diner",
					installation: 2,
					installation_url: `${GITHUB}/settings/installations/2`,
					permission: "approval_required",
				},
			] as JsonGitHubRepositories["repositories"],
		};
		return route.fulfill(answer(200, repositories));
	});
	await page.route(`${GITHUB}/**`, (route) =>
		route.fulfill({ contentType: "text/plain", body: "GitHub" }),
	);
	return sent;
};

/** Everything this browser keeps for the page: its storage, its history entry, and the console's cache. */
const stored = (page: Page) =>
	page.evaluate(async () => {
		const cache = await new Promise<string>((resolve) => {
			const open = indexedDB.open("bencher-console");
			open.onsuccess = () => {
				const read = open.result
					.transaction("cache")
					.objectStore("cache")
					.getAll();
				read.onsuccess = () => resolve(JSON.stringify(read.result));
				read.onerror = () => resolve("");
			};
			open.onerror = () => resolve("");
		});
		return [
			JSON.stringify({ ...localStorage }),
			JSON.stringify({ ...sessionStorage }),
			JSON.stringify(history.state),
			location.href,
			cache,
		].join("\n");
	});

test.describe("as the project's Maintainer", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a Link repository that never reaches GitHub, a return page that
	// keeps the code in its URL or its history entry, sends it to a referrer
	// or a cache, or does not open the project's Connections, grants kept
	// anywhere the browser stores, and a link that does not send its grant or
	// show once made.
	test("Link repository goes to GitHub and back, and links the repository", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		const sent = await fakeGitHub(page, project);
		await page.goto(nextPath(project.slug, "connections"));
		await card(page).getByRole("button", { name: "Link repository" }).click();
		await expect(page).toHaveURL(/^https:\/\/github\.example\/login\//);

		const headers = (await page.goto(returnUrl(project)))?.headers() ?? {};
		expect(headers["referrer-policy"]).toBe("no-referrer");
		expect(headers["cache-control"]).toContain("no-store");
		await expect(page).toHaveURL(nextPath(project.slug, "connections"));
		const list = card(page).getByRole("list", { name: "Repositories" });
		await expect(
			list.getByRole("button", { name: "Link pompeii-llc/rosti" }),
		).toBeVisible();
		await expect(list).toContainText("Approval required");
		await expect(
			list.getByRole("button", { name: "Link bagge-farm/diner" }),
		).toHaveCount(0);
		expect(sent.find(({ method }) => method === "POST")?.body).toEqual({
			code: CODE,
			state: stateFor(project),
		});

		// The cache persists to IndexedDB a moment after it changes.
		await settle(page, 1_500);
		const kept = await stored(page);
		// The cache saved the page's link, so its store is no empty check.
		expect(kept).toContain(JSON.stringify(["console", "github", project.slug]));
		expect(kept).not.toContain(GRANT);
		expect(kept).not.toContain(CODE);

		await list.getByRole("button", { name: "Link pompeii-llc/rosti" }).click();
		await expect(card(page)).toContainText("Connected");
		await expect(card(page)).toContainText(seed.member.user.name);
		await expect(
			card(page).getByRole("button", { name: "Unlink" }),
		).toBeVisible();
		await expect(list).toHaveCount(0);
		expect(sent.find(({ method }) => method === "PUT")?.body).toEqual({
			grant: GRANT,
		});

		// The return's own entry was replaced, so Back passes it by for GitHub's.
		const visited: string[] = [];
		page.on("framenavigated", (frame) => {
			if (frame === page.mainFrame()) {
				visited.push(frame.url());
			}
		});
		await page.goBack();
		await expect(page).toHaveURL(/^https:\/\/github\.example\/login\//);
		expect(visited.filter((url) => url.includes(CODE))).toEqual([]);
	});

	// Kills repositories, and their grants, that outlive the page they were
	// listed on.
	test("a reload drops the repositories a return listed", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await fakeGitHub(page, project);
		await page.goto(returnUrl(project));
		const list = card(page).getByRole("list", { name: "Repositories" });
		await expect(list).toBeVisible();

		await page.reload();
		await expect(
			card(page).getByRole("button", { name: "Link repository" }),
		).toBeVisible();
		await expect(list).toHaveCount(0);
	});

	// Kills a refusal that strands the reader, and a way back that does not
	// reach the project's Connections by the UUID the state names.
	test("a refused exchange says why and leads back to the project", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await page.route(`${seed.api_url}/v0/github/exchange`, (route) =>
			route.request().method() === "POST"
				? route.fulfill({
						status: 400,
						headers: CORS,
						json: { message: "The GitHub link state is not valid" },
					})
				: route.fallback(),
		);
		await page.goto(returnUrl(project));
		await expect(page.getByRole("alert")).toContainText(
			"The GitHub link state is not valid",
		);
		await expect(page).toHaveURL(RETURN);
		await page.getByRole("link", { name: "Back to the project" }).click();
		await expect(
			page.getByRole("heading", { level: 1, name: "Connections" }),
		).toBeVisible();
		await expect(card(page)).toContainText("This server has no GitHub App");
	});

	// Kills a return page that leaves GitHub's code and state in its URL for
	// the console to read when its session ends, or is ended in another tab,
	// after the page's first script ran and before the console's code loads.
	for (const [ended, session] of [
		["signs out", null],
		[
			"has their session expire",
			JSON.stringify({ ...seed.member, expiration: "2000-01-01T00:00:00Z" }),
		],
	] as const) {
		test(`a reader who ${ended} as the return page loads leaves without the code or the state`, async ({
			page,
			request,
		}) => {
			const project = await createProject(request);
			let release = () => {};
			const held = new Promise<void>((resolve) => {
				release = resolve;
			});
			await page.route(
				(url) =>
					url.origin === seed.console_url && url.pathname.endsWith(".js"),
				async (route) => {
					await held;
					await route.continue();
				},
			);
			await page.goto(returnUrl(project), { waitUntil: "commit" });
			await page.waitForFunction(() => document.readyState !== "loading");
			await page.evaluate(
				([key, value]) => {
					if (value === null) {
						localStorage.removeItem(key);
					} else {
						localStorage.setItem(key, value);
					}
				},
				[USER_KEY, session] as const,
			);
			release();
			await signsIn(page, project);
		});
	}

	// Kills a server with no app drawn as a project with no link, which offers
	// a Link repository the API would refuse.
	test("a server with no GitHub App says so, with no action", async ({
		page,
	}) => {
		await page.goto(nextPath(hashbrown.slug, "connections"));
		await expect(card(page)).toContainText(
			"This server has no GitHub App, so a project cannot link a GitHub repository.",
		);
		await expect(card(page).getByRole("button")).toHaveCount(0);
	});

	// Kills a rail with no Connections, and one that leaves the app for it.
	test("the rail opens Connections in the app", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "settings"));
		await expect(page.getByRole("textbox", { name: "Name" })).toHaveValue(
			hashbrown.name,
		);
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});
		await page
			.getByRole("navigation", { name: "Settings", exact: true })
			.getByRole("link", { name: "Connections" })
			.click();
		await expect(
			page.getByRole("heading", { level: 1, name: "Connections" }),
		).toBeVisible();
		await expect(card(page)).toContainText("This server has no GitHub App");
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

for (const [reader, storageState] of [
	["signed out", signedOut],
	[
		"signed in with an expired token",
		signedIn({ ...seed.member, expiration: "2000-01-01T00:00:00Z" }),
	],
] as const) {
	test.describe(reader, () => {
		test.use({ storageState });

		// Kills a return page that signs the reader in itself, skips the
		// sign-in guard, or carries GitHub's code or state to the sign-in page
		// or the way back it keeps; and a return after sign in that sends an
		// exchange with no code or never says it read nothing.
		test("a return sends the reader to sign in without the code or the state", async ({
			page,
			request,
		}) => {
			const project = await createProject(request);
			const exchanged: string[] = [];
			page.on("request", (sent) => {
				if (sent.url().startsWith(`${seed.api_url}/v0/github/exchange`)) {
					exchanged.push(sent.method());
				}
			});
			await page.goto(returnUrl(project));
			await signsIn(page, project);

			const { state } = await startGitHub(page, LOG_IN);
			await finishGitHub(page, state);
			await expect(page).toHaveURL(new URL(RETURN, seed.console_url).href);
			await expect(page.getByRole("alert")).toContainText(
				"Bencher did not read your GitHub repositories",
			);
			await expect(
				page.getByRole("link", { name: "Back to the project" }),
			).toHaveCount(0);
			expect(exchanged).toEqual([]);
		});
	});
}

test.describe("as a reader who is a member of nothing", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills Connections controls drawn for a reader without manage.
	test("Connections is read only, with no action", async ({ page }) => {
		await page.goto(nextPath(hashbrown.slug, "connections"));
		await expect(card(page)).toBeVisible();
		await expect(page.getByText("Read only", { exact: true })).toBeVisible();
		await expect(card(page).getByRole("button")).toHaveCount(0);
		await expect(card(page).getByRole("link")).toHaveCount(0);
	});
});

test.describe("accessibility", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("Connections with a return's repositories passes axe in both themes", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await fakeGitHub(page, project);
		await axeInBothThemes(
			page,
			returnUrl(project),
			() =>
				expect(
					card(page).getByRole("button", { name: "Link pompeii-llc/rosti" }),
				).toBeVisible(),
			async () => {
				await page.goto(returnUrl(project));
			},
		);
	});

	test("a linked repository and its Unlink dialog pass axe in both themes", async ({
		page,
		request,
	}) => {
		const project = await createProject(request);
		await fakeGitHub(page, project);
		await page.goto(returnUrl(project));
		await card(page)
			.getByRole("button", { name: "Link pompeii-llc/rosti" })
			.click();
		await expect(card(page)).toContainText("Connected");
		await axeInBothThemes(
			page,
			nextPath(project.slug, "connections"),
			async () => {
				await card(page).getByRole("button", { name: "Unlink" }).click();
				await expect(page.getByRole("alertdialog")).toBeVisible();
			},
		);
	});
});
