import { describe, expect, test } from "vitest";
import { isWebUrl, sameOriginUrl } from "./webUrl";

describe("isWebUrl", () => {
	test.each([
		"http://example.com",
		"https://example.com",
		"HTTPS://example.com/path?query=string#fragment",
	])("links %s", (url) => {
		expect(isWebUrl(url)).toBe(true);
	});

	test.each([
		"javascript:alert(document.domain)",
		"JavaScript:alert(document.domain)",
		" javascript:alert(document.domain)",
		"java\tscript:alert(document.domain)",
		"data:text/html,<script>alert(document.domain)</script>",
		"vbscript:msgbox(1)",
		"file:///etc/passwd",
		"mailto:user@example.com",
		"/console",
		"//example.com",
		"example.com",
		"",
		undefined,
		null,
	])("does not link %s", (url) => {
		expect(isWebUrl(url)).toBe(false);
	});
});

describe("sameOriginUrl", () => {
	const origin = "https://bencher.dev";

	test.each([
		[
			"/console/projects?per_page=8#top",
			"https://bencher.dev/console/projects?per_page=8#top",
		],
		["https://bencher.dev/console", "https://bencher.dev/console"],
		["https://bencher.dev//evil.com", "https://bencher.dev//evil.com"],
		["/.//evil.com", "https://bencher.dev//evil.com"],
		["/a/..//evil.com", "https://bencher.dev//evil.com"],
		["https://bencher.dev/\\evil.com", "https://bencher.dev//evil.com"],
		["/x/../\\\\evil.com", "https://bencher.dev///evil.com"],
	])("keeps %s on this origin", (url, expected) => {
		expect(sameOriginUrl(url, origin)).toBe(expected);
	});

	test.each([
		"javascript:alert(document.domain)",
		"data:text/html,<script>alert(document.domain)</script>",
		"https://example.com/console",
		"//example.com/console",
		"/\\example.com/console",
		"",
		undefined,
		null,
	])("drops %s", (url) => {
		expect(sameOriginUrl(url, origin)).toBeUndefined();
	});
});
