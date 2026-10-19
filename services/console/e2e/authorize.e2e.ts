import { randomUUID } from "node:crypto";
import type { APIRequestContext, Page } from "@playwright/test";
import {
	USER_KEY,
	expect,
	nextPath,
	seed,
	signedIn,
	signedOut,
	test,
} from "./fixtures";
import { finishGitHub, startGitHub } from "./github";
import { mailedLink } from "./mail";

// The app the harness configures: it calls itself ChatGPT, but it is served from
// client.example, and the browser never really reaches its redirects.
const CLIENT_ID = "https://client.example/oauth/client.json";
const CALLBACK = "https://client.example/callback";
// Loopback redirects match on any port.
const LOOPBACK = "http://127.0.0.1:61017/callback";
const CHALLENGE = "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM";
const CONSENT = "/auth/authorize";

const PROJECT_SCOPES = "projects:read projects:write";
// A page of the console that a sign in would return to, but for the request.
const wayBack = Buffer.from(nextPath(seed.projects.hashbrown.slug)).toString(
	"base64",
);
const keptReturn = (page: Page) =>
	page.evaluate(() => localStorage.getItem("BENCHER_RETURN"));

const authorizeUrl = (redirectUri: string, state: string, scope: string) =>
	`${seed.api_url}/oauth/authorize?${new URLSearchParams({
		response_type: "code",
		client_id: CLIENT_ID,
		redirect_uri: redirectUri,
		code_challenge: CHALLENGE,
		code_challenge_method: "S256",
		scope,
		state,
	})}`;

/** Start an app's request to connect, as its browser would, landing on the consent page. */
const connect = async (
	page: Page,
	redirectUri = CALLBACK,
	scope = PROJECT_SCOPES,
) => {
	const state = randomUUID();
	await page.route(`${new URL(redirectUri).origin}/**`, (route) =>
		route.fulfill({ contentType: "text/plain", body: "Connected" }),
	);
	await page.goto(authorizeUrl(redirectUri, state, scope));
	await expect(page).toHaveURL(
		new RegExp(`${CONSENT}\\?request=[0-9a-f-]{36}$`),
	);
	const uuid = new URL(page.url()).searchParams.get("request") ?? "";
	return { uuid, state };
};

/** The query the browser arrived at the app with. */
const arrived = async (page: Page, redirectUri = CALLBACK) => {
	await expect(page).toHaveURL(
		(url) => `${url.origin}${url.pathname}` === redirectUri,
	);
	return new URL(page.url()).searchParams;
};

