// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { RETURN_TTL, backPage, keepReturn, takeReturn } from "./back";

const ORIGIN = "https://bencher.example";
const NOW = Date.parse("2026-10-19T00:00:00Z");
const KEY = "BENCHER_RETURN";

const b64 = (text: string) =>
	btoa(String.fromCodePoint(...new TextEncoder().encode(text)));

const memory = (entries: Record<string, string> = {}) => {
	const map = new Map(Object.entries(entries));
	return {
		map,
		getItem: (key: string) => map.get(key) ?? null,
		setItem: (key: string, value: string) => {
			map.set(key, value);
		},
		removeItem: (key: string) => {
			map.delete(key);
		},
	};
};

const kept = (href: string, at = NOW) => JSON.stringify({ href, at });

describe("backPage", () => {
	// Kills a return that drops the page's query or hash, or that is not absolute,
	// so a path that normalizes to `//host` could leave the origin.
	test("is the named page, whole and absolute on the origin", () => {
		expect(
			backPage(
				b64("/next/console/projects/hashbrown/alerts?status=all#top"),
				ORIGIN,
			),
		).toBe(`${ORIGIN}/next/console/projects/hashbrown/alerts?status=all#top`);
		expect(backPage(b64("/perf/hashbrown?é=1"), ORIGIN)).toBe(
			`${ORIGIN}/perf/hashbrown?%C3%A9=1`,
		);
		expect(backPage(b64(`${ORIGIN}/next/console/projects//evil`), ORIGIN)).toBe(
			`${ORIGIN}/next/console/projects//evil`,
		);
		expect(backPage(b64("/next/console/projects/%2F%2Fevil"), ORIGIN)).toBe(
			`${ORIGIN}/next/console/projects/%2F%2Fevil`,
		);
	});

	// Kills a return that refuses a console or public plot page for the
	// encodings its own query, hash, and slugs carry.
	test("is a console or public plot page in any of its own encodings", () => {
		for (const back of [
			"/console",
			"/next/console/projects/hashbrown/",
			"/next/console/projects/hashbrown/explore?q=%25&x=%2561#a%25b",
			"/perf/hashbrown?heads=%25",
			"/next/perf/hashbrown/embed?title=50%25%20faster",
			"/next/console/projects/caf%C3%A9/",
			"/console/projects/hashbrown/perf?clear=true#x%20y",
			"/next/console/projects/hashbrown/runs/0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f",
		]) {
			expect(backPage(b64(back), ORIGIN), back).toBe(`${ORIGIN}${back}`);
		}
	});

	// Kills a return to a page outside the console and the public plot, or one
	// whose dot segments or backslashes resolve outside them once decoded.
	test("is nothing outside the console and the public plot", () => {
		for (const back of [
			"/x/..%2fauth/logout",
			"/x/..%5Cauth/logout",
			"/next/console/..%2f..%2fauth/logout",
			"/perf/..%2fauth/logout",
			"/perf/..%5Cauth/logout",
			"/perf/..%252fauth/logout",
			"/next/perf/%2e%2e%2fauth/logout",
			"/console/x%5Cy/..%2f..%2fauth/logout",
			"/perf/a%5Cb/..%2f..%2fauth/logout",
			"/console/x%5Cy/%2e%2e%2f%2e%2e%2fauth/logout",
			"/next/console/a%5Cb%5Cc/..%2f..%2f..%2fauth/authorize?request=0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f",
			"/perf/%5C..%5C..%5Cauth/logout",
			"/console/%2e%2fprojects/hashbrown",
			"/consoles/",
			"/docs/",
			"/pricing",
			"/",
			"/explore",
		]) {
			expect(backPage(b64(back), ORIGIN), back).toBeUndefined();
		}
	});

	// Kills a return followed to another origin or a script, however it is spelled.
	test("is nothing for another origin or scheme", () => {
		for (const back of [
			"javascript:alert(1)",
			"JaVaScRiPt:alert(1)",
			" javascript:alert(1)",
			"\tjava\nscript:alert(1)",
			"data:text/html,<script>alert(1)</script>",
			"//evil.com",
			"\\\\evil.com",
			"/\\evil.com",
			" //evil.com",
			"https://evil.com",
			"HTTPS://EVIL.COM/next/console",
			`${ORIGIN}.evil.com/next/console`,
			`${ORIGIN}@evil.com/next/console`,
			"http://bencher.example/next/console",
			"https://api.bencher.example/v0/projects",
			`blob:${ORIGIN}/0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f`,
		]) {
			expect(backPage(b64(back), ORIGIN), back).toBeUndefined();
		}
	});

	// Kills a sign in that returns to a sign in, a sign out, a flow of its own,
	// or a file rather than a page.
	test("is nothing for a path that is not a page", () => {
		for (const back of [
			"/auth",
			"/auth/login",
			"/auth/logout",
			"/auth/confirm?token=x",
			"/auth/authorize?request=x",
			"/AUTH/logout",
			"/%61uth/logout",
			"/auth%2flogout",
			"/auth%5Clogout",
			"/%2Fauth/logout",
			"/%2561uth/logout",
			"/%61uth/authorize?request=0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f",
			"/_astro/main.js",
			"/_image?href=x",
			"/robots.txt",
			"/perf/hashbrown/robots.txt",
			"/next/console/projects/hashbrown/main.js",
			"/v0/bmf.json",
		]) {
			expect(backPage(b64(back), ORIGIN), back).toBeUndefined();
		}
	});

	// Kills a decoder that throws, or reads a value that is not base64 as a path.
	test("is nothing for a value that is not a page's base64", () => {
		for (const back of [undefined, null, 1, "", "%%%"]) {
			expect(backPage(back, ORIGIN), String(back)).toBeUndefined();
		}
	});
});

