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
	// Kills a classic page that sends a credential of its own: the session
	// cookie travels by itself.
	test("returns GET config with no Authorization", () => {
		const opts = getOptions("/v0/projects");
		expect(opts.method).toBe("GET");
		expect(opts.url).toBe("/api/v0/projects");
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});
});

describe("postOptions", () => {
	test("returns POST config with data and no Authorization", () => {
		const data = { name: "test" };
		const opts = postOptions("/v0/projects", data);
		expect(opts.method).toBe("POST");
		expect(opts.data).toEqual(data);
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});
});

describe("putOptions", () => {
	test("returns PUT config with data", () => {
		const data = { name: "updated" };
		const opts = putOptions("/v0/projects/slug", data);
		expect(opts.method).toBe("PUT");
		expect(opts.data).toEqual(data);
	});
});

describe("pathOptions", () => {
	test("returns PATCH config", () => {
		const data = { name: "patched" };
		const opts = pathOptions("/v0/projects/slug", data);
		expect(opts.method).toBe("PATCH");
		expect(opts.data).toEqual(data);
	});
});
