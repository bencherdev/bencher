import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import { expect, nextPath, seed, signedIn, signedOut, test } from "./fixtures";
import { finishGitHub, startGitHub } from "./github";
import { mailedLink } from "./mail";

const RETURN_KEY = "BENCHER_RETURN";
// Fixed, since a test's title must read the same wherever it is collected.
const REQUEST = "0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f";
const TTL = 30 * 60 * 1000;

const base64 = (text: string) => Buffer.from(text).toString("base64");
const logIn = (back: string) =>
	`/auth/login?${new URLSearchParams({ back: base64(back) })}`;
const absolute = (path: string) => new URL(path, seed.console_url).href;
const keptReturn = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), RETURN_KEY);
const kept = (path: string, at: number) =>
	JSON.stringify({ href: absolute(path), at });
// The member's projects change as other tests run; the admin's do not.
const mainProject = new URL(nextPath(seed.admin_newest.slug), seed.console_url)
	.href;

const alerts = `${nextPath(seed.projects.hashbrown.slug, "alerts")}?status=all`;

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a sign in that forgets the page that sent the reader there, keeps it
	// where a magic link opened in another tab cannot read it, or keeps it for a
	// later sign in.
	test("a magic link opened in a new tab returns to the page", async ({
		page,
		request,
	}) => {
		const email = `${randomUUID().slice(0, 8)}@nowhere.com`;
		const signup = await request.post(`${seed.api_url}/v0/auth/signup`, {
			data: { name: "Le Quack", email, i_agree: true },
		});
		expect(signup.status()).toBe(202);
		// The sign in forms ask for a reCAPTCHA token, which this API never checks.
		await page.route("https://www.google.com/recaptcha/**", (route) =>
			route.abort(),
		);

		await page.goto(alerts);
		await expect(page).toHaveURL(logIn(alerts));
		// The form replaces its placeholder once it loads, so fill until it takes.
		const button = page.getByRole("button", { name: "Log in", exact: true });
		await expect(async () => {
			await page.getByPlaceholder("email@example.com").fill(email);
			await expect(button).toBeEnabled({ timeout: 1_000 });
		}).toPass();
		await button.click();
		await expect(page).toHaveURL((url) => url.pathname === "/auth/confirm");

		const link = await mailedLink(email, "Confirm Bencher Login");
		const tab = await page.context().newPage();
		await tab.goto(link.href);
		await tab.getByRole("button", { name: `Sign in as ${email}` }).click();
		await expect(tab).toHaveURL(absolute(alerts));
		await expect(tab.getByText("Read only", { exact: true })).toBeVisible();
		expect(await keptReturn(tab)).toBeNull();
	});

	// Kills a provider sign in that lands on the console instead of the page,
	// or keeps the page for a later sign in.
	test("a provider sign in returns to the page", async ({ page }) => {
		const path = nextPath(seed.projects.private.slug, "runs");
		await page.goto(path);
		await expect(page).toHaveURL(logIn(path));
		const { state } = await startGitHub(page, logIn(path));
		await finishGitHub(page, state);
		await expect(page).toHaveURL(absolute(path));
		expect(await keptReturn(page)).toBeNull();
	});

	// Kills a switch between log in and sign up that drops the way back, which
	// the page switched to would then forget.
	test("switching between log in and sign up keeps the way back", async ({
		page,
	}) => {
		const back = new URLSearchParams({ back: base64(alerts) });
		await page.goto(`/auth/login?${back}`);
		await expect(
			page.getByRole("link", { name: "sign up", exact: true }),
		).toHaveAttribute("href", `/auth/signup?${back}`);
		await page.goto(`/auth/signup?${back}`);
		await expect(
			page.getByRole("link", { name: "log in", exact: true }),
		).toHaveAttribute("href", `/auth/login?${back}`);
	});
});

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.admin) });

	// Kills a signed in visit to sign in that drops the page for the console.
	test("signing in again returns to the page", async ({ page }) => {
		await page.goto(logIn(alerts));
		await expect(page).toHaveURL(absolute(alerts));
		expect(await keptReturn(page)).toBeNull();
	});

	// Kills a sign up page that does not keep its way back.
	test("signing up again returns to the page", async ({ page }) => {
		await page.goto(
			`/auth/signup?${new URLSearchParams({ back: base64(alerts) })}`,
		);
		await expect(page).toHaveURL(absolute(alerts));
	});

	// Kills a way back followed off the console's origin, into a script, or to
	// a page that is not one, in any spelling.
	test("a hostile way back never leaves the console", async ({ page }) => {
		const visited: string[] = [];
		const dialogs: string[] = [];
		page.on("framenavigated", (frame) => {
			if (frame === page.mainFrame()) {
				visited.push(frame.url());
			}
		});
		page.on("dialog", (dialog) => {
			dialogs.push(dialog.message());
			void dialog.dismiss();
		});
		await page.route(/evil\.example/, (route) =>
			route.fulfill({ contentType: "text/plain", body: "Evil" }),
		);
		const hostile = [
			"javascript:alert(1)",
			" JavaScript:alert(1)",
			"\tjava\nscript:alert(1)",
			"//evil.example/next/console",
			"/\\evil.example/next/console",
			" //evil.example",
			"https://evil.example/next/console",
			`${seed.console_url}@evil.example/next/console`,
			`${seed.api_url}/v0/projects`,
			"/auth/logout",
		];
		for (const back of hostile) {
			await page.goto(logIn(back));
			await expect(page, back).toHaveURL(mainProject);
		}
		// Not base64, and base64 of a percent encoded path.
		for (const back of [
			"javascript:alert(1)",
			base64("%2F%2Fevil.example"),
			base64("/%2F%2Fevil.example"),
		]) {
			await page.goto(`/auth/login?${new URLSearchParams({ back })}`);
			await expect(page, back).toHaveURL(mainProject);
		}
		expect(dialogs).toEqual([]);
		expect(
			visited.filter((url) => new URL(url).origin !== seed.console_url),
		).toEqual([]);
	});

	// Kills a way back to sign in or out, to a scheme that is not the web, or
	// outside the console and the public plot, however the server would decode it.
	for (const back of [
		"/%61uth/logout",
		"/auth%2flogout",
		"/auth%5Clogout",
		"/%2Fauth/logout",
		"/%2561uth/logout",
		`/%61uth/authorize?request=${REQUEST}`,
		`blob:${seed.console_url}/${REQUEST}`,
		"/x/..%2fauth/logout",
		"/x/..%5Cauth/logout",
		"/next/console/..%2f..%2fauth/logout",
		"/perf/..%2fauth/logout",
		"/perf/..%5Cauth/logout",
		"/perf/..%252fauth/logout",
		"/next/perf/%2e%2e%2fauth/logout",
		"/console/x%5Cy/..%2f..%2fauth/logout",
		"/perf/a%5Cb/..%2f..%2fauth/logout",
		"/console/x%5Cy/%2e%2e%2f%2e%2e%2fauth/logout",
		`/next/console/a%5Cb%5Cc/..%2f..%2f..%2fauth/authorize?request=${REQUEST}`,
		"/consoles/",
		"/docs/",
		"/pricing",
		"/explore",
	]) {
		test(`a way back to ${back} lands on the main project`, async ({
			page,
		}) => {
			await page.goto(logIn(back));
			await expect(page).toHaveURL(mainProject);
		});
	}

	// Kills a reader without a way back who lands anywhere but their main project.
	test("with no way back the reader lands on the main project", async ({
		page,
	}) => {
		await page.goto("/auth/login");
		await expect(page).toHaveURL(mainProject);
	});
});

