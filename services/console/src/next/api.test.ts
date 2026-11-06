import { describe, expect, test } from "vitest";
import { ApiError, createApi } from "./api";

const respond =
	(status: number, body: unknown, headers: Record<string, string> = {}) =>
	async (url: string | URL | Request, init?: RequestInit) => {
		calls.push({ url: String(url), init });
		return new Response(JSON.stringify(body), { status, headers });
	};
let calls: { url: string; init: RequestInit | undefined }[] = [];

const api = (fetch: typeof globalThis.fetch, onUnauthorized = () => {}) =>
	createApi({ fetch, onUnauthorized });

describe("createApi", () => {
	// Kills a request sent anywhere but the console's own relay, and one that
	// carries a credential of its own: the session cookie travels by itself.
	test("reads through the relay with no Authorization", async () => {
		calls = [];
		const { data } = await api(respond(200, { name: "Hashbrown" })).get<{
			name: string;
		}>("/v0/projects/hashbrown");
		expect(data).toEqual({ name: "Hashbrown" });
		expect(calls[0]?.url).toBe("/api/v0/projects/hashbrown");
		expect(new Headers(calls[0]?.init?.headers).has("authorization")).toBe(
			false,
		);
	});

	// Kills a refused session that leaves the reader where nothing loads, read
	// or change, and anything else that sends them to sign in.
	test("a 401 to a read or a change, and nothing else, ends the session", async () => {
		const ended = (status: number, change: boolean) => {
			let count = 0;
			const client = api(respond(status, { message: "no" }), () => {
				count += 1;
			});
			return (change ? client.send("PATCH", "/v0/x", {}) : client.get("/v0/x"))
				.catch(() => undefined)
				.then(() => count);
		};
		expect(await ended(401, false)).toBe(1);
		expect(await ended(401, true)).toBe(1);
		for (const status of [400, 403, 404, 429, 500]) {
			expect(await ended(status, false), String(status)).toBe(0);
			expect(await ended(status, true), String(status)).toBe(0);
		}
	});

	// Kills errors that lose what went wrong, which decides retry and sign in.
	test("names what went wrong", async () => {
		const kinds = await Promise.all(
			[
				[401, "unauthorized"],
				[403, "forbidden"],
				[404, "not_found"],
				[400, "client"],
				[503, "server"],
			].map(async ([status]) =>
				api(respond(status as number, { message: "no" }))
					.get("/v0/x")
					.catch((error: ApiError) => [error.status, error.kind]),
			),
		);
		expect(kinds).toEqual([
			[401, "unauthorized"],
			[403, "forbidden"],
			[404, "not_found"],
			[400, "client"],
			[503, "server"],
		]);
	});

	// Kills a lost connection reported as an answer from the API.
	test("names a request that never reached the API", async () => {
		const error = await api(async () => {
			throw new TypeError("Failed to fetch");
		})
			.get("/v0/x")
			.catch((error: ApiError) => error);
		expect(error).toBeInstanceOf(ApiError);
		expect((error as ApiError).kind).toBe("network");
	});

	// Kills a cancelled query turned into an error the reader would see.
	test("lets a cancelled request stay cancelled", async () => {
		const abort = new DOMException("aborted", "AbortError");
		const error = await api(async () => {
			throw abort;
		})
			.get("/v0/x")
			.catch((error: unknown) => error);
		expect(error).toBe(abort);
	});

	// Kills a change sent without its body or its method, or with a credential.
	test("sends a change as JSON with no Authorization", async () => {
		calls = [];
		const { data } = await api(respond(200, { name: "Hash Browns" })).send<{
			name: string;
		}>("PATCH", "/v0/projects/hashbrown", { name: "Hash Browns" });
		expect(data).toEqual({ name: "Hash Browns" });
		const [call] = calls;
		expect(call?.url).toBe("/api/v0/projects/hashbrown");
		expect(call?.init?.method).toBe("PATCH");
		expect(call?.init?.body).toBe('{"name":"Hash Browns"}');
		const headers = new Headers(call?.init?.headers);
		expect(headers.has("authorization")).toBe(false);
		expect(headers.get("content-type")).toBe("application/json");
	});

	// Kills a delete that fails on the API's empty answer.
	test("a change answered with no body resolves", async () => {
		calls = [];
		const deleted = await api(
			async (url: string | URL | Request, init?: RequestInit) => {
				calls.push({ url: String(url), init });
				return new Response(null, { status: 204 });
			},
		).send("DELETE", "/v0/projects/hashbrown");
		expect(deleted.data).toBeUndefined();
		expect(calls[0]?.init?.method).toBe("DELETE");
		expect(calls[0]?.init?.body).toBeUndefined();
	});

	// Kills a refused change that loses the status a page explains it by.
	test("a refused change keeps its status and the API's message", async () => {
		const error = await api(respond(402, { message: "No plan" }))
			.send("PATCH", "/v0/projects/hashbrown", { visibility: "private" })
			.catch((error: ApiError) => error);
		expect(error).toBeInstanceOf(ApiError);
		expect((error as ApiError).status).toBe(402);
		expect((error as ApiError).kind).toBe("client");
		expect((error as ApiError).message).toContain("No plan");
	});
});
