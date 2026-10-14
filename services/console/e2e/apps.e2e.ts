import { createHash, randomBytes, randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import type { APIRequestContext, Page } from "@playwright/test";
import type {
	JsonAuthUser,
	JsonOAuthGrant,
	JsonUserKeyCreated,
} from "../src/types/bencher";
import { unique } from "./api";
import { VERSION_KEY, expect, seed, signedIn, test } from "./fixtures";

// The app the harness configures, which calls itself ChatGPT.
const CLIENT_ID = "https://client.example/oauth/client.json";
const CALLBACK = "https://client.example/callback";
const CORS = { "Access-Control-Allow-Origin": "*" };

const appsPath = (user: JsonAuthUser) =>
	`/console/users/${user.user.slug}/apps`;
const grantsUrl = (user: JsonAuthUser) =>
	`${seed.api_url}/v0/users/${user.user.slug}/oauth/grants`;
const heading = (page: Page) =>
	page.getByRole("heading", { level: 3, name: "Connected Apps" });

// Each test that connects an app does so for a user of its own, since every
// app the harness connects looks the same.
test.describe("connected apps", () => {
	// Kills a page that calls the wrong endpoint to revoke, revokes without
	// asking or on Cancel, offers a revoked app again while the list reloads,
	// keeps showing an app it revoked, or drops focus along the way.
	test("an app is listed, and revoking it removes it", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		const grant = await connectApp(request, user, user.token);
		const page = await (
			await newContext({ storageState: signedIn(user) })
		).newPage();
		await page.goto(appsPath(user));

		const app = page.getByRole("article", { name: "client.example" });
		await expect(app.getByRole("heading")).toHaveText("client.example");
		await expect(app.getByText("It calls itself “ChatGPT”.")).toBeVisible();
		await expect(app.getByRole("listitem")).toHaveText([
			"View your projects and their results, including private projects",
			"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts, and submit runs that create projects or claim unclaimed organizations",
		]);
		await expect(app.locator("time")).toHaveAttribute(
			"datetime",
			grant.created,
		);
		await expect(app.getByText("Last refreshed: Never")).toBeVisible();
		await expect(app.getByText("Approved: while logged in")).toBeVisible();

		const deletes: string[] = [];
		page.on("request", (request) => {
			if (request.method() === "DELETE") {
				deletes.push(request.url());
			}
		});
		const revoke = app.getByRole("button", { name: "Revoke client.example" });
		const cancel = app.getByRole("button", { name: "Cancel" });
		const sure = app.getByRole("button", { name: "I am 💯 sure" });
		await revoke.focus();
		await page.keyboard.press("Enter");
		await expect(app.getByText("Are you sure?")).toContainText(
			"client.example loses access",
		);
		await expect(cancel).toBeFocused();
		await page.keyboard.press("Enter");
		await expect(revoke).toBeFocused();
		await expect(sure).toHaveCount(0);
		expect(deletes).toEqual([]);

		// Hold the list the page reads after revoking, to see the row meanwhile.
		let release = () => {};
		const released = new Promise<void>((resolve) => {
			release = resolve;
		});
		await page.route(
			(url) => url.href.startsWith(`${grantsUrl(user)}?`),
			async (route) => {
				if (route.request().method() === "GET" && deletes.length > 0) {
					await released;
				}
				await route.fallback();
			},
		);
		await revoke.click();
		await sure.click();
		await expect.poll(() => deletes.length).toBe(1);
		await expect(sure).toBeDisabled();
		await expect(revoke).toHaveCount(0);
		release();

		await expect(app).toHaveCount(0);
		await expect(page.getByText("No connected apps")).toBeVisible();
		await expect(heading(page)).toBeFocused();
		await expect(
			page
				.getByRole("status")
				.filter({ hasText: "client.example was disconnected." }),
		).toBeVisible();
		expect(deletes).toEqual([`${grantsUrl(user)}/${grant.uuid}`]);
		await page.reload();
		await expect(page.getByText("No connected apps")).toBeVisible();
		await expect(app).toHaveCount(0);
	});

	// Kills a page that hides which key an app was approved with.
	test("an app approved with a key names the key", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		const name = unique("laptop");
		const key = await post<JsonUserKeyCreated>(
			request,
			`/v0/users/${user.user.slug}/keys`,
			user.token,
			{ name },
		);
		await connectApp(request, user, key.key);
		const page = await (
			await newContext({ storageState: signedIn(user) })
		).newPage();
		await page.goto(appsPath(user));

		const app = page.getByRole("article", { name: "client.example" });
		await expect(app).toContainText(
			`Approved: with the API key ${name}. Revoking or expiring that key disconnects the app.`,
		);
		await expect(app.getByRole("link", { name })).toHaveAttribute(
			"href",
			`/console/users/${user.user.slug}/keys/${key.uuid}`,
		);
	});

	// Kills a page that calls an app revoked elsewhere a failure and keeps showing it.
	test("an app already revoked elsewhere is just gone", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		const grant = await connectApp(request, user, user.token);
		const page = await (
			await newContext({ storageState: signedIn(user) })
		).newPage();
		await page.goto(appsPath(user));
		const app = page.getByRole("article", { name: "client.example" });
		await app.getByRole("button", { name: "Revoke client.example" }).click();

		const revoked = await request.delete(`${grantsUrl(user)}/${grant.uuid}`, {
			headers: { Authorization: `Bearer ${user.token}` },
		});
		expect(revoked.ok()).toBe(true);
		await app.getByRole("button", { name: "I am 💯 sure" }).click();

		await expect(page.getByText("No connected apps")).toBeVisible();
		await expect(page.getByText("Lettuce romaine calm!")).toHaveCount(0);
	});

	// Kills a new console account menu with no Connected Apps right after Keys,
	// or one that does not lead to the reader's apps.
	test("the new console's account menu leads to connected apps", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		await connectApp(request, user, user.token);
		const page = await (
			await newContext({
				storageState: signedIn(user, {
					[VERSION_KEY]: JSON.stringify({ [seed.projects.hashbrown.slug]: 1 }),
				}),
			})
		).newPage();
		await page.goto(`/console/users/${user.user.slug}/keys`);
		await expect(page).toHaveURL(`/next/console/users/${user.user.slug}/keys`);

		await page
			.getByRole("banner")
			.getByRole("button", { name: "Account" })
			.click();
		const menu = page.getByRole("menu", { name: "Account" });
		await expect(menu).toBeVisible();
		const names = (await menu.getByRole("menuitem").allInnerTexts()).map(
			(name) => name.trim(),
		);
		expect(names[names.indexOf("Keys") + 1]).toBe("Connected Apps");
		await menu.getByRole("menuitem", { name: "Connected Apps" }).click();

		await expect(page).toHaveURL(appsPath(user));
		await expect(
			page.getByRole("article", { name: "client.example" }),
		).toBeVisible();
	});
});

