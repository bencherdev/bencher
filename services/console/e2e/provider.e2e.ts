import type { Page } from "@playwright/test";
import type { JsonProviderSignIn } from "../src/types/bencher";
import { axeInBothThemes } from "./axe";
import { typeCode } from "./door";
import {
	READER_KEY,
	RELAY,
	expect,
	seed,
	sessionCookie,
	signedOut,
	test,
} from "./fixtures";
import { GITHUB_FINISH, GITHUB_START } from "./github";
import { mailMark, mailedCode } from "./mail";
import { fromConsole, signUp } from "./session";

const PENDING_COOKIE = "__Host-bencher_pending";

test.use({ storageState: signedOut });

const heading = (page: Page, name: string) =>
	page.getByRole("heading", { level: 1, name });
const hint = (page: Page) =>
	page.evaluate((key) => localStorage.getItem(key), READER_KEY);
/** Expect the reader's hint once the landing that signed in stops navigating. */
const expectHinted = (page: Page) =>
	expect(async () => expect(await hint(page)).not.toBeNull()).toPass();

/**
 * Answer the GitHub finish with `json`, once `held` settles, signed in as the
 * member when it signs in, and return what each finish sent.
 */
const finishWith = async (
	page: Page,
	json: JsonProviderSignIn,
	held: Promise<void> = Promise.resolve(),
) => {
	const sent: unknown[] = [];
	await page.route(GITHUB_FINISH, async (route) => {
		sent.push(route.request().postDataJSON());
		await held;
		if (json.signed_in) {
			await page.context().addCookies([sessionCookie(seed.member.session)]);
		}
		await route.fulfill({ json });
	});
	return sent;
};

const toProve = (email: string): JsonProviderSignIn => ({
	inbox: { handle: "ada-work", email, resend_after: 60 },
});

