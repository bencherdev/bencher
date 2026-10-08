import { describe, expect, test } from "vitest";
import { type JsonProject, Visibility } from "../../types/bencher";
import { ApiError } from "../api";
import {
	type Draft,
	patchOf,
	patched,
	problemsOf,
	refusalOf,
	webUrl,
} from "./general";

const saved: Draft = {
	name: "Hashbrown",
	slug: "hashbrown",
	url: "https://github.com/pompeii-llc/hashbrown",
	visibility: Visibility.Public,
};

const project = {
	uuid: "a",
	organization: "o",
	name: "Hashbrown",
	slug: "hashbrown",
	url: "https://github.com/pompeii-llc/hashbrown",
	visibility: Visibility.Public,
	bmf_version: 1,
	created: "2026-09-01T00:00:00Z",
	modified: "2026-09-01T00:00:00Z",
} as JsonProject;

describe("patchOf", () => {
	// Kills a save that resends untouched fields: a resent visibility asks the
	// plan check again, and a resent slug can collide.
	test("sends only what changed, and nothing when nothing did", () => {
		expect(patchOf(saved, saved)).toBeUndefined();
		expect(patchOf(saved, { ...saved, name: "Hash Browns" })).toEqual({
			name: "Hash Browns",
		});
		expect(
			patchOf(saved, {
				...saved,
				slug: "hash-browns",
				visibility: Visibility.Private,
			}),
		).toEqual({ slug: "hash-browns", visibility: Visibility.Private });
	});

	// Kills a name the API refuses for its spaces, and an emptied URL the
	// API never hears about.
	test("trims the name and the URL, and an emptied URL removes it", () => {
		expect(patchOf(saved, { ...saved, name: "  Hashbrown " })).toBeUndefined();
		expect(patchOf(saved, { ...saved, name: " Hash Browns " })).toEqual({
			name: "Hash Browns",
		});
		expect(patchOf(saved, { ...saved, url: "  " })).toEqual({ url: null });
		expect(
			patchOf({ ...saved, url: "" }, { ...saved, url: "" }),
		).toBeUndefined();
	});
});

describe("problemsOf", () => {
	// Kills a form that sends what the API refuses, and one that refuses what
	// the API takes.
	test("names each field the API would refuse", () => {
		expect(problemsOf(saved)).toEqual({});
		expect(problemsOf({ ...saved, url: "" })).toEqual({});
		expect(problemsOf({ ...saved, slug: "hash-browns-2" })).toEqual({});

		expect(Object.keys(problemsOf({ ...saved, name: "  " }))).toEqual(["name"]);
		// 33 two-byte letters are 66 bytes, over the API's 64.
		expect(Object.keys(problemsOf({ ...saved, name: "é".repeat(33) }))).toEqual(
			["name"],
		);
		expect(problemsOf({ ...saved, name: "é".repeat(32) })).toEqual({});

		for (const slug of [
			"",
			"Hash",
			"hash browns",
			"hash--browns",
			"-hash",
			"hash-",
			"h".repeat(65),
		]) {
			expect(Object.keys(problemsOf({ ...saved, slug })), slug).toEqual([
				"slug",
			]);
		}
		expect(problemsOf({ ...saved, slug: "h".repeat(64) })).toEqual({});

		expect(
			Object.keys(problemsOf({ ...saved, url: "github.com/pompeii-llc" })),
		).toEqual(["url"]);
	});

	// Kills a URL that runs script, or opens anything but a web page, when a
	// reader follows the project's link.
	test("takes only a web URL", () => {
		for (const url of [
			"javascript:alert(document.domain)",
			"JavaScript:alert(1)",
			"data:text/html,<script>alert(1)</script>",
			"ftp://example.com/hashbrown",
		]) {
			expect(Object.keys(problemsOf({ ...saved, url })), url).toEqual(["url"]);
		}
		expect(problemsOf({ ...saved, url: "http://example.com" })).toEqual({});
	});
});

describe("webUrl", () => {
	// Kills a stored URL drawn as a link whatever its scheme.
	test("is the URL only when it opens a web page", () => {
		expect(webUrl("https://github.com/pompeii-llc/hashbrown")).toBe(
			"https://github.com/pompeii-llc/hashbrown",
		);
		expect(webUrl("http://example.com")).toBe("http://example.com");
		expect(webUrl("javascript:alert(document.domain)")).toBeUndefined();
		expect(webUrl(" javascript:alert(1)")).toBeUndefined();
		expect(webUrl("data:text/html,x")).toBeUndefined();
		expect(webUrl(undefined)).toBeUndefined();
	});
});

describe("patched", () => {
	// Kills an optimistic project that misses a change, or keeps a removed URL.
	test("applies a change to the project the way the API will", () => {
		expect(
			patched(project, {
				name: "Hash Browns",
				slug: "hash-browns",
				visibility: Visibility.Private,
			}),
		).toEqual({
			...project,
			name: "Hash Browns",
			slug: "hash-browns",
			visibility: Visibility.Private,
		});
		const removed = patched(project, { url: null });
		expect(removed.url).toBeUndefined();
		expect(removed.name).toBe("Hashbrown");
	});
});

describe("refusalOf", () => {
	const refused = (status: number, message: string) =>
		new ApiError(status, "client", JSON.stringify({ message }));

	// Kills a plan refusal shown away from Visibility, or as the API's raw words.
	test("a missing plan belongs to Visibility", () => {
		expect(
			refusalOf(refused(402, "No plan"), { visibility: Visibility.Private }),
		).toEqual({
			field: "visibility",
			message: "Bencher kept the project public: private needs a paid plan.",
		});
	});

	// Kills a taken slug reported as a failure of the whole form.
	test("a conflict over a new slug belongs to Slug", () => {
		expect(refusalOf(refused(409, "UNIQUE"), { slug: "taken" }).field).toBe(
			"slug",
		);
		expect(refusalOf(refused(409, "UNIQUE"), { name: "x" }).field).toBe("form");
	});

	// Kills a refusal that hides why, and a lost connection read as a refusal.
	test("anything else says what the API said, or that it did not answer", () => {
		expect(refusalOf(refused(400, "Bad URL"), { url: "x" })).toEqual({
			field: "form",
			message: "Bencher did not save the changes: Bad URL",
		});
		expect(
			refusalOf(new ApiError(undefined, "network", "Failed"), { name: "x" }),
		).toEqual({
			field: "form",
			message: "The Bencher API did not answer, so nothing was saved.",
		});
	});
});
