import { AxiosError, AxiosHeaders } from "axios";
import { describe, expect, test } from "vitest";
import {
	apiErrorMessage,
	apiUrl,
	apiHost,
	getOptions,
	postOptions,
	putOptions,
	pathOptions,
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

const FALLBACK = "Invalid user. Please, try again.";

/** What axios hands a catch for a request the API refused with `data`. */
const refused = (data: unknown) => {
	const headers = new AxiosHeaders();
	const config = { headers };
	return new AxiosError(
		"Request failed with status code 400",
		"ERR_BAD_REQUEST",
		config,
		null,
		{
			status: 400,
			statusText: "Bad Request",
			headers,
			config,
			data,
		},
	);
};

describe("apiErrorMessage", () => {
	test("returns the message the API sent", () => {
		const error = refused({
			request_id: "0",
			error_code: "Locked",
			message: "This user account has been locked",
		});
		expect(apiErrorMessage(error, FALLBACK)).toBe(
			"This user account has been locked",
		);
	});

	test("caps a very long message", () => {
		const message = "no ".repeat(256);
		const capped = apiErrorMessage(refused({ message }), FALLBACK);
		expect(capped.length).toBe(256);
		expect(message.startsWith(capped)).toBe(true);
	});

	test("keeps the first line of a message the API sent over two", () => {
		const message =
			"Failed to create client JWT for OAuth2: boom\nPlease report this issue: https://github.com/bencherdev/bencher/issues/new?title=Failed";
		expect(apiErrorMessage(refused({ message }), FALLBACK)).toBe(
			"Failed to create client JWT for OAuth2: boom",
		);
	});

	test("caps on a whole character, never half of a surrogate pair", () => {
		const message = `${"a".repeat(255)}\u{1F600} tail`;
		const capped = apiErrorMessage(refused({ message }), FALLBACK);
		expect(Array.from(capped)).toHaveLength(256);
		expect(capped.endsWith("\u{1F600}")).toBe(true);
	});

	test("falls back when the message starts with a line break", () => {
		const message =
			"\nPlease report this issue: https://github.com/bencherdev/bencher/issues/new";
		expect(apiErrorMessage(refused({ message }), FALLBACK)).toBe(FALLBACK);
	});

	test("falls back when the body carries no message", () => {
		expect(apiErrorMessage(refused({ request_id: "0" }), FALLBACK)).toBe(
			FALLBACK,
		);
	});

	test("falls back when the message is not text", () => {
		expect(apiErrorMessage(refused({ message: { why: "no" } }), FALLBACK)).toBe(
			FALLBACK,
		);
	});

	test("falls back when the message is blank", () => {
		expect(apiErrorMessage(refused({ message: "  " }), FALLBACK)).toBe(
			FALLBACK,
		);
	});

	test("falls back when the request never reached the API", () => {
		const error = new AxiosError("Network Error", "ERR_NETWORK", {
			headers: new AxiosHeaders(),
		});
		expect(apiErrorMessage(error, FALLBACK)).toBe(FALLBACK);
	});
});
