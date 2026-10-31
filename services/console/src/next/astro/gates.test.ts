import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../../util/convert";
import { USER_KEY } from "../reader";
import { VERSION_KEY } from "../memory";
import { projectPath } from "../paths";
import GitHubReturn from "./GitHubReturn.astro";
import { takeGitHubReturn } from "./githubReturn";
import Landing from "./Landing.astro";
import NextHead from "./NextHead.astro";
import Shortcut from "./Shortcut.astro";

const container = await AstroContainer.create();

/** The inline scripts a component renders, run in order on a page at `url` with `storage`. */
const run = (html: string, url: string, storage: Record<string, string>) => {
	const page = new Window().document;
	page.body.innerHTML = html;
	const replaced: string[] = [];
	const { pathname, search, hash } = new URL(url);
	for (const script of page.querySelectorAll("script")) {
		new Function("localStorage", "location", script.textContent ?? "")(
			{ getItem: (key: string) => storage[key] ?? null },
			{ pathname, search, hash, replace: (to: string) => replaced.push(to) },
		);
	}
	return { replaced };
};

const SIGNED_IN = { [USER_KEY]: '{"token":"tok","user":{"uuid":"u"}}' };
const versions = (slug: string, version: number) => ({
	[VERSION_KEY]: JSON.stringify({ [slug]: version }),
});

describe("NextHead", () => {
	const next = async (url: string, storage: Record<string, string>) =>
		run(await container.renderToString(NextHead), url, storage).replaced;
	const URL_ = "https://bencher.dev/console/projects/hashbrown/runs";

	// Kills a gate that still sends a version 0 project back to the classic
	// console, which would keep its pages out of the new one.
	test("a project remembered on either version stays", async () => {
		for (const version of [0, 1]) {
			expect(
				await next(`${URL_}#top`, {
					...SIGNED_IN,
					...versions("hashbrown", version),
				}),
			).toEqual([]);
		}
		expect(await next(URL_, { ...SIGNED_IN, [VERSION_KEY]: "{" })).toEqual([]);
	});

	// Kills a reader who is not signed in left on a page that cannot load, and
	// one sent to the classic console because the project was on version 0.
	test("a reader who is not signed in goes to sign in with the way back", async () => {
		for (const storage of [{}, versions("hashbrown", 0)]) {
			const [to, ...rest] = await next(`${URL_}?page=2#top`, storage);
			expect(rest).toEqual([]);
			const href = new URL(to ?? "", "https://bencher.dev");
			expect(href.pathname).toBe("/auth/login");
			expect(decodeBase64(href.searchParams.get("back"))).toBe(
				"/console/projects/hashbrown/runs?page=2#top",
			);
		}
	});
});

describe("Landing", () => {
	const land = async (
		storage: Record<string, string>,
		url = "https://bencher.dev/",
	) => run(await container.renderToString(Landing), url, storage).replaced;
	const reader = {
		[USER_KEY]: JSON.stringify({
			token: "tok",
			user: { uuid: "u" },
			expiration: "2999-01-01T00:00:00Z",
		}),
	};

	// Kills a root page that waits for its own code to send a signed in reader
	// on, or a script that reads other keys than the landing's.
	test("a signed in reader goes to the main project before the page loads", async () => {
		expect(await land({ ...reader, ...versions("hashbrown", 1) })).toEqual([
			projectPath("hashbrown"),
		]);
	});

	// Kills a version 0 project sent anywhere but its root in the console.
	test("a version 0 project lands on its root too", async () => {
		expect(await land({ ...reader, ...versions("hashbrown", 0) })).toEqual([
			projectPath("hashbrown"),
		]);
	});

	// Kills a root page that sends a reader who is not signed in away.
	test("a reader who is not signed in stays", async () => {
		expect(await land(versions("hashbrown", 1))).toEqual([]);
	});

	// Kills a script that reads no query string, dropping an invite before
	// the page can accept it.
	test("a query string stays", async () => {
		expect(
			await land(
				{ ...reader, ...versions("hashbrown", 1) },
				"https://bencher.dev/?invite=x",
			),
		).toEqual([]);
	});

	// Kills a script that checks the token against any time but now.
	test("a reader whose token has expired stays", async () => {
		const expired = {
			[USER_KEY]: JSON.stringify({
				token: "tok",
				user: { uuid: "u" },
				expiration: new Date(Date.now() - 60_000).toISOString(),
			}),
		};
		expect(await land({ ...expired, ...versions("hashbrown", 1) })).toEqual([]);
	});
});

