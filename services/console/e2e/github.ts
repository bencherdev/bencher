import { randomUUID } from "node:crypto";
import type { Page } from "@playwright/test";
import type { JsonOAuthUser } from "../src/types/bencher";
import { expect, seed } from "./fixtures";

const CORS = { "Access-Control-Allow-Origin": "*" };
const GITHUB = `${seed.api_url}/v0/auth/github`;

/** Start a GitHub sign in from the login page at `login`, as far as GitHub, which the browser never really reaches. */
export const startGitHub = async (page: Page, login: string) => {
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
	await page.goto(login);
	await page.getByRole("button", { name: "Log in with GitHub" }).click();
	await expect(page).toHaveURL(/^https:\/\/github\.example\//);
	return { started, state };
};

/** Come back from GitHub with `state`, signed in as the member, for the request `authorization` when one. */
export const finishGitHub = async (
	page: Page,
	state: string,
	authorization?: string,
) => {
	const json: JsonOAuthUser = { user: seed.member };
	if (authorization) {
		json.authorization = authorization;
	}
	await page.route(GITHUB, (route) =>
		route.request().method() === "POST"
			? route.fulfill({ status: 202, headers: CORS, json })
			: route.continue(),
	);
	await page.goto(
		`/auth/github?${new URLSearchParams({ code: "code", state })}`,
	);
};
