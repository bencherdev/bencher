import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import { createReader } from "./api";
import { axeInBothThemes } from "./axe";
import { continueWithEmail, typeCode } from "./door";
import {
	READER_KEY,
	SESSION_COOKIE,
	sessionCookie,
	ended,
	expect,
	consolePath,
	seed,
	signedIn,
	signedOut,
	test,
	RELAY,
} from "./fixtures";
import { finishGitHub, startGitHub } from "./github";
import { inSession, signUp } from "./session";

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
const wayBack = Buffer.from(consolePath(seed.projects.hashbrown.slug)).toString(
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
const main = (page: Page) => page.getByRole("main");
const hint = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), READER_KEY);
const signedInAs = (page: Page, email: string) =>
	expect(main(page)).toContainText(new RegExp(`signed in as\\s*${email}`));

test.describe("signed in", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a consent page that sends the browser anywhere but the app's
	// registered redirect, or without the code and the app's own state.
	test("approving sends the browser to the app with a code", async ({
		page,
	}) => {
		const { state } = await connect(page);
		await signedInAs(page, seed.member.user.email);
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

	// Kills a page whose HTML repeats its address, the request's only
	// capability, where the page's script does not read it from the address.
	test("the page never repeats its request in its HTML", async ({
		page,
		request,
	}) => {
		const { uuid } = await connect(page);
		const html = await (await request.get(`${CONSENT}?request=${uuid}`)).text();
		expect(html.toLowerCase()).not.toContain(uuid);
	});

	// Kills a page that names the app by the name it gives itself, hides what it
	// may do (writing reads too), or hides where the answer goes.
	test("the app is named by its host first", async ({ page }) => {
		await connect(page);
		await expect(heading(page)).toHaveText("Connect client.example to Bencher");
		await expect(main(page)).toContainText(/calls itself\s*ChatGPT/);
		await expect(page.getByRole("listitem")).toHaveText([
			"View your projects and their results, including private projects",
			"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts, and submit runs that create projects or claim unclaimed organizations",
		]);
		await expect(main(page)).toContainText(/returns to\s*client\.example/);
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

	// Kills a page that hides where a loopback answer goes, or refuses one.
	test("a loopback redirect returns to its host", async ({ page }) => {
		const { state } = await connect(page, LOOPBACK);
		await expect(main(page)).toContainText(/returns to\s*127\.0\.0\.1/);
		await approve(page).click();
		expect((await arrived(page, LOOPBACK)).get("state")).toBe(state);
	});

	// Kills a badge shown for an app the API does not vouch for, or never shown.
	test("the verified badge is the API's to give", async ({ page }) => {
		await connect(page);
		await expect(page.getByText("Verified", { exact: true })).toBeVisible();

		await page.route(`${RELAY}/v0/oauth/authorizations/*`, async (route) => {
			const response = await route.fetch();
			const json = await response.json();
			json.client.verified = false;
			await route.fulfill({ response, json });
		});
		await page.reload();
		await expect(heading(page)).toHaveText("Connect client.example to Bencher");
		await expect(page.getByText("Verified", { exact: true })).toHaveCount(0);
	});

	// Kills a page that shows a request answered elsewhere as a raw error, or
	// offers to answer it again, whether it learns so loading or answering.
	test("a request answered elsewhere says so", async ({ page, request }) => {
		const { uuid } = await connect(page);
		await expect(approve(page)).toBeVisible();
		const answer = await request.post(
			`${seed.api_url}/v0/oauth/authorizations/${uuid}`,
			{
				headers: { Authorization: `Bearer ${seed.member.key}` },
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

	// Kills a session approval sent without the session's cookie.
	test("approving with the session binds the grant to the reader", async ({
		page,
	}) => {
		const sent: (string | undefined)[] = [];
		page.on("request", async (request) => {
			if (
				request.method() === "POST" &&
				request.url().startsWith(`${RELAY}/v0/oauth/authorizations/`)
			) {
				sent.push((await request.allHeaders()).cookie);
			}
		});
		await connect(page);
		await approve(page).click();
		await arrived(page);
		expect(sent).toHaveLength(1);
		expect(sent[0]).toContain(`${SESSION_COOKIE}=`);
	});
});

test.describe("signed in as a reader of the test's own", () => {
	// Kills a different account that leaves the session alive or its hint
	// behind, or leaves the page for the console.
	test("a different account signs out and stays", async ({ newContext }) => {
		const reader = await createReader();
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		const { uuid } = await connect(page);
		await page.getByRole("button", { name: "Use a different account" }).click();

		await expect(page.getByRole("link", { name: "Sign in" })).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Sign in to approve" }),
		).toBeFocused();
		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		expect(await hint(page)).toBeNull();
		expect((await inSession(reader, "GET", "/v0/auth/session")).status).toBe(
			401,
		);
	});

	// Kills a session that ended while the page was open left signed in, or
	// shown as a raw error.
	test("approving after the session ends asks to sign in again", async ({
		newContext,
	}) => {
		const reader = await createReader();
		const page = await (
			await newContext({ storageState: signedIn(reader) })
		).newPage();
		await connect(page);
		await signedInAs(page, reader.user.email);
		await inSession(reader, "DELETE", "/v0/auth/session");
		await approve(page).click();

		await expect(page.getByRole("alert")).toHaveText(
			"Your session ended. Sign in again, or approve with a user API key.",
		);
		await expect(page.getByRole("link", { name: "Sign in" })).toBeVisible();
		await expect(
			page.getByRole("heading", { name: "Sign in to approve" }),
		).toBeFocused();
		expect(await hint(page)).toBeNull();
	});
});

test.describe("signed in to a session that ended", () => {
	test.use({ storageState: signedIn(ended(seed.member)) });

	// Kills a consent page that offers Approve on a session the API has
	// ended, or keeps the reader's hint for it.
	test("the page asks to sign in and forgets the hint", async ({ page }) => {
		await connect(page);
		await expect(
			page.getByRole("heading", { name: "Sign in to approve" }),
		).toBeVisible();
		await expect(approve(page)).toHaveCount(0);
		await expect.poll(() => hint(page)).toBeNull();
	});
});

test.describe("signed out", () => {
	test.use({ storageState: signedOut });

	// Kills a denial that hands the app a code, loses its state, or goes with
	// a session cookie the browser holds.
	test("denying sends the browser back without a code", async ({ page }) => {
		const cookies: (string | undefined)[] = [];
		page.on("request", async (request) => {
			if (
				request.method() === "POST" &&
				request.url().startsWith(`${RELAY}/v0/oauth/authorizations/`)
			) {
				cookies.push((await request.allHeaders()).cookie);
			}
		});
		const { state } = await connect(page);
		await page.context().addCookies([sessionCookie(seed.member.session)]);
		await page.getByRole("button", { name: "Deny" }).click();

		const query = await arrived(page);
		expect(cookies).toEqual([undefined]);
		expect(query.get("error")).toBe("access_denied");
		expect(query.get("state")).toBe(state);
		expect(query.get("iss")).toBe(seed.api_url);
		expect(query.get("code")).toBeNull();
	});

	// Kills a pasted key kept as a session, or anywhere else in the browser,
	// sent anywhere but the decision's body, sent with a session cookie the
	// browser holds, or refused for the spaces a paste carries.
	test("a pasted user key approves and is kept nowhere", async ({ page }) => {
		const decisions: { headers: Record<string, string>; body: unknown }[] = [];
		page.on("request", async (request) => {
			if (
				request.method() === "POST" &&
				request.url().startsWith(`${RELAY}/v0/oauth/authorizations/`)
			) {
				decisions.push({
					headers: await request.allHeaders(),
					body: request.postDataJSON(),
				});
			}
		});
		const { state } = await connect(page);
		// A session signed in since the page drew, which a key's approval leaves out.
		await page.context().addCookies([sessionCookie(seed.member.session)]);
		const key = seed.outsider.key;
		await page.getByLabel("Or approve with a user API key").fill(`  ${key}  `);
		await page.getByRole("button", { name: "Approve with key" }).click();

		const query = await arrived(page);
		expect(query.get("code")).toMatch(/^bencher_code_/);
		expect(query.get("state")).toBe(state);
		expect(decisions).toHaveLength(1);
		expect(decisions[0]?.body).toEqual({ approve: true, key });
		expect(decisions[0]?.headers).not.toHaveProperty("authorization");
		expect(decisions[0]?.headers).not.toHaveProperty("cookie");

		// Back on the console in the same tab, nothing it keeps holds the key.
		await page.goto("/robots.txt");
		const kept = await page.evaluate(() => ({
			local: { ...localStorage },
			session: { ...sessionStorage },
		}));
		expect(JSON.stringify(kept)).not.toContain(key);
		expect(kept.local).not.toHaveProperty(READER_KEY);
	});

	// Kills a consent page whose error reports carry a pasted key or the
	// request's UUID. The console under test reports nowhere, so this reads the
	// filter the reporter was given once it loads, with the page idle, and runs
	// it on an event holding both.
	test("error reports filter out API keys and the request", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		await expect(page.getByRole("button", { name: "Deny" })).toBeVisible();

		const key = `bencher_user_${"a".repeat(30)}`;
		const report = () =>
			page.evaluate(
				({ key, uuid }) => {
					type Filter = (event: unknown, hint: object) => unknown;
					type Scope = {
						getClient: () =>
							| undefined
							| { getOptions: () => { beforeSend?: Filter } };
					};
					const carrier =
						(globalThis as { __SENTRY__?: Record<string, unknown> })
							.__SENTRY__ ?? {};
					const [filter] = Object.values(carrier).flatMap((sdk) => {
						const beforeSend = (
							sdk as undefined | { defaultCurrentScope?: Scope }
						)?.defaultCurrentScope
							?.getClient()
							?.getOptions().beforeSend;
						return beforeSend ? [beforeSend] : [];
					});
					return filter
						? JSON.stringify(
								filter(
									{
										message: `Bearer ${key} at ${uuid}`,
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
									},
									{},
								),
							)
						: undefined;
				},
				{ key, uuid },
			);
		await expect.poll(report, { timeout: 15_000 }).toContain("[Filtered]");
		const filtered = (await report()) ?? "";
		expect(filtered).not.toContain(key);
		expect(filtered.toLowerCase()).not.toContain(uuid);
	});

	// Kills a page that sends a project key to approve, or refuses it without
	// saying why.
	test("a project key is refused before it is sent", async ({ page }) => {
		const answers: string[] = [];
		page.on("request", (request) => {
			if (
				request.method() === "POST" &&
				request.url().startsWith(`${RELAY}/v0/oauth/`)
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
			if (request.url().startsWith(`${RELAY}/v0/oauth/`)) {
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
		await expect(main(page).getByRole("button")).toHaveCount(0);
		await expect(main(page).getByRole("link")).toHaveCount(0);
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

	// Kills a sign up that drops the request.
	test("signing up keeps the request", async ({ page }) => {
		const { uuid } = await connect(page);
		await page.getByRole("link", { name: "Sign up" }).click();
		await expect(page).toHaveURL(`/auth/signup?authorization=${uuid}`);
	});

	// Kills a sign in that drops the request on the way through its email, that
	// lands anywhere but the consent page once the code is in, or that asks
	// again because its email was typed in capitals.
	test("signing in by code returns to the request", async ({ page }) => {
		const { user } = await signUp();
		const { uuid, state } = await connect(page);
		await page.getByRole("link", { name: "Sign in" }).click();
		await expect(page).toHaveURL(`/auth/login?authorization=${uuid}`);
		// The API reads an email in capitals as the same email.
		const { code } = await continueWithEmail(page, user.email.toUpperCase());
		await typeCode(page, code);

		await expect(page).toHaveURL(`${CONSENT}?request=${uuid}`);
		await signedInAs(page, user.email);
		await approve(page).click();
		expect((await arrived(page)).get("state")).toBe(state);
	});

	// Kills a link that signs in a browser that did not start the sign in, so
	// a link planted on someone else signs them in to its account unasked, and
	// a link that drops the request.
	test("a link from another browser asks before signing in", async ({
		page,
		newContext,
	}) => {
		const { user } = await signUp();
		const { uuid } = await connect(page);
		await page.getByRole("link", { name: "Sign in" }).click();
		const { link } = await continueWithEmail(page, user.email);
		expect(link.origin).toBe(seed.console_url);
		expect(link.pathname).toBe("/auth/confirm");
		expect(link.search).toBe("");
		const fresh = await (await newContext()).newPage();
		await fresh.goto(link.href);

		const signIn = fresh.getByRole("button", {
			name: `Sign in as ${user.email}`,
		});
		await expect(signIn).toBeVisible();
		await expect(fresh).toHaveURL("/auth/confirm");
		expect(await hint(fresh)).toBeNull();
		await signIn.click();
		await expect(fresh).toHaveURL(`${CONSENT}?request=${uuid}`);
		await signedInAs(fresh, user.email);
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

	// Kills a provider callback that signs in a browser that did not start the
	// sign in, or drops the request once asked.
	test("a provider callback from another browser asks before signing in", async ({
		page,
	}) => {
		const { uuid } = await connect(page);
		await finishGitHub(page, randomUUID(), uuid);

		const signIn = page.getByRole("button", { name: "Continue with GitHub" });
		await expect(signIn).toBeVisible();
		expect(await hint(page)).toBeNull();
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
			page.getByRole("button", { name: "Continue with GitHub" }),
		).toBeVisible();
		expect(await hint(page)).toBeNull();
	});
});

test.describe("accessibility", () => {
	// Kills contrast, names, and roles that fail axe in either theme.
	for (const [who, storageState, ready] of [
		["signed in", signedIn(seed.member), (page: Page) => approve(page)],
		[
			"signed out",
			signedOut,
			(page: Page) => page.getByLabel("Or approve with a user API key"),
		],
	] as const) {
		test.describe(who, () => {
			test.use({ storageState });

			test(`the consent page ${who} passes axe in both themes`, async ({
				page,
			}) => {
				const { uuid } = await connect(page);
				await axeInBothThemes(page, `${CONSENT}?request=${uuid}`, () =>
					expect(ready(page)).toBeVisible(),
				);
			});
		});
	}
});