const heading = (page: Page) => page.getByRole("heading", { level: 1 });
const approve = (page: Page) =>
	page.getByRole("button", { name: "Approve", exact: true });

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a consent page that sends the browser anywhere but the app's
	// registered redirect, or without the code and the app's own state.
	test("approving sends the browser to the app with a code", async ({
		page,
	}) => {
		const { state } = await connect(page);
		await expect(
			page.getByText(`You are signed in as ${seed.member.user.email}.`),
		).toBeVisible();
		await approve(page).click();

		const query = await arrived(page);
		expect(query.get("code")).toMatch(/^bencher_code_[A-Za-z0-9]{30}$/);
		expect(query.get("state")).toBe(state);
		expect(query.get("iss")).toBe(seed.api_url);
	});

	// Kills a page that can be framed to trick a click on Approve, or that hands
	// its address, the request's only capability, to another site.
	test("the page cannot be framed and keeps its address", async ({ page }) => {
		const thirdParty: string[] = [];
		page.on("request", (request) => {
			if (request.url().includes("googletagmanager")) {
				thirdParty.push(request.url());
			}
		});
		const { uuid } = await connect(page);
		const response = await page.goto(`${CONSENT}?request=${uuid}`);
		const headers = response?.headers() ?? {};
		expect(headers["content-security-policy"]).toContain(
			"frame-ancestors 'none'",
		);
		expect(headers["referrer-policy"]).toBe("no-referrer");
		expect(headers["cache-control"]).toBe("no-store");
		await expect(approve(page)).toBeVisible();
		expect(thirdParty).toEqual([]);
	});

	// Kills a page whose HTML repeats its address, the request's only capability,
	// anywhere but the island that answers it.
	test("the page repeats its request only to its island", async ({
		page,
		request,
	}) => {
		const { uuid } = await connect(page);
		const html = await (await request.get(`${CONSENT}?request=${uuid}`)).text();
		expect(html).toContain(uuid);
		expect(html.replace(/<astro-island[^>]*>/g, "")).not.toContain(uuid);
	});

	// Kills a page that names the app by the name it gives itself, hides what it
	// may do (writing reads too), or hides where the answer goes.
	test("the app is named by its host first", async ({ page }) => {
		await connect(page);
		await expect(heading(page)).toHaveText("Connect client.example to Bencher");
		await expect(page.getByText("It calls itself “ChatGPT”.")).toBeVisible();
		await expect(page.getByRole("listitem")).toHaveText([
			"View your projects and their results, including private projects",
			"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts, and submit runs that create projects or claim unclaimed organizations",
		]);
		await expect(page.getByText("After you answer, you go to")).toContainText(
			"client.example",
		);
		await expect(page.getByText("a program on this computer")).toHaveCount(0);
	});

	// Kills a page that shows an identity scope by its raw name, or not at all.
	test("identity scopes are said in plain words", async ({ page }) => {
		await connect(page, CALLBACK, "openid email profile projects:read");
		await expect(page.getByRole("listitem")).toHaveText([
			"View your email address and whether it is verified",
			"Know which Bencher account you are signed in as",
			"View your name and username",
			"View your projects and their results, including private projects",
		]);
	});

	// Kills a page that lets an answer go to a local program without warning.
	test("a loopback redirect is a program on this computer", async ({
		page,
	}) => {
		const { state } = await connect(page, LOOPBACK);
		await expect(page.getByText("a program on this computer")).toBeVisible();
		await approve(page).click();
		expect((await arrived(page, LOOPBACK)).get("state")).toBe(state);
	});

	// Kills a badge shown for an app the API does not vouch for, or never shown.
	test("the verified badge is the API's to give", async ({ page }) => {
		await connect(page);
		await expect(page.getByText("Verified", { exact: true })).toBeVisible();
		await expect(page.getByText("Bencher knows this app.")).toBeVisible();

		await page.route(
			`${seed.api_url}/v0/oauth/authorizations/*`,
			async (route) => {
				const response = await route.fetch();
				const json = await response.json();
				json.client.verified = false;
				await route.fulfill({ response, json });
			},
		);
		await page.reload();
		await expect(heading(page)).toHaveText("Connect client.example to Bencher");
		await expect(page.getByText("Verified", { exact: true })).toHaveCount(0);
		await expect(page.getByText("Bencher knows this app.")).toHaveCount(0);
	});

	// Kills a page that shows a request answered elsewhere as a raw error, or
	// offers to answer it again, whether it learns so loading or answering.
	test("a request answered elsewhere says so", async ({ page, request }) => {
		const { uuid } = await connect(page);
		await expect(approve(page)).toBeVisible();
		const answer = await request.post(
			`${seed.api_url}/v0/oauth/authorizations/${uuid}`,
			{
				headers: { Authorization: `Bearer ${seed.member.token}` },
				data: { approve: true },
			},
		);
		expect(answer.status()).toBe(200);
		const answered = page.getByText(
			"This request to connect was already answered.",
		);

		await approve(page).click();
		await expect(answered).toBeVisible();
		await expect(approve(page)).toHaveCount(0);
		await expect(heading(page)).toBeFocused();

		await page.reload();
		await expect(answered).toBeVisible();
		await expect(approve(page)).toHaveCount(0);
	});

	// Kills a signed in visit to sign in that drops the request for the console.
	test("signing in again returns to the request", async ({ page }) => {
		const { uuid } = await connect(page);
		await page.goto(`/auth/login?authorization=${uuid}`);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
	});

	// Kills a sign in that returns to the page asked for over a pending request,
	// or keeps that page for a later sign in.
	test("a pending request wins over the page asked for", async ({ page }) => {
		const { uuid } = await connect(page);
		await page.goto(
			`/auth/login?${new URLSearchParams({ authorization: uuid, back: wayBack })}`,
		);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		expect(await keptReturn(page)).toBeNull();
	});

	// Kills a different account that leaves the session behind, or leaves the
	// page for the console.
	test("a different account signs out and stays", async ({ page }) => {
		const { uuid } = await connect(page);
		await page.getByRole("button", { name: "Use a different account" }).click();

		await expect(page.getByRole("link", { name: "Log in" })).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Log in to approve" }),
		).toBeFocused();
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
	});
});

