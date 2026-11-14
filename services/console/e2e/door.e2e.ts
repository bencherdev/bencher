import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import { axeInBothThemes } from "./axe";
import { continueWithEmail, digit, pasteCode, typeCode } from "./door";
import {
	READER_KEY,
	SESSION_COOKIE,
	ended,
	TURNSTILE,
	TURNSTILE_REFUSES,
	expect,
	seed,
	signedIn,
	signedOut,
	test,
	RELAY,
} from "./fixtures";
import { mailMark, mailedSignIn } from "./mail";
import { fromConsole, inSession, signUp } from "./session";

const newEmail = () => `${randomUUID().slice(0, 8)}@nowhere.com`;
const heading = (page: Page, name: string) =>
	page.getByRole("heading", { level: 1, name });
const hint = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), READER_KEY);

/** Shaped as an invitation, for the door to read its address; the API never sees it. */
const inviteFor = (email: string) =>
	[
		{ alg: "HS256", typ: "JWT" },
		{ aud: "invite", sub: email },
	]
		.map((part) => Buffer.from(JSON.stringify(part)).toString("base64url"))
		.concat("c2ln")
		.join(".");

/** Six digits that are not `code`, the `n`th of such. */
const wrongCode = (code: string, n: number) =>
	String((Number(code) + n) % 1_000_000).padStart(6, "0");

/** Runs in the page: the current time is `now`, as the suite's frozen date is, but later. */
const dateAt = (now: number) => {
	const before = Date;
	globalThis.Date = new Proxy(before, {
		construct: (target, args, newTarget) =>
			Reflect.construct(target, args.length > 0 ? args : [now], newTarget),
		apply: () => new before(now).toString(),
		get: (target, property, receiver) =>
			property === "now" ? () => now : Reflect.get(target, property, receiver),
	});
};