describe("Shortcut", () => {
	const go = async (url: string, storage: Record<string, string>) =>
		run(
			await container.renderToString(Shortcut, { request: new Request(url) }),
			url,
			storage,
		).replaced;
	const reader = {
		[USER_KEY]: JSON.stringify({
			token: "tok",
			user: { slug: "muriel-bagge" },
		}),
	};

	// Kills a shortcut that names no user sent anywhere but the reader's own
	// page of the same kind.
	test("a signed in reader lands on their own page", async () => {
		const cases: [string, string][] = [
			["/console/users", "settings"],
			["/console/users/settings", "settings"],
			["/console/users/keys/add", "keys"],
			["/console/users/tokens", "tokens"],
		];
		for (const [path, page] of cases) {
			expect(await go(`https://bencher.dev${path}`, reader), path).toEqual([
				`/console/users/muriel-bagge/${page}`,
			]);
		}
	});

	// Kills a reader who is not signed in left on a blank page, and one sent
	// to sign in without the shortcut to return to.
	test("a reader who is not signed in goes to sign in and back", async () => {
		const [to, ...rest] = await go(
			"https://bencher.dev/console/users/keys/add",
			{},
		);
		expect(rest).toEqual([]);
		const href = new URL(to ?? "", "https://bencher.dev");
		expect(href.pathname).toBe("/auth/login");
		expect(decodeBase64(href.searchParams.get("back"))).toBe(
			"/console/users/keys/add",
		);
	});
});

describe("GitHubReturn", () => {
	/** The return page's first script, run on a page at `url` for a reader whose storage is `storage`. */
	const land = async (url: string, storage: Record<string, string>) => {
		const page = new Window().document;
		page.body.innerHTML = await container.renderToString(GitHubReturn);
		const source = page.querySelector("script")?.textContent ?? "";
		const { pathname, search, hash } = new URL(url);
		const replaced: string[] = [];
		const memory = {};
		new Function("window", "localStorage", "location", "history", source)(
			memory,
			{ getItem: (key: string) => storage[key] ?? null },
			{ pathname, search, hash },
			{
				state: null,
				replaceState: (_state: unknown, _title: string, to: string) =>
					replaced.push(to),
			},
		);
		return { replaced, memory };
	};

	// Kills a code or a state left in the URL for any reader, signed in or
	// not, where the sign-in guard, a session that ends as the page loads, or
	// a sign out in another tab carries it to sign in and the way back it
	// keeps; and a page that drops what the exchange needs, or keeps it in
	// memory once the exchange took it.
	test("the code and the state leave the URL at once, for the exchange alone", async () => {
		for (const storage of [
			{
				[USER_KEY]: JSON.stringify({
					token: "tok",
					user: { uuid: "u" },
					expiration: "2999-01-01T00:00:00Z",
				}),
			},
			{},
		]) {
			const { replaced, memory } = await land(
				"https://bencher.dev/console/github?code=c0de&state=s7a7e",
				storage,
			);
			expect(replaced).toEqual(["/console/github"]);
			expect(takeGitHubReturn(memory)).toEqual({
				code: "c0de",
				state: "s7a7e",
			});
			expect(takeGitHubReturn(memory)).toEqual({
				code: undefined,
				state: undefined,
			});
		}
	});
});
