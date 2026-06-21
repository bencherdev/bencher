// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { apiHost } from "./http";

describe("apiHost fallback", () => {
	test("falls back to window.location when hostname is empty", () => {
		const result = apiHost("");
		expect(result).toContain("://");
		expect(result).toContain(":6610");
	});
});
