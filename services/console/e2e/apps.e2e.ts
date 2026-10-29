import { createHash, randomBytes, randomUUID } from "node:crypto";
import { readFileSync } from "node:fs";
import type { APIRequestContext, Page } from "@playwright/test";
import type {
	JsonAuthUser,
	JsonOAuthGrant,
	JsonUserKeyCreated,
} from "../src/types/bencher";
import { unique } from "./api";
import { axeInBothThemes } from "./axe";
import { expect, seed, signedIn, test } from "./fixtures";

// The app the harness configures, which calls itself ChatGPT.
const CLIENT_ID = "https://client.example/oauth/client.json";
const CALLBACK = "https://client.example/callback";

const appsPath = (user: JsonAuthUser) =>
	`/console/users/${user.user.slug}/apps`;
const grantsUrl = (user: JsonAuthUser) =>
	`${seed.api_url}/v0/users/${user.user.slug}/oauth/grants`;

// Each test that connects an app does so for a user of its own, since every
// app the harness connects looks the same.
test.describe("connected apps", () => {
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
				storageState: signedIn(user),
			})
		).newPage();
		await page.goto(`/console/users/${user.user.slug}/keys`);
		await expect(page).toHaveURL(`/console/users/${user.user.slug}/keys`);

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
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});
		await menu.getByRole("menuitem", { name: "Connected Apps" }).click();

		await expect(page).toHaveURL(appsPath(user));
		await expect(
			page.getByRole("region", { name: "client.example" }),
		).toBeVisible();
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});
});

test.describe("the new console's Connected Apps", () => {
	const nextHeading = (page: Page) =>
		page.getByRole("heading", { level: 1, name: "Connected Apps" });

	// Kills a page that drops a field of an app, revokes without
	// asking or on Cancel, calls the wrong endpoint, or keeps showing an app
	// it revoked, before or after a reload.
	test("an app is listed, Cancel keeps it, and Revoke removes it", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		const grant = await connectApp(request, user, user.token);
		const page = await (
			await newContext({ storageState: signedIn(user) })
		).newPage();
		await page.goto(appsPath(user));

		const app = page.getByRole("region", { name: "client.example" });
		await expect(app.getByRole("heading", { level: 2 })).toHaveText(
			"client.example",
		);
		await expect(app).toContainText(/calls itself\s*ChatGPT/);
		await expect(app.getByRole("listitem")).toHaveText([
			"View your projects and their results, including private projects",
			"View, create, and change data in your projects, including private projects, such as reports, thresholds, and alerts, and submit runs that create projects or claim unclaimed organizations",
		]);
		await expect(app.locator("time")).toHaveAttribute(
			"datetime",
			grant.created,
		);
		await expect(app).toContainText(/refreshed\s*Never/);
		await expect(app).toContainText(/approved\s*while signed in/);

		const deletes: string[] = [];
		page.on("request", (sent) => {
			if (sent.method() === "DELETE") {
				deletes.push(sent.url());
			}
		});
		const revoke = app.getByRole("button", { name: "Revoke client.example" });
		await revoke.click();
		const confirm = page.getByRole("alertdialog", {
			name: "Revoke client.example?",
		});
		await expect(confirm).toContainText(
			"client.example loses access to this account at once.",
		);
		await confirm.getByRole("button", { name: "Cancel" }).click();
		await expect(confirm).toBeHidden();
		await expect(app).toBeVisible();
		expect(deletes).toEqual([]);

		await revoke.click();
		await confirm.getByRole("button", { name: "Revoke app" }).click();
		await expect(app).toHaveCount(0);
		await expect(page.getByText("No connected apps")).toBeVisible();
		await expect(nextHeading(page)).toBeFocused();
		await expect(page.getByRole("main").getByRole("status")).toHaveText(
			"Revoked client.example",
		);
		expect(deletes).toEqual([`${grantsUrl(user)}/${grant.uuid}`]);

		await page.reload();
		await expect(page.getByText("No connected apps")).toBeVisible();
		await expect(app).toHaveCount(0);
	});

	// Kills an app approved with a key that hides the key, or a key link that
	// leaves the app or leads anywhere but the reader's Keys.
	test("an app approved with a key names it and leads to Keys", async ({
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

		const app = page.getByRole("region", { name: "client.example" });
		await expect(app).toContainText(new RegExp(`approved\\s*with key ${name}`));
		await page.evaluate(() => {
			(window as Window & { stayed?: boolean }).stayed = true;
		});
		await app.getByRole("link", { name }).click();

		await expect(page).toHaveURL(`/console/users/${user.user.slug}/keys`);
		await expect(
			page
				.getByRole("table", { name: "Active keys" })
				.getByRole("row", { name: new RegExp(name) }),
		).toBeVisible();
		expect(
			await page.evaluate(
				() => (window as Window & { stayed?: boolean }).stayed,
			),
		).toBe(true);
	});

	test.describe("as the member", () => {
		test.use({ storageState: signedIn(seed.member) });

		// Kills a rail with no Connected Apps after Keys, one that leaves the
		// app, a shortcut that does not find the reader, and a shortcut that
		// stays in the history for Back to return to.
		test("the rail and the shortcut lead here", async ({ page }) => {
			const keys = `/console/users/${seed.member.user.slug}/keys`;
			await page.goto(keys);
			const rail = page.getByRole("navigation", { name: "Account" });
			const sections = (await rail.getByRole("link").allInnerTexts()).map(
				(name) => name.trim(),
			);
			expect(sections[sections.indexOf("Keys") + 1]).toBe("Connected Apps");
			await page.evaluate(() => {
				(window as Window & { stayed?: boolean }).stayed = true;
			});
			await rail.getByRole("link", { name: "Connected Apps" }).click();

			await expect(page).toHaveURL(appsPath(seed.member));
			await expect(nextHeading(page)).toBeVisible();
			await expect(
				rail.getByRole("link", { name: "Connected Apps" }),
			).toHaveAttribute("aria-current", "page");
			expect(
				await page.evaluate(
					() => (window as Window & { stayed?: boolean }).stayed,
				),
			).toBe(true);

			await page.goto(keys);
			await page.goto("/console/users/apps");
			await expect(page).toHaveURL(appsPath(seed.member));
			await expect(nextHeading(page)).toBeVisible();
			await expect(page.getByText("No connected apps")).toBeVisible();

			await page.goBack();
			await expect(page).toHaveURL(keys);
			await expect(
				page.getByRole("heading", { level: 1, name: "Keys" }),
			).toBeVisible();
		});
	});

	test.describe("signed in with a user key", () => {
		test.use({ storageState: signedIn(seed.outsider) });

		// Kills a page that offers a key what only a signed in session may
		// manage, or offers a Retry that can never work.
		test("connected apps need a session", async ({ page }) => {
			await page.goto(appsPath(seed.outsider));
			await expect(
				page.getByText("Not available with an API key"),
			).toBeVisible();
			await expect(page.getByRole("button", { name: "Retry" })).toHaveCount(0);
		});
	});

	// Kills contrast, names, and roles that fail axe in either theme.
	test("the page passes axe in both themes", async ({
		newContext,
		request,
	}) => {
		const user = await signUp(request);
		await connectApp(request, user, user.token);
		const page = await (
			await newContext({ storageState: signedIn(user) })
		).newPage();
		await axeInBothThemes(page, appsPath(user), () =>
			expect(
				page.getByRole("button", { name: "Revoke client.example" }),
			).toBeVisible(),
		);
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
