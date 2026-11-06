import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import type { JsonProviderSignIn, JsonSignedIn } from "../src/types/bencher";
import { expect, type Reader, RELAY, seed, sessionCookie } from "./fixtures";

/** Where the console starts and finishes a GitHub sign-in, which the suite's API cannot reach. */
export const GITHUB_START = `${RELAY}/v0/auth/providers/github/start`;
export const GITHUB_FINISH = `${RELAY}/v0/auth/providers/github/finish`;

/**
 * Start a GitHub sign-in from the door at `door`, as far as GitHub, which the
 * browser never really reaches. `started` holds what each start asked to carry.
 */
export const startGitHub = async (page: Page, door: string) => {
	const started: unknown[] = [];
	const state = randomUUID();
	await page.route(GITHUB_START, (route) => {
		started.push(route.request().postDataJSON());
		return route.fulfill({
			json: {
				url: `https://github.example/login/oauth/authorize?${new URLSearchParams({ state })}`,
			},
		});
	});
	await page.route("https://github.example/**", (route) =>
		route.fulfill({ contentType: "text/plain", body: "GitHub" }),
	);
	await page.goto(door);
	await page.getByRole("button", { name: "Continue with GitHub" }).click();
	await expect(page).toHaveURL(/^https:\/\/github\.example\//);
	return { started, state };
};

/**
 * Come back from GitHub with `state`, signed in as `reader`, the member unless
 * named, for the request `authorization` when one. The finish begins the
 * reader's session as the API's would, in its cookie.
 */
export const finishGitHub = async (
	page: Page,
	state: string,
	authorization?: string,
	reader: Reader = seed.member,
) => {
	const signed_in: JsonSignedIn = { user: reader.user, new: false };
	if (authorization) {
		signed_in.authorization = authorization;
	}
	await page.route(GITHUB_FINISH, async (route) => {
		await page.context().addCookies([sessionCookie(reader.session)]);
		const json: JsonProviderSignIn = { signed_in };
		return route.fulfill({ json });
	});
	await page.goto(
		`/auth/github?${new URLSearchParams({ code: "code", state })}`,
	);
};