describe("keepReturn", () => {
	// Kills a sign in page that does not keep the page, or keeps one it refuses.
	test("keeps a page and forgets any other value", () => {
		const storage = memory();
		keepReturn(storage, b64("/next/console/projects/hashbrown/"), ORIGIN, NOW);
		expect(JSON.parse(storage.map.get(KEY) ?? "null")).toEqual({
			href: `${ORIGIN}/next/console/projects/hashbrown/`,
			at: NOW,
		});
		keepReturn(storage, b64("https://evil.com"), ORIGIN, NOW);
		expect(storage.map.has(KEY)).toBe(false);
		storage.map.set(KEY, kept(`${ORIGIN}/console`));
		keepReturn(storage, undefined, ORIGIN, NOW);
		expect(storage.map.has(KEY)).toBe(false);
	});
});

describe("takeReturn", () => {
	// Kills a return used twice, or one a later sign in still finds.
	test("is the kept page once", () => {
		const storage = memory({ [KEY]: kept(`${ORIGIN}/perf/hashbrown`) });
		expect(takeReturn(storage, ORIGIN, NOW + RETURN_TTL)).toBe(
			`${ORIGIN}/perf/hashbrown`,
		);
		expect(storage.map.has(KEY)).toBe(false);
		expect(takeReturn(storage, ORIGIN, NOW)).toBeUndefined();
	});

	// Kills a return that outlives the sign in that asked for it, or one kept
	// with a time ahead of now.
	test("is nothing once stale, and is forgotten", () => {
		for (const at of [NOW - RETURN_TTL - 1, NOW + 1, Number.NaN]) {
			const storage = memory({ [KEY]: kept(`${ORIGIN}/perf/hashbrown`, at) });
			expect(takeReturn(storage, ORIGIN, NOW), String(at)).toBeUndefined();
			expect(storage.map.has(KEY)).toBe(false);
		}
	});

	// Kills a return trusted because it was checked when kept: storage another
	// script or an older console wrote is checked again when read.
	test("checks the kept page again", () => {
		for (const value of [
			kept("https://evil.com/next/console"),
			kept("javascript:alert(1)"),
			kept(`${ORIGIN}/auth/logout`),
			kept(`${ORIGIN}/%61uth/logout`),
			kept(`blob:${ORIGIN}/0b5c4f6e-2a1d-4c3b-9e8f-7a6b5c4d3e2f`),
			JSON.stringify({ href: 1, at: NOW }),
			JSON.stringify({ at: NOW }),
			"{",
			"null",
		]) {
			const storage = memory({ [KEY]: value });
			expect(takeReturn(storage, ORIGIN, NOW), value).toBeUndefined();
			expect(storage.map.has(KEY)).toBe(false);
		}
	});
});
