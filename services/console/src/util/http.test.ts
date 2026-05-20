import { describe, expect, test } from "vitest";
import {
	apiUrl,
	apiHost,
	getOptions,
	postOptions,
	putOptions,
	pathOptions,
	deleteOptions,
} from "./http";

describe("apiHost", () => {
	test("returns hostname when provided", () => {
		expect(apiHost("https://api.bencher.dev")).toBe("https://api.bencher.dev");
	});
});

describe("apiUrl", () => {
	test("concatenates hostname and pathname", () => {
		expect(apiUrl("https://api.bencher.dev", "/v0/projects")).toBe(
			"https://api.bencher.dev/v0/projects",
		);
	});
});

describe("getOptions", () => {
	test("returns GET config with auth token", () => {
		const opts = getOptions(
			"https://api.bencher.dev",
			"/v0/projects",
			"my-token",
		);
		expect(opts.method).toBe("GET");
		expect(opts.url).toBe("https://api.bencher.dev/v0/projects");
		expect(opts.headers).toEqual({
			"Content-Type": "application/json",
			Authorization: "Bearer my-token",
		});
	});

	test("returns GET config without auth when token is null", () => {
		const opts = getOptions("https://api.bencher.dev", "/v0/projects", null);
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});

	test("returns GET config without auth when token is undefined", () => {
		const opts = getOptions(
			"https://api.bencher.dev",
			"/v0/projects",
			undefined,
		);
		expect(opts.headers).toEqual({ "Content-Type": "application/json" });
	});
});

describe("postOptions", () => {
	test("returns POST config with data", () => {
		const data = { name: "test" };
		const opts = postOptions(
			"https://api.bencher.dev",
			"/v0/projects",
			"tok",
			data,
		);
		expect(opts.method).toBe("POST");
		expect(opts.data).toEqual(data);
		expect(opts.headers.Authorization).toBe("Bearer tok");
	});
});

describe("putOptions", () => {
	test("returns PUT config with data", () => {
		const data = { name: "updated" };
		const opts = putOptions(
			"https://api.bencher.dev",
			"/v0/projects/slug",
			"tok",
			data,
		);
		expect(opts.method).toBe("PUT");
		expect(opts.data).toEqual(data);
	});
});

describe("pathOptions", () => {
	test("returns PATCH config", () => {
		const data = { name: "patched" };
		const opts = pathOptions(
			"https://api.bencher.dev",
			"/v0/projects/slug",
			"tok",
			data,
		);
		expect(opts.method).toBe("PATCH");
		expect(opts.data).toEqual(data);
	});
});

describe("deleteOptions", () => {
	test("returns DELETE config without data", () => {
		const opts = deleteOptions(
			"https://api.bencher.dev",
			"/v0/projects/slug",
			"tok",
		);
		expect(opts.method).toBe("DELETE");
		expect(opts.url).toBe("https://api.bencher.dev/v0/projects/slug");
		expect(opts.headers.Authorization).toBe("Bearer tok");
	});
});
