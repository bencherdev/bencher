import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import type { JsonSignedIn } from "../src/types/bencher";
import { expect, type Reader, RELAY, seed, sessionCookie } from "./fixtures";

const CORS = { "Access-Control-Allow-Origin": "*" };
const GITHUB = `${RELAY}/v0/auth/github`;

/** Start a GitHub sign in from the door at `door`, as far as GitHub, which the browser never really reaches. */
export const startGitHub = async (page: Page, door: string) => {
	const started: string[] = [];
	const state = randomUUID();
	await page.route(
		(url) => `${url.origin}${url.pathname}` === GITHUB,
		(route) => {
			if (route.request().method() !== "GET") {
				return route.fallback();
			}
			started.push(route.request().url());
			return route.fulfill({
				headers: CORS,
				json: {
					url: `https://github.example/login/oauth/authorize?${new URLSearchParams({ state })}`,
				},
			});
		},
	);
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
 * named, for the request `authorization` when one. The exchange begins the
 * reader's session as the API's would, in its cookie.
 */
export const finishGitHub = async (
	page: Page,
	state: string,
	authorization?: string,
	reader: Reader = seed.member,
) => {
	const json: JsonSignedIn = { user: reader.user, new: false };
	if (authorization) {
		json.authorization = authorization;
	}
	await page.route(GITHUB, async (route) => {
		if (route.request().method() !== "POST") {
			return route.continue();
		}
		await page.context().addCookies([sessionCookie(reader.session)]);
		return route.fulfill({ status: 202, headers: CORS, json });
	});
	await page.goto(
		`/auth/github?${new URLSearchParams({ code: "code", state })}`,
	);
};
