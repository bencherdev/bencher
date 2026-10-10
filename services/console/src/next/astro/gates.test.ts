import { experimental_AstroContainer as AstroContainer } from "astro/container";
import { Window } from "happy-dom";
import { describe, expect, test } from "vitest";
import { decodeBase64 } from "../../util/convert";
import { USER_KEY } from "../reader";
import { VERSION_KEY } from "../memory";
import { classicPlace } from "../paths";
import ClassicGate from "./ClassicGate.astro";
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
	const URL_ = "https://bencher.dev/next/console/projects/hashbrown/reports";

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
			"/next/console/projects/hashbrown/reports?page=2#top",
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
		).toEqual(["/next/console/projects/hashbrown/reports#top"]);
	});

	// Kills a classic page that leaves for a project it should keep.
	test("a project remembered on version 0, or not at all, stays", async () => {
		expect((await classic(URL_, versions("hashbrown", 0))).replaced).toEqual(
			[],
		);
		expect((await classic(URL_, {})).replaced).toEqual([]);
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

	// Kills a gate on pages that are not a project's.
	test("renders nothing outside a project", async () => {
		expect(
			(await classic("https://bencher.dev/console/projects", {})).source,
		).toBeUndefined();
	});
});