test.describe("as the member", () => {
	test.use({ storageState: signedIn(seed.member) });

	// Kills a page that shows a server without OAuth as a raw error.
	test("a server that does not connect apps says so", async ({ page }) => {
		await page.route(
			(url) => url.href.startsWith(`${grantsUrl(seed.member)}?`),
			(route) =>
				route.request().method() === "GET"
					? route.fulfill({
							status: 404,
							headers: CORS,
							json: { message: "Not Found" },
						})
					: route.fallback(),
		);
		await page.goto(appsPath(seed.member));
		await expect(
			page.getByText("Apps cannot connect to this Bencher server."),
		).toBeVisible();
		await expect(page.getByText("Not Found")).toHaveCount(0);
		await expect(page.getByText("Lettuce romaine calm!")).toHaveCount(0);
		await expect(page.getByRole("link", { name: "Log out" })).toHaveCount(0);
	});

	// Kills a classic settings menu, account menu, or shortcut that does not lead here.
	test("the user's menus lead to connected apps", async ({ page }) => {
		await page.goto(`/console/users/${seed.member.user.slug}/keys`);
		await page
			.getByRole("complementary")
			.getByRole("link", { name: "Connected Apps" })
			.click();
		await expect(page).toHaveURL(appsPath(seed.member));
		await expect(heading(page)).toBeVisible();
		await expect(
			page.locator(".navbar-dropdown").getByRole("link", {
				name: "Connected Apps",
				includeHidden: true,
			}),
		).toHaveAttribute("href", appsPath(seed.member));

		await page.goto("/console/users/apps");
		await expect(page).toHaveURL(appsPath(seed.member));
	});
});

