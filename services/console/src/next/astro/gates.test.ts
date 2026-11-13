import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../../util/base64";
import { READER_KEY } from "../reader";
import { VERSION_KEY } from "../memory";
import { projectPath } from "../paths";
import AuthReturn from "./AuthReturn.astro";
import { takeAuthReturn } from "./authReturn";
import EmailReturn from "./EmailReturn.astro";
import { takeEmailChange } from "./emailReturn";
import GitHubReturn from "./GitHubReturn.astro";
import { takeGitHubReturn } from "./githubReturn";
import InviteReturn from "./InviteReturn.astro";
import { takeInvite } from "./inviteReturn";
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

const HINT = JSON.stringify({
	uuid: "u",
	slug: "muriel-bagge",
	name: "Muriel",
});
const SIGNED_IN = { [READER_KEY]: HINT };
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
	const reader = SIGNED_IN;

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
});

describe("Shortcut", () => {
	const go = async (url: string, storage: Record<string, string>) =>
		run(
			await container.renderToString(Shortcut, { request: new Request(url) }),
			url,
			storage,
		).replaced;
	const reader = SIGNED_IN;

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
		for (const storage of [SIGNED_IN, {}]) {
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

describe("AuthReturn", () => {
	/** An auth page's first script, run on a page at `url`. */
	const land = async (url: string) => {
		const page = new Window().document;
		page.body.innerHTML = await container.renderToString(AuthReturn);
		const source = page.querySelector("script")?.textContent ?? "";
		const { pathname, search, hash } = new URL(url);
		const replaced: string[] = [];
		const memory = {};
		new Function("window", "location", "history", source)(
			memory,
			{ pathname, search, hash },
			{
				state: null,
				replaceState: (_state: unknown, _title: string, to: string) =>
					replaced.push(to),
			},
		);
		return { replaced, memory };
	};

	// Kills a link's secret, an old link's token, or a provider's code or state
	// left in the address for history, a report, or a later script to read, and
	// a page that drops what it needs, or keeps it once taken.
	test("what the address carried leaves it at once, for the page alone", async () => {
		const none = {
			secret: undefined,
			token: undefined,
			code: undefined,
			state: undefined,
		};
		for (const [url, kept] of [
			["https://bencher.dev/auth/confirm#s3cr3t", { secret: "s3cr3t" }],
			["https://bencher.dev/auth/confirm?token=a.b.c", { token: "a.b.c" }],
			[
				"https://bencher.dev/auth/github?code=c0de&state=s7a7e",
				{ code: "c0de", state: "s7a7e" },
			],
		] as const) {
			const { replaced, memory } = await land(url);
			expect(replaced).toEqual([new URL(url).pathname]);
			expect(takeAuthReturn(memory)).toEqual({ ...none, ...kept });
			expect(takeAuthReturn(memory)).toEqual(none);
		}
	});
});

describe("InviteReturn", () => {
	/** The door's first script, run on a page at `url`. */
	const land = async (url: string) => {
		const page = new Window().document;
		page.body.innerHTML = await container.renderToString(InviteReturn);
		const source = page.querySelector("script")?.textContent ?? "";
		const { pathname, search, hash } = new URL(url);
		const replaced: string[] = [];
		const memory = {};
		new Function("window", "location", "history", source)(
			memory,
			{ pathname, search, hash },
			{
				state: null,
				replaceState: (_state: unknown, _title: string, to: string) =>
					replaced.push(to),
			},
		);
		return { replaced, memory };
	};

	// Kills an invitation's secret left in the address for history, a report,
	// or a later script to read; a door that drops what the rest of its address
	// carries; one that drops the secret, or keeps it once taken; and one that
	// rewrites an address that carries no invitation.
	test("an invitation's secret leaves the address at once, for the page alone", async () => {
		const { replaced, memory } = await land(
			"https://bencher.dev/auth/login?claim=c1a1d&back=Lw#invite=s3cr3t",
		);
		expect(replaced).toEqual(["/auth/login?claim=c1a1d&back=Lw"]);
		expect(takeInvite(memory)).toBe("s3cr3t");
		expect(takeInvite(memory)).toBeUndefined();

		for (const url of [
			"https://bencher.dev/auth/login",
			"https://bencher.dev/auth/login?invite=a.b.c",
			"https://bencher.dev/auth/login#top",
		]) {
			const { replaced, memory } = await land(url);
			expect(replaced, url).toEqual([]);
			expect(takeInvite(memory), url).toBeUndefined();
		}
	});
});

describe("EmailReturn", () => {
	/** The email change page's first script, run on a page at `url`. */
	const land = async (url: string) => {
		const page = new Window().document;
		page.body.innerHTML = await container.renderToString(EmailReturn);
		const source = page.querySelector("script")?.textContent ?? "";
		const { pathname, search, hash } = new URL(url);
		const replaced: string[] = [];
		const memory = {};
		new Function("window", "location", "history", source)(
			memory,
			{ pathname, search, hash },
			{
				state: null,
				replaceState: (_state: unknown, _title: string, to: string) =>
					replaced.push(to),
			},
		);
		return { replaced, memory };
	};

	// Kills an email change's secret left in the address for history, a
	// report, or a later script to read; an old link's token and user left
	// there; a page that drops the secret, or keeps it once taken.
	test("an email change's secret leaves the address at once, for the page alone", async () => {
		for (const [url, secret] of [
			["https://bencher.dev/auth/email#secret=s3cr3t", "s3cr3t"],
			["https://bencher.dev/auth/email?user=u&token=a.b.c", undefined],
			["https://bencher.dev/auth/email", undefined],
		] as const) {
			const { replaced, memory } = await land(url);
			expect(replaced, url).toEqual(["/auth/email"]);
			expect(takeEmailChange(memory), url).toBe(secret);
			expect(takeEmailChange(memory), url).toBeUndefined();
		}
	});
});