test.describe("with a badly signed token", () => {
	const [header, payload] = seed.member.token.split(".");
	test.use({
		storageState: signedIn({
			...seed.member,
			token: `${header}.${payload}.${"A".repeat(43)}`,
		}),
	});

	// Kills a refused session left signed in, or shown as a raw error.
	test("approving signs out and asks to log in again", async ({ page }) => {
		await connect(page);
		await approve(page).click();

		await expect(page.getByRole("alert")).toHaveText(
			"Your session ended. Log in again, or approve with a user API key.",
		);
		await expect(page.getByRole("link", { name: "Log in" })).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Log in to approve" }),
		).toBeFocused();
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
	});
});

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a denial that hands the app a code, or loses its state.
	test("denying sends the browser back without a code", async ({ page }) => {
		const { state } = await connect(page);
		await page.getByRole("button", { name: "Deny" }).click();

		const query = await arrived(page);
		expect(query.get("error")).toBe("access_denied");
		expect(query.get("state")).toBe(state);
		expect(query.get("iss")).toBe(seed.api_url);
		expect(query.get("code")).toBeNull();
	});

	// Kills a pasted key kept as a session, or anywhere else in the browser, or
	// refused for the spaces a paste carries.
	test("a pasted user key approves and is kept nowhere", async ({ page }) => {
		const { state } = await connect(page);
		const key = seed.outsider.token;
		const field = page.getByLabel("Or approve with a user API key");
		await expect(field).toHaveAccessibleDescription(
			/The key is sent once to approve and never stored\./,
		);
		await field.fill(`  ${key}  `);
		await page.getByRole("button", { name: "Approve with key" }).click();

		const query = await arrived(page);
		expect(query.get("code")).toMatch(/^bencher_code_/);
		expect(query.get("state")).toBe(state);

		// Back on the console in the same tab, nothing it keeps holds the key.
		await page.goto("/robots.txt");
		const kept = await page.evaluate(() => ({
			local: { ...localStorage },
			session: { ...sessionStorage },
		}));
		expect(JSON.stringify(kept)).not.toContain(key);
		expect(kept.local).not.toHaveProperty(USER_KEY);
	});

	// Kills a consent page whose error reports carry a pasted key or the
	// request's UUID. The console under test reports nowhere, so this reads the
	// processors the page gave the reporter and runs each on an event holding both.
	test("error reports filter out API keys and the request", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		await expect(page.getByRole("button", { name: "Deny" })).toBeVisible();

		const key = `bencher_user_${"a".repeat(30)}`;
		const reports = await page.evaluate(
			({ key, uuid }) => {
				type Event = Record<string, unknown>;
				type Processor = (event: Event, hint: object) => null | Event;
				type Scope = {
					getScopeData: () => { eventProcessors: Processor[] };
				};
				const event = () => ({
					message: `Bearer ${key}`,
					transaction: `GET /v0/oauth/authorizations/${uuid}`,
					request: {
						url: `${location.origin}/auth/authorize?request=${uuid.toUpperCase()}`,
						query_string: `request=${uuid}`,
					},
					breadcrumbs: [
						{
							category: "fetch",
							data: { url: `/v0/oauth/authorizations/${uuid}` },
						},
					],
				});
				const carrier =
					(globalThis as { __SENTRY__?: Record<string, unknown> }).__SENTRY__ ??
					{};
				return Object.values(carrier)
					.map(
						(sdk) =>
							(sdk as undefined | { defaultIsolationScope?: Scope })
								?.defaultIsolationScope,
					)
					.flatMap((scope) => scope?.getScopeData().eventProcessors ?? [])
					.map((processor) => {
						try {
							return JSON.stringify(processor(event(), {}));
						} catch {
							return undefined;
						}
					});
			},
			{ key, uuid },
		);
		const filtered = reports.filter((report) => report?.includes("[Filtered]"));
		expect(filtered).not.toEqual([]);
		for (const report of filtered) {
			expect(report).not.toContain(key);
			expect(report?.toLowerCase()).not.toContain(uuid);
		}
	});

	// Kills a page that sends a project key to approve, or refuses it without
	// saying why.
	test("a project key is refused before it is sent", async ({ page }) => {
		const answers: string[] = [];
		page.on("request", (request) => {
			if (
				request.method() === "POST" &&
				request.url().startsWith(`${seed.api_url}/v0/oauth/`)
			) {
				answers.push(request.url());
			}
		});
		const { uuid } = await connect(page);
		await page
			.getByLabel("Or approve with a user API key")
			.fill(`bencher_run_${"a".repeat(30)}`);
		await page.getByRole("button", { name: "Approve with key" }).click();

		await expect(
			page.getByText(
				"A project API key (bencher_run_) cannot approve a request to connect.",
				{ exact: false },
			),
		).toBeVisible();
		expect(answers).toEqual([]);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
	});

	// Kills a page that navigates, or offers an answer, for a request it cannot load.
	test("an unknown or malformed request is an error", async ({ page }) => {
		const reads: string[] = [];
		page.on("request", (request) => {
			if (request.url().startsWith(`${seed.api_url}/v0/oauth/`)) {
				reads.push(request.url());
			}
		});

		await page.goto(`${CONSENT}?request=not-a-uuid`);
		await expect(
			page.getByText("This link to connect an app is not valid."),
		).toBeVisible();
		expect(reads).toEqual([]);

		const unknown = randomUUID();
		await page.goto(`${CONSENT}?request=${unknown}`);
		await expect(
			page.getByText("This request to connect is no longer available.", {
				exact: false,
			}),
		).toBeVisible();
		expect(reads).toHaveLength(1);
		await expect(page).toHaveURL(`${CONSENT}?request=${unknown}`);
		await expect(page.getByRole("button")).toHaveCount(0);
		await expect(page.getByRole("link")).toHaveCount(0);
	});

	// Kills a page that shows the text of its link, or one text for every error.
	test("an error is told in the console's own words", async ({ page }) => {
		for (const [error, text] of [
			["invalid_request", "The app's request to connect was not valid."],
			[
				"invalid_client",
				"Bencher could not verify the app that asked to connect.",
			],
			[
				"temporarily_unavailable",
				"Bencher is too busy to take the app's request right now.",
			],
			[
				"invalid_redirect_uri",
				"The app asked to send you somewhere it has not registered.",
			],
			[
				"Sign in again at evil.example",
				"The app's request to connect could not be completed.",
			],
		] as const) {
			await page.goto(`${CONSENT}?${new URLSearchParams({ error })}`);
			await expect(page.getByRole("alert")).toContainText(text);
			await expect(page.getByText("evil.example")).toHaveCount(0);
		}
	});

	// Kills a sign up, or a switch between log in and sign up, that drops the request.
	test("signing up and switching keep the request", async ({ page }) => {
		const { uuid } = await connect(page);
		await page.getByRole("link", { name: "Sign up" }).click();
		await expect(page).toHaveURL(`/auth/signup?authorization=${uuid}`);
		await page.getByRole("link", { name: "log in", exact: true }).click();
		await expect(page).toHaveURL(`/auth/login?authorization=${uuid}`);
		await expect(
			page.getByRole("link", { name: "sign up", exact: true }),
		).toHaveAttribute("href", `/auth/signup?authorization=${uuid}`);
	});

	// Kills a sign in that drops the request on the way through its email, that
	// lands anywhere but the consent page once confirmed, or that asks again
	// because its email was typed in capitals.
	test("logging in by email returns to the request", async ({
		page,
		request,
	}) => {
		const { uuid, state, email } = await logInByEmail(page, request);
		const link = await mailedLink(email, "Confirm Bencher Login");
		expect(link.origin).toBe(seed.console_url);
		expect(link.pathname).toBe("/auth/confirm");
		expect([...link.searchParams.keys()]).toEqual(["token", "authorization"]);
		await page.goto(link.href);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		await expect(
			page.getByText(`You are signed in as ${email}.`),
		).toBeVisible();

		await approve(page).click();
		expect((await arrived(page)).get("state")).toBe(state);
	});

	// Kills a confirm link that signs in a browser that did not start the sign in,
	// so a link planted on someone else signs them in to its account unasked.
	test("a confirm link from another browser asks before signing in", async ({
		page,
		request,
		browser,
	}) => {
		const { uuid, email } = await logInByEmail(page, request);
		const link = await mailedLink(email, "Confirm Bencher Login");
		const other = await browser.newContext();
		const fresh = await other.newPage();
		await fresh.goto(link.href);

		const signIn = fresh.getByRole("button", { name: `Sign in as ${email}` });
		await expect(signIn).toBeVisible();
		await expect(fresh).toHaveURL((url) => url.pathname === "/auth/confirm");
		expect(
			await fresh.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
		await signIn.click();
		await expect(fresh).toHaveURL(`${CONSENT}?request=${uuid}`);
		await expect(
			fresh.getByText(`You are signed in as ${email}.`),
		).toBeVisible();
		await other.close();
	});

	// Kills a confirm link that signs in a tab that started signing in as someone
	// else, whatever email its address names, or a tab that still signs in unasked
	// once it has asked.
	test("a confirm link for another account asks before signing in", async ({
		page,
		request,
	}) => {
		const { email } = await logInByEmail(page, request);
		const theirs = await page.context().newPage();
		const { uuid: theirUuid } = await connect(theirs);
		await theirs.close();
		const theirEmail = `${randomUUID().slice(0, 8)}@nowhere.com`;
		const signup = await request.post(`${seed.api_url}/v0/auth/signup`, {
			data: {
				name: "Le Quack",
				email: theirEmail,
				i_agree: true,
				authorization: theirUuid,
			},
		});
		expect(signup.status()).toBe(202);
		const theirLink = await mailedLink(theirEmail, "Confirm Bencher Signup");
		theirLink.searchParams.set("email", email);
		await page.goto(theirLink.href);

		await expect(
			page.getByRole("button", { name: `Sign in as ${theirEmail}` }),
		).toBeVisible();
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();

		await page.goto((await mailedLink(email, "Confirm Bencher Login")).href);
		await expect(
			page.getByRole("button", { name: `Sign in as ${email}` }),
		).toBeVisible();
	});

	// Kills a provider sign in that drops the request on its way to the provider,
	// or logs where it goes.
	test("a provider sign in carries the request", async ({ page }) => {
		const logged: string[] = [];
		page.on("console", (message) => logged.push(message.text()));
		const { uuid } = await connect(page);
		const { started } = await startGitHub(
			page,
			`/auth/login?authorization=${uuid}`,
		);

		expect(started).toHaveLength(1);
		expect(new URL(started[0] ?? "").searchParams.get("authorization")).toBe(
			uuid,
		);
		expect(logged.filter((text) => text.includes("github.example"))).toEqual(
			[],
		);
	});

	// Kills a provider sign in that lands on the console instead of the request,
	// or that asks the browser that started it to sign in again.
	test("signing in with a provider returns to the request", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		const { state } = await startGitHub(
			page,
			`/auth/login?authorization=${uuid}`,
		);
		await finishGitHub(page, state, uuid);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		await expect(approve(page)).toBeVisible();
	});

	// Kills a provider sign in that returns to the page asked for over a pending
	// request, or keeps that page for a later sign in.
	test("signing in with a provider returns to the request over the page asked for", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		const { state } = await startGitHub(
			page,
			`/auth/login?${new URLSearchParams({ authorization: uuid, back: wayBack })}`,
		);
		await finishGitHub(page, state, uuid);
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		expect(await keptReturn(page)).toBeNull();
	});

	// Kills a provider callback that signs in a browser that did not start the sign in.
	test("a provider callback from another browser asks before signing in", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		await finishGitHub(page, randomUUID(), uuid);

		const signIn = page.getByRole("button", {
			name: `Sign in as ${seed.member.user.email}`,
		});
		await expect(signIn).toBeVisible();
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
		await signIn.click();
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		await expect(approve(page)).toBeVisible();
	});

	// Kills a provider callback that signs in a tab whose own provider sign in was
	// abandoned, whatever state the callback carries.
	test("an abandoned provider sign in asks before another callback", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		await startGitHub(page, `/auth/login?authorization=${uuid}`);
		await finishGitHub(page, randomUUID(), uuid);

		await expect(
			page.getByRole("button", {
				name: `Sign in as ${seed.member.user.email}`,
			}),
		).toBeVisible();
		expect(
			await page.evaluate((key) => localStorage.getItem(key), USER_KEY),
		).toBeNull();
	});
});