test.describe("with a kept page", () => {
	test.use({
		storageState: signedIn(seed.admin, {
			[RETURN_KEY]: kept(alerts, Date.parse(seed.now)),
		}),
	});

	// Kills a confirm page that ignores the page another tab kept, or a kept
	// page used more than once.
	test("a signed in reader is returned to it once", async ({ page }) => {
		await page.goto("/auth/confirm");
		await expect(page).toHaveURL(absolute(alerts));
		await page.goto("/auth/confirm");
		await expect(page).toHaveURL(mainProject);
	});

	// Kills a provider callback opened while signed in that drops the kept page.
	test("a signed in provider callback returns to it", async ({ page }) => {
		await page.goto("/auth/github");
		await expect(page).toHaveURL(absolute(alerts));
		expect(await keptReturn(page)).toBeNull();
	});
});

test.describe("with a stale kept page", () => {
	test.use({
		storageState: signedIn(seed.admin, {
			[RETURN_KEY]: kept(alerts, Date.parse(seed.now) - TTL - 1),
		}),
	});

	// Kills a kept page that outlives the sign in that asked for it.
	test("a signed in reader lands on the main project", async ({ page }) => {
		await page.goto("/auth/confirm");
		await expect(page).toHaveURL(mainProject);
		expect(await keptReturn(page)).toBeNull();
	});
});
