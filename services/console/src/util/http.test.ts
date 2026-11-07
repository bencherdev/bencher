import { describe, expect, test } from "vitest";
import {
	apiUrl,
	getOptions,
	postOptions,
	putOptions,
	pathOptions,
} from "./http";

describe("apiUrl", () => {
	// Kills a classic page that reads the API anywhere but the console's own relay.
	test("reads through the console's own relay", () => {
		expect(apiUrl("/v0/projects")).toBe("/api/v0/projects");
	});
});

describe("getOptions", () => {
	test("returns GET config with auth token", () => {
		const opts = getOptions("/v0/projects", "my-token");
		expect(opts.method).toBe("GET");
		expect(opts.url).toBe("/api/v0/projects");
		expect(opts.headers).toEqual({
			"Content-Type": "application/json",
			Authorization: "Bearer my-token",
		});
	});

	test("returns GET config without auth when token is null", () => {
		const opts = getOptions("/v0/projects", null);
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});

	test("returns GET config without auth when token is undefined", () => {
		const opts = getOptions("/v0/projects", undefined);
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});
});

describe("postOptions", () => {
	test("returns POST config with data", () => {
		const data = { name: "test" };
		const opts = postOptions("/v0/projects", "tok", data);
		expect(opts.method).toBe("POST");
		expect(opts.data).toEqual(data);
		expect(opts.headers.Authorization).toBe("Bearer tok");
	});
});

describe("putOptions", () => {
	test("returns PUT config with data", () => {
		const data = { name: "updated" };
		const opts = putOptions("/v0/projects/slug", "tok", data);
		expect(opts.method).toBe("PUT");
		expect(opts.data).toEqual(data);
	});
});

describe("pathOptions", () => {
	test("returns PATCH config", () => {
		const data = { name: "patched" };
		const opts = pathOptions("/v0/projects/slug", "tok", data);
		expect(opts.method).toBe("PATCH");
		expect(opts.data).toEqual(data);
	});
});
