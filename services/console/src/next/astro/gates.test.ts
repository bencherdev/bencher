import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../../util/convert";
import { USER_KEY } from "../reader";
import { VERSION_KEY } from "../memory";
import { classicPlace, profilePath, projectPath } from "../paths";
import AccountGate from "./AccountGate.astro";
import ClassicGate from "./ClassicGate.astro";
import Landing from "./Landing.astro";
import NextHead from "./NextHead.astro";

const container = await AstroContainer.create();

/** The inline script a component renders, run on a page at `url` with `storage`. */
const run = (html: string, url: string, storage: Record<string, string>) => {
	const page = new Window().document;
	page.body.innerHTML = html;
	const source = page.querySelector("script")?.textContent;
	const replaced: string[] = [];
	const { pathname, search, hash } = new URL(url);
	if (source) {
		new Function("localStorage", "location", source)(
			{ getItem: (key: string) => storage[key] ?? null },
			{ pathname, search, hash, replace: (to: string) => replaced.push(to) },
		);
	}
	return { source, replaced };
};

const SIGNED_IN = { [USER_KEY]: '{"token":"tok","user":{"uuid":"u"}}' };
const versions = (slug: string, version: number) => ({
	[VERSION_KEY]: JSON.stringify({ [slug]: version }),
});

describe("NextHead", () => {
	const next = async (url: string, storage: Record<string, string>) =>
		run(
			await container.renderToString(NextHead, {
				props: {
					slug: "hashbrown",
					classic: classicPlace(new URL(url).pathname),
				},
			}),
			url,
			storage,
		).replaced;
	const URL_ = "https://bencher.dev/next/console/projects/hashbrown/runs";

	// Kills a version gate that waits for the app to load the project.
	test("a project remembered on version 0 goes to the classic page at once", async () => {
		expect(
			await next(URL_, { ...SIGNED_IN, ...versions("hashbrown", 0) }),
		).toEqual(["/console/projects/hashbrown/reports"]);
	});

	// Kills a gate that drops the place on the page, or carries one to the
	// perf page, which reads none.
	test("the classic page keeps the hash, except the perf page", async () => {
		const remembered = { ...SIGNED_IN, ...versions("hashbrown", 0) };
		expect(await next(`${URL_}#top`, remembered)).toEqual([
			"/console/projects/hashbrown/reports#top",
		]);
		expect(
			await next(
				"https://bencher.dev/next/console/projects/hashbrown/explore#top",
				remembered,
			),
		).toEqual(["/console/projects/hashbrown/perf"]);
	});

	// Kills a gate that moves a project it should keep, or reads another slug.
	test("a project remembered on version 1, or not at all, stays", async () => {
		expect(
			await next(URL_, { ...SIGNED_IN, ...versions("hashbrown", 1) }),
		).toEqual([]);
		expect(await next(URL_, { ...SIGNED_IN, ...versions("other", 0) })).toEqual(
			[],
		);
		expect(await next(URL_, { ...SIGNED_IN, [VERSION_KEY]: "{" })).toEqual([]);
	});

	// Kills a reader who is not signed in left on a page that cannot load.
	test("a reader who is not signed in goes to sign in with the way back", async () => {
		const [to] = await next(`${URL_}?page=2#top`, {});
		const href = new URL(to ?? "", "https://bencher.dev");
		expect(href.pathname).toBe("/auth/login");
		expect(decodeBase64(href.searchParams.get("back"))).toBe(
			"/next/console/projects/hashbrown/runs?page=2#top",
		);
	});
});