test.describe("signed in with a user key", () => {
	test.use({ storageState: signedIn(seed.outsider) });

	// Kills a page that offers a key what only a logged in session may manage.
	test("connected apps need a session", async ({ page }) => {
		await page.goto(appsPath(seed.outsider));
		await expect(
			page.getByText(
				"Connected apps are managed only when you log in, not with an API key.",
			),
		).toBeVisible();
	});
});

/** A new user, signed in as the console stores them. */
const signUp = async (request: APIRequestContext): Promise<JsonAuthUser> => {
	const email = `${randomUUID().slice(0, 8)}@nowhere.com`;
	const signup = await request.post(`${seed.api_url}/v0/auth/signup`, {
		data: { name: "Le Quack", email, i_agree: true },
	});
	expect(signup.status()).toBe(202);
	let token: undefined | string;
	await expect
		.poll(() => {
			token = confirmTokens().find((jwt) => subject(jwt) === email);
			return token;
		})
		.toBeDefined();
	return post<JsonAuthUser>(request, "/v0/auth/confirm", "", { token });
};

/** Every login or signup token the API emailed, read from its log. */
const confirmTokens = () =>
	[
		...readFileSync(seed.api_log, "utf8").matchAll(
			/\/auth\/confirm\?[^\s"\\]*token=([\w-]+\.[\w-]+\.[\w-]+)/g,
		),
	].map((match) => match[1] ?? "");

const subject = (jwt: string) =>
	JSON.parse(Buffer.from(jwt.split(".")[1] ?? "", "base64url").toString()).sub;

/** Connect the app for a user with no other app, approving with `bearer`. */
const connectApp = async (
	request: APIRequestContext,
	user: JsonAuthUser,
	bearer: string,
) => {
	const verifier = randomBytes(32).toString("base64url");
	const authorize = await request.get(
		`${seed.api_url}/oauth/authorize?${new URLSearchParams({
			response_type: "code",
			client_id: CLIENT_ID,
			redirect_uri: CALLBACK,
			code_challenge: createHash("sha256").update(verifier).digest("base64url"),
			code_challenge_method: "S256",
			scope: "projects:read projects:write",
			state: randomUUID(),
		})}`,
		{ maxRedirects: 0 },
	);
	expect(authorize.status()).toBe(302);
	const uuid = new URL(authorize.headers().location ?? "").searchParams.get(
		"request",
	);
	const answer = await post<{ url: string }>(
		request,
		`/v0/oauth/authorizations/${uuid}`,
		bearer,
		{ approve: true },
	);
	const token = await request.post(`${seed.api_url}/oauth/token`, {
		form: {
			grant_type: "authorization_code",
			code: new URL(answer.url).searchParams.get("code") ?? "",
			client_id: CLIENT_ID,
			redirect_uri: CALLBACK,
			code_verifier: verifier,
		},
	});
	expect(token.status()).toBe(200);
	const grants = await request.get(grantsUrl(user), {
		headers: { Authorization: `Bearer ${user.token}` },
	});
	expect(grants.status()).toBe(200);
	const [grant] = (await grants.json()) as JsonOAuthGrant[];
	expect(grant).toBeDefined();
	return grant as JsonOAuthGrant;
};

const post = async <T>(
	request: APIRequestContext,
	path: string,
	token: string,
	data: unknown,
): Promise<T> => {
	const response = await request.post(`${seed.api_url}${path}`, {
		headers: token ? { Authorization: `Bearer ${token}` } : {},
		data,
	});
	if (!response.ok()) {
		throw new Error(
			`POST ${path} answered ${response.status()}: ${await response.text()}`,
		);
	}
	return (await response.json()) as T;
};