/** Every request URL the page sends from now on. */
const sentUrls = (page: Page) => {
	const urls: string[] = [];
	page.on("request", (sent) => urls.push(sent.url()));
	return urls;
};

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a door that draws a provider the server lacks, hides the one it
	// has, or names the wrong page.
	test("each door draws the server's providers and the email", async ({
		page,
	}) => {
		for (const [path, name] of [
			["/auth/login", "Sign in to Bencher"],
			["/auth/signup", "Sign up for Bencher"],
		] as const) {
			await page.goto(path);
			await expect(heading(page, name)).toBeVisible();
			await expect(
				page.getByRole("button", { name: "Continue with GitHub" }),
			).toBeVisible();
			await expect(
				page.getByRole("button", { name: "Continue with Google" }),
			).toHaveCount(0);
			await expect(page.getByRole("textbox", { name: "Email" })).toBeVisible();
			await expect(
				page.getByRole("link", { name: "Terms of Use" }),
			).toBeVisible();
		}
	});

	// Kills an address typed before the page's script ran that the script
	// throws away, or the focus it takes from the field.
	test("an address typed before the script runs stays", async ({ page }) => {
		let release = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route(
			(url) =>
				url.origin === seed.console_url && /\.(m?js)$/.test(url.pathname),
			async (route) => {
				await held;
				await route.continue();
			},
		);
		await page.goto("/auth/login", { waitUntil: "commit" });
		const field = page.getByRole("textbox", { name: "Email" });
		await field.fill("ada@example.com");
		release();
		await page.waitForLoadState();
		await expect(
			page.getByRole("button", { name: "Continue with GitHub" }),
		).toBeEnabled();
		await expect(field).toHaveValue("ada@example.com");
		await expect(field).toBeFocused();
	});

	// Kills a press that beats the page's script and sends the address in the
	// URL of the request it makes.
	test("a press before the script runs keeps the address out of every URL", async ({
		page,
	}) => {
		const email = newEmail();
		await page.route(
			(url) =>
				url.origin === seed.console_url && /\.(m?js)$/.test(url.pathname),
			() => {},
		);
		await page.goto("/auth/login", { waitUntil: "commit" });
		await page.getByRole("textbox", { name: "Email" }).fill(email);
		const sent = sentUrls(page);
		const submitted = page.waitForRequest((request) =>
			request.isNavigationRequest(),
		);
		await page.getByRole("button", { name: "Continue with email" }).click();
		const post = await submitted;
		expect(post.method()).toBe("POST");
		expect((await post.response())?.status()).toBe(200);
		expect(
			sent.filter(
				(url) => url.includes(email) || url.includes(encodeURIComponent(email)),
			),
		).toEqual([]);
	});

	// Kills an invite whose address the reader must type again.
	test("an invite's address is filled in", async ({ page }) => {
		const email = newEmail();
		await page.goto(
			`/auth/signup?${new URLSearchParams({ invite: inviteFor(email) })}`,
		);
		await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
			email,
		);
	});

	// Kills a refused check that sends anyway, says nothing, or leaves no way on.
	test("a refused Turnstile check is the error banner, and Try again goes on", async ({
		page,
	}) => {
		const sent = sentUrls(page);
		await page.goto("/auth/login");
		await page.evaluate((key) => {
			Reflect.set(window, key, true);
		}, TURNSTILE_REFUSES);
		await page.getByRole("textbox", { name: "Email" }).fill(newEmail());
		await page.getByRole("button", { name: "Continue with email" }).click();
		await expect(page.getByRole("alert")).toHaveText(/Verification failed/);
		expect(
			sent.filter((url) => url.startsWith(`${RELAY}/v0/auth/email`)),
		).toEqual([]);

		await page.evaluate((key) => {
			Reflect.deleteProperty(window, key);
		}, TURNSTILE_REFUSES);
		await page.getByRole("button", { name: "Try again" }).click();
		await expect(heading(page, "Check your inbox")).toBeVisible();
	});

	// Kills a sign up that never lands, lands a new account anywhere but
	// onboarding, keeps the session where a script can read it, or sets a cookie
	// a script can read.
	test("a sign up by code lands a new account on onboarding, with the session in an HttpOnly cookie alone", async ({
		page,
		context,
	}) => {
		const email = newEmail();
		await page.goto("/auth/signup");
		const { code } = await continueWithEmail(page, email);
		await pasteCode(page, code);

		await expect(page).toHaveURL(/\/console\/onboard\//);
		const cookie = (await context.cookies()).find(
			({ name }) => name === SESSION_COOKIE,
		);
		expect(cookie).toMatchObject({ httpOnly: true, secure: true });
		const session = cookie?.value ?? "";
		expect(session).not.toBe("");
		const held = await page.evaluate(async () => {
			const stores = [
				JSON.stringify({ ...localStorage }),
				JSON.stringify({ ...sessionStorage }),
				document.cookie,
			];
			for (const { name } of await indexedDB.databases()) {
				if (!name) {
					continue;
				}
				const db = await new Promise<IDBDatabase>((resolve, reject) => {
					const open = indexedDB.open(name);
					open.onsuccess = () => resolve(open.result);
					open.onerror = () => reject(open.error);
				});
				for (const store of db.objectStoreNames) {
					const all = await new Promise<unknown[]>((resolve) => {
						const read = db.transaction(store).objectStore(store).getAll();
						read.onsuccess = () => resolve(read.result);
						read.onerror = () => resolve([]);
					});
					stores.push(JSON.stringify(all));
				}
				db.close();
			}
			for (const name of Object.getOwnPropertyNames(window)) {
				try {
					stores.push(JSON.stringify(Reflect.get(window, name)) ?? "");
				} catch {
					// Not data.
				}
			}
			return stores;
		});
		expect(held.filter((store) => store.includes(session))).toEqual([]);
		expect(JSON.parse((await hint(page)) ?? "{}")).toEqual({
			uuid: expect.any(String),
			slug: expect.any(String),
			name: expect.any(String),
		});
	});

	// Kills a wrong code taken, or not said.
	test("a wrong code says so, and the right one still signs in", async ({
		page,
	}) => {
		const { user } = await signUp();
		await page.goto("/auth/login");
		const { code } = await continueWithEmail(page, user.email);
		await typeCode(page, code === "000000" ? "000001" : "000000");
		await expect(page.getByRole("alert")).toHaveText("Wrong code");
		await expect(digit(page, 1)).toBeFocused();
		await pasteCode(page, code);
		await expect(page).toHaveURL(/\/console/);
	});

	// Kills a Use another email that keeps the sign-in alive, or makes the
	// reader type the address again.
	test("Use another email ends the sign-in and returns to the door with the address selected", async ({
		page,
		context,
	}) => {
		const email = newEmail();
		await page.goto("/auth/login");
		const { code } = await continueWithEmail(page, email);
		const pending = (await context.cookies()).find(
			({ name }) => name === "__Host-bencher_pending",
		);
		expect(pending).toBeDefined();
		await page.getByRole("button", { name: "Use another email" }).click();

		await expect(page).toHaveURL("/auth/login");
		const field = page.getByRole("textbox", { name: "Email" });
		await expect(field).toHaveValue(email);
		await expect(field).toBeFocused();
		await page.keyboard.type("x@nowhere.com");
		await expect(field).toHaveValue("x@nowhere.com");
		const proved = await fromConsole("POST", "/v0/auth/code", {
			cookie: ["__Host-bencher_pending", pending?.value ?? ""],
			body: { code },
		});
		expect(proved.status).toBe(401);
	});

	// Kills a link page that leaves its secret in the address while it
	// redeems it, or sends it in any request's URL.
	test("a link in the browser that asked signs in, its secret never in an address", async ({
		page,
	}) => {
		const email = newEmail();
		await page.goto("/auth/login");
		const { link } = await continueWithEmail(page, email);
		const secret = link.hash.slice(1);
		expect(secret).not.toBe("");
		// The email's button opens a tab of its own.
		const tab = await page.context().newPage();
		const sent = sentUrls(tab);
		let release = () => {};
		const held = new Promise<void>((resolve) => {
			release = resolve;
		});
		await tab.route(`${RELAY}/v0/auth/link`, async (route) => {
			await held;
			await route.continue();
		});
		await tab.goto(link.href, { waitUntil: "commit" });
		await tab.waitForFunction(() => document.readyState !== "loading");
		await expect(tab).toHaveURL("/auth/confirm");
		release();

		await expect(tab).toHaveURL(/\/console\//);
		expect(sent.filter((url) => url.includes(secret))).toEqual([]);
		expect(await hint(tab)).not.toBeNull();
	});

	// Kills a link pasted into the tab already on Check your inbox, which
	// changes only the address's fragment, left unread.
	test("a link pasted into the tab on Check your inbox signs in", async ({
		page,
	}) => {
		await page.goto("/auth/login");
		const { link } = await continueWithEmail(page, newEmail());
		await page.goto(link.href);
		await expect(page).toHaveURL(/\/console\/onboard\//);
	});

	// Kills a link sent before this release read as a sign in, or left in
	// the address.
	test("an old link with a token is expired", async ({ page }) => {
		const email = newEmail();
		await page.goto(
			`/auth/confirm?${new URLSearchParams({ token: inviteFor(email) })}`,
		);
		await expect(heading(page, "Check your inbox")).toBeVisible();
		await expect(page.getByRole("alert")).toHaveText("Expired");
		await expect(page.getByText(email)).toBeVisible();
		await expect(page).toHaveURL("/auth/confirm");
	});
});

test.describe("signed in to a session that ended", () => {
	test.use({ storageState: signedIn(ended(seed.member)) });

	// Kills a door that sends a reader whose session ended on to where a
	// sign-in goes, which sends them back to sign in, or keeps their hint.
	test("the door forgets the hint and stays", async ({ page }) => {
		const documents: string[] = [];
		page.on("request", (sent) => {
			if (sent.resourceType() === "document") {
				documents.push(new URL(sent.url()).pathname);
			}
		});
		const path = `/auth/login?authorization=${randomUUID()}`;
		await page.goto(path);
		await expect.poll(() => hint(page)).toBeNull();
		await page.waitForLoadState("networkidle");
		await expect(page).toHaveURL(path);
		await expect(heading(page, "Sign in to Bencher")).toBeVisible();
		expect(documents).toEqual(["/auth/login"]);
	});
});

test.describe("Resend after a sign-in ended", () => {
	test.use({ storageState: signedOut });

	// Kills a Resend from No tries left that asks only for the sign-in that
	// ended, which strands the reader on Sign-in failed, instead of starting
	// another for the same address.
	test("after the last try, Resend starts again and its code signs in", async ({
		page,
	}) => {
		const email = newEmail();
		await page.goto("/auth/signup");
		const { code } = await continueWithEmail(page, email);
		// Resend opens a minute after the email, and the suite's date stands
		// still, so the reader comes back two minutes later.
		await page.addInitScript(dateAt, Date.parse(seed.now) + 120_000);
		await page.reload();
		await expect(heading(page, "Check your inbox")).toBeVisible();
		for (let n = 1; n <= 5; n++) {
			const answered = page.waitForResponse((response) =>
				response.url().endsWith("/v0/auth/code"),
			);
			await typeCode(page, wrongCode(code, n));
			expect((await answered).status()).toBe(403);
		}
		await expect(page.getByRole("alert")).toHaveText("No tries left");
		await expect(digit(page, 1)).toBeDisabled();

		const mark = mailMark();
		await page.getByRole("button", { name: "Resend", exact: true }).click();
		await expect(page.getByRole("status")).toHaveText("Sent");
		const { code: next } = await mailedSignIn(email, mark);
		await typeCode(page, next);
		await expect(page).toHaveURL(/\/console\//);
	});

	// Kills an old link's Expired whose Resend asks only for a sign-in this
	// browser never began, instead of starting one for the link's address.
	test("an old link's Resend starts a sign-in whose code signs in", async ({
		page,
	}) => {
		const email = newEmail();
		await page.goto(
			`/auth/confirm?${new URLSearchParams({ token: inviteFor(email) })}`,
		);
		await expect(page.getByRole("alert")).toHaveText("Expired");

		const mark = mailMark();
		await page.getByRole("button", { name: "Resend", exact: true }).click();
		await expect(page.getByRole("status")).toHaveText("Sent");
		const { code } = await mailedSignIn(email, mark);
		await typeCode(page, code);
		await expect(page).toHaveURL(/\/console\//);
	});
});

// Kills a sign out that leaves the session alive, its hint, or the browser
// anywhere but the door.
test("/auth/logout ends the session and lands on the door", async ({
	newContext,
}) => {
	const reader = await signUp();
	const page = await (
		await newContext({ storageState: signedIn(reader) })
	).newPage();
	await page.goto("/auth/logout");
	await expect(page).toHaveURL("/auth/login");
	await expect(heading(page, "Sign in to Bencher")).toBeVisible();
	expect(await hint(page)).toBeNull();
	expect((await inSession(reader, "GET", "/v0/auth/session")).status).toBe(401);
});

// Kills an auth page a referrer or a cache can carry, one that loads Google
// Analytics, and Turnstile loaded anywhere but the door and Check your inbox.
test("auth pages send no referrer, are never stored, and load Turnstile only where a sign-in starts", async ({
	context,
	request,
}) => {
	const email = newEmail();
	const pages = [
		["/auth/login", true],
		["/auth/signup", true],
		["/auth/authorize?request=0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f", false],
		["/auth/email?user=x&token=y", false],
		["/auth/github", false],
		["/auth/google", false],
		["/auth/logout", false],
		[`/auth/confirm?${new URLSearchParams({ token: inviteFor(email) })}`, true],
		["/auth/confirm#s3cr3t", false],
	] as const;
	for (const [path] of [...pages, ["/auth"]] as const) {
		const response = await request.get(path, { maxRedirects: 0 });
		expect(response.status(), path).toBeLessThan(400);
		expect(response.headers()["referrer-policy"], path).toBe("no-referrer");
		expect(response.headers()["cache-control"], path).toBe("no-store");
	}
	// A page that sends the reader on to the door lands on a page of no scripts,
	// so whatever loads came from the page itself.
	const door = (url: URL) => url.pathname === "/auth/login";
	for (const [path, turnstile] of pages) {
		// A page of its own, so nothing an earlier page loaded is read as this one's.
		const page = await context.newPage();
		const loaded: string[] = [];
		page.on("request", (sent) => loaded.push(sent.url()));
		if (path !== "/auth/login") {
			await page.route(door, (route) =>
				route.fulfill({ contentType: "text/html", body: "<h1>Door</h1>" }),
			);
		}
		await page.goto(path);
		await page.waitForLoadState("networkidle");
		await page.close();
		const third = loaded.filter((url) => !url.startsWith(seed.console_url));
		expect(
			third.some((url) => url.startsWith(TURNSTILE.replace("/**", ""))),
			path,
		).toBe(turnstile);
		expect(
			third.filter((url) =>
				/google-analytics\.com|googletagmanager\.com|analytics\.google\.com/.test(
					url,
				),
			),
			path,
		).toEqual([]);
	}
});

test.describe("accessibility", () => {
	test.use({ storageState: signedOut });

	// Kills contrast, names, and roles that fail axe in either theme.
	test("the door passes axe in both themes", async ({ page }) => {
		await axeInBothThemes(page, "/auth/signup", () =>
			expect(heading(page, "Sign up for Bencher")).toBeVisible(),
		);
	});

	test("Check your inbox, with a wrong code, passes axe in both themes", async ({
		page,
	}) => {
		const { user } = await signUp();
		await page.goto("/auth/login");
		const { code } = await continueWithEmail(page, user.email);
		const wrong = code === "000000" ? "000001" : "000000";
		await axeInBothThemes(
			page,
			"/auth/confirm",
			() => expect(page.getByRole("alert")).toHaveText("Wrong code"),
			async () => {
				await expect(heading(page, "Check your inbox")).toBeVisible();
				await typeCode(page, wrong);
			},
		);
	});

	test("a provider's return from another browser passes axe in both themes", async ({
		page,
	}) => {
		// Each scan returns afresh: the page took the code out of its URL.
		await axeInBothThemes(
			page,
			"/auth/github?code=c0de&state=s7a7e",
			() =>
				expect(
					page.getByRole("button", { name: "Continue with GitHub" }),
				).toBeVisible(),
			async () => {},
		);
	});

	test("a new email's confirm passes axe in both themes", async ({ page }) => {
		const params = new URLSearchParams({
			user: seed.member.user.uuid,
			token: inviteFor(seed.member.user.email),
		});
		// Each scan returns afresh: the page took the user and the token out of its URL.
		await axeInBothThemes(
			page,
			`/auth/email?${params}`,
			() =>
				expect(
					page.getByRole("button", { name: "Change email" }),
				).toBeVisible(),
			async () => {},
		);
	});

	test("Sign in as passes axe in both themes", async ({ page, newContext }) => {
		const { user } = await signUp();
		const asker = await (await newContext()).newPage();
		await asker.goto("/auth/login");
		const { link } = await continueWithEmail(asker, user.email);
		await axeInBothThemes(
			page,
			link.href,
			() =>
				expect(
					page.getByRole("button", { name: `Sign in as ${user.email}` }),
				).toBeVisible(),
			async () => {},
		);
	});
});