describe("ClassicGate", () => {
	const classic = async (url: string, storage: Record<string, string>) =>
		run(
			await container.renderToString(ClassicGate, {
				props: { slug: "hashbrown" },
				request: new Request(url),
			}),
			url,
			storage,
		);
	const URL_ = "https://bencher.dev/console/projects/hashbrown/reports";

	// Kills a classic page that waits for its own code to leave.
	test("a project remembered on version 1 goes to the same place under /next/", async () => {
		expect(
			(await classic(`${URL_}?per_page=8#top`, versions("hashbrown", 1)))
				.replaced,
		).toEqual(["/next/console/projects/hashbrown/runs#top"]);
	});

	// Kills a classic page that leaves for a project it should keep.
	test("a project remembered on version 0, or not at all, stays", async () => {
		expect((await classic(URL_, versions("hashbrown", 0))).replaced).toEqual(
			[],
		);
		expect((await classic(URL_, {})).replaced).toEqual([]);
	});

	// Kills a classic perf link that arrives in Explore without its query.
	test("the perf page goes to Explore with its query", async () => {
		expect(
			(
				await classic(
					"https://bencher.dev/console/projects/hashbrown/perf?branches=b&x_axis=version",
					versions("hashbrown", 1),
				)
			).replaced,
		).toEqual([
			"/next/console/projects/hashbrown/explore?branches=b&x_axis=version",
		]);
	});

	// Kills a classic page sent to "Page not found" under /next/.
	test("a page the new console does not have stays", async () => {
		expect(
			(
				await classic(
					"https://bencher.dev/console/projects/hashbrown/metrics/abc",
					versions("hashbrown", 1),
				)
			).replaced,
		).toEqual([]);
	});

	// Kills a classic alert page that sends a reader who is not signed in to
	// sign in under /next/, rather than where the classic page would send them,
	// and one that keeps a signed in reader.
	test("the classic alert page sends only a signed in reader to the new console", async () => {
		const ALERT = "https://bencher.dev/console/projects/hashbrown/alerts/a";
		const gate = async (storage: Record<string, string>) =>
			run(
				await container.renderToString(ClassicGate, {
					props: { slug: "hashbrown", signedOut: "/perf/hashbrown/alerts/a" },
					request: new Request(ALERT),
				}),
				ALERT,
				storage,
			).replaced;
		expect(await gate({ ...SIGNED_IN, ...versions("hashbrown", 1) })).toEqual([
			"/next/console/projects/hashbrown/alerts/a",
		]);
		expect(await gate(versions("hashbrown", 1))).toEqual([
			"/perf/hashbrown/alerts/a",
		]);
		expect(await gate(versions("hashbrown", 0))).toEqual([]);
	});

	// Kills a gate on pages that are not a project's.
	test("renders nothing outside a project", async () => {
		expect(
			(await classic("https://bencher.dev/console/projects", {})).source,
		).toBeUndefined();
	});
});

describe("ClassicGate on the public perf page", () => {
	const URL_ =
		"https://bencher.dev/perf/hashbrown?branches=b&x_axis=version&tab=benchmarks";
	const NEXT = "/next/perf/hashbrown?branches=b&x_axis=version";
	const perf = async (storage: Record<string, string>) =>
		run(
			await container.renderToString(ClassicGate, {
				props: { slug: "hashbrown", next: NEXT },
				request: new Request(URL_),
			}),
			URL_,
			storage,
		).replaced;

	// Kills a public perf page that waits for its own code to leave, or that
	// leaves with the classic page's query instead of the public plot's.
	test("a project remembered on version 1 goes to the public plot with its query", async () => {
		expect(await perf(versions("hashbrown", 1))).toEqual([NEXT]);
	});

	// Kills a public perf page that leaves for a project it should keep.
	test("a project remembered on version 0, or not at all, stays", async () => {
		expect(await perf(versions("hashbrown", 0))).toEqual([]);
		expect(await perf({})).toEqual([]);
	});
});

describe("AccountGate", () => {
	const URL_ = "https://bencher.dev/console/users/muriel-bagge/settings#top";
	const gate = async (storage: Record<string, string>) =>
		run(
			await container.renderToString(AccountGate, {
				props: { user: "muriel-bagge" },
				request: new Request(URL_),
			}),
			URL_,
			storage,
		).replaced;

	// Kills a classic Profile that waits for its own code to leave, or that
	// asks the version of one project rather than any.
	test("a reader with a version 1 project goes to Profile under /next/", async () => {
		expect(
			await gate({
				[VERSION_KEY]: JSON.stringify({ "crinkle-cut": 0, hashbrown: 1 }),
			}),
		).toEqual([profilePath("muriel-bagge")]);
	});

	// Kills a gate that moves a reader of version 0 projects only, or one
	// that fails on a memory it cannot read.
	test("a reader without a version 1 project stays", async () => {
		expect(await gate(versions("crinkle-cut", 0))).toEqual([]);
		expect(await gate({})).toEqual([]);
		expect(await gate({ [VERSION_KEY]: "{" })).toEqual([]);
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