// Kills a provider callback that leaves the code or the state in the address
// while it finishes, or sends either in any request's URL.
test("a provider's return keeps its code and state out of every later address", async ({
	page,
}) => {
	const code = "c0de-in-the-address";
	const state = "s7a7e-in-the-address";
	const arrival = `/auth/github?${new URLSearchParams({ code, state })}`;
	const urls: string[] = [];
	page.on("request", (sent) => urls.push(sent.url()));
	let release = () => {};
	const held = new Promise<void>((resolve) => {
		release = resolve;
	});
	const sent = await finishWith(
		page,
		{ signed_in: { user: seed.member.user, new: false } },
		held,
	);

	await page.goto(arrival, { waitUntil: "commit" });
	await page.waitForFunction(() => document.readyState !== "loading");
	await expect(page).toHaveURL("/auth/github");
	await expect(page.getByRole("status")).toHaveText("Signing in with GitHub");
	release();

	await expect(page).toHaveURL(/\/console\//);
	expect(sent).toEqual([{ code, state }]);
	expect(
		urls.filter(
			(url) =>
				!url.endsWith(arrival) && (url.includes(code) || url.includes(state)),
		),
	).toEqual([]);
	await expectHinted(page);
});

// Every real return arrives by the provider's redirect, from another site.
// Kills a binding cookie that such an arrival keeps from the finish.
test("a return from the provider's site carries the binding cookie to the finish", async ({
	page,
	context,
}) => {
	await context.addCookies([
		{ ...sessionCookie("b1nd1ng-s3cr3t"), name: "__Host-bencher_oauth" },
	]);
	const finishes: Record<string, string>[] = [];
	await page.route(GITHUB_FINISH, async (route) => {
		finishes.push(await route.request().allHeaders());
		await route.fulfill({
			status: 401,
			json: { error_code: "provider_refused", message: "The sign-in failed" },
		});
	});
	await page.route("https://github.example/**", (route) =>
		route.fulfill({
			status: 302,
			headers: {
				location: `${seed.console_url}/auth/github?code=c0de&state=s7a7e`,
			},
		}),
	);

	await page.goto("https://github.example/login/oauth/authorize");
	await expect(page.getByRole("alert")).toHaveText("Sign-in failed");
	expect(finishes).toHaveLength(1);
	expect(finishes[0]?.cookie).toContain("__Host-bencher_oauth=b1nd1ng-s3cr3t");
});

/**
 * Continue with GitHub from the door at `door`, as far as the return, whose
 * finish asks `email` to be proven first.
 */
const continueToProve = async (page: Page, door: string, email: string) => {
	await finishWith(page, toProve(email));
	await page.route(GITHUB_START, (route) =>
		route.fulfill({
			json: { url: `${seed.console_url}/auth/github?code=c0de&state=s7a7e` },
		}),
	);
	await page.goto(door);
	await page.getByRole("button", { name: "Continue with GitHub" }).click();
	await expect(page).toHaveURL("/auth/confirm");
};

// Kills an account to prove that signs in, hides which provider account asks,
// loses it on a reload, or whose code does not sign in.
test("a provider account to prove goes on to Check your inbox with its handle, and the code signs in", async ({
	page,
	context,
}) => {
	const { user } = await signUp();
	const mark = mailMark();
	// The email a provider's sign-in starts, in this browser.
	const started = await fromConsole("POST", "/v0/auth/email", {
		body: { email: user.email },
	});
	expect(started.status).toBe(202);
	await context.addCookies([
		{
			...sessionCookie(started.cookies.get(PENDING_COOKIE) ?? ""),
			name: PENDING_COOKIE,
		},
	]);
	const code = await mailedCode(user.email, mark);

	await continueToProve(page, "/auth/login", user.email);
	await expect(heading(page, "Check your inbox")).toBeVisible();
	await expect(page.locator(".au-who")).toHaveText("ada-work");
	await expect(page.getByText(user.email)).toBeVisible();
	expect(await hint(page)).toBeNull();
	await page.reload();
	await expect(page.locator(".au-who")).toHaveText("ada-work");

	await typeCode(page, code);
	await expect(page).toHaveURL(/\/console\//);
	await expectHinted(page);
});

// Kills a provider's Check your inbox whose Use another email loses the door
// the sign-in began at, or what it carried.
test("Use another email returns to the door the provider's sign-in began at", async ({
	page,
}) => {
	await continueToProve(page, "/auth/signup?plan=pro", "ada@nowhere.com");
	await page.getByRole("button", { name: "Use another email" }).click();
	await expect(page).toHaveURL("/auth/signup?plan=pro");
	await expect(page.getByRole("textbox", { name: "Email" })).toHaveValue(
		"ada@nowhere.com",
	);
});

// Kills a provider's refusal that asks the API to finish, says nothing, or
// signs anything in; an API refusal that reads as the provider's; and one that
// does not say which.
test("a refusal by the provider or the API says so and signs nothing in", async ({
	page,
}) => {
	let refusal = { status: 401, error_code: "provider_refused" };
	const finishes: string[] = [];
	await page.route(`${RELAY}/v0/auth/providers/**`, (route) => {
		finishes.push(route.request().url());
		return route.fulfill({
			status: refusal.status,
			json: { error_code: refusal.error_code, message: "Refused" },
		});
	});

	await page.goto("/auth/google?error=access_denied&state=s7a7e");
	await expect(page.getByRole("alert")).toHaveText("Refused by Google");
	expect(finishes).toEqual([]);

	await page.goto("/auth/github?code=c0de&state=s7a7e");
	await expect(page.getByRole("alert")).toHaveText("Sign-in failed");
	refusal = { status: 403, error_code: "locked" };
	await page.goto("/auth/github?code=c0de&state=s7a7e");
	await expect(page.getByRole("alert")).toHaveText("This account is locked");
	expect(finishes).toEqual([
		`${RELAY}/v0/auth/providers/github/finish`,
		`${RELAY}/v0/auth/providers/github/finish`,
	]);
	expect(await hint(page)).toBeNull();
	await page.getByRole("link", { name: "Back to sign in" }).click();
	await expect(page).toHaveURL("/auth/login");
});

test.describe("accessibility", () => {
	// Kills contrast, names, and roles that fail axe in either theme.
	test("Check your inbox for a provider account passes axe in both themes", async ({
		page,
	}) => {
		await finishWith(page, toProve("ada@nowhere.com"));
		// Each scan returns afresh from the door, since the return leaves for
		// Check your inbox on its own.
		await axeInBothThemes(
			page,
			"/auth/login",
			() => expect(page.locator(".au-who")).toHaveText("ada-work"),
			async () => {
				await page.goto("/auth/github?code=c0de&state=s7a7e");
				await expect(page).toHaveURL("/auth/confirm");
			},
		);
	});
});