/** Ask to connect, then log in by email for the request, as far as the confirm page. */
const logInByEmail = async (page: Page, request: APIRequestContext) => {
	const email = `${randomUUID().slice(0, 8)}@nowhere.com`;
	const signup = await request.post(`${seed.api_url}/v0/auth/signup`, {
		data: { name: "Le Quack", email, i_agree: true },
	});
	expect(signup.status()).toBe(202);
	// The sign in forms ask for a reCAPTCHA token, which this API never checks.
	await page.route("https://www.google.com/recaptcha/**", (route) =>
		route.abort(),
	);

	const { uuid, state } = await connect(page);
	await page.getByRole("link", { name: "Log in" }).click();
	await expect(page).toHaveURL(`/auth/login?authorization=${uuid}`);
	// The form replaces its placeholder once it loads, so fill until it takes.
	// The API reads an email in capitals as the same email.
	const logIn = page.getByRole("button", { name: "Log in", exact: true });
	await expect(async () => {
		await page.getByPlaceholder("email@example.com").fill(email.toUpperCase());
		await expect(logIn).toBeEnabled({ timeout: 1_000 });
	}).toPass();
	await logIn.click();
	await expect(page).toHaveURL((url) => url.pathname === "/auth/confirm");
	expect(new URL(page.url()).searchParams.get("authorization")).toBe(uuid);
	return { uuid, state, email };
};
