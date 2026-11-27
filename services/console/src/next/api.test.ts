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

	// Kills a change refused for want of a fresh sign-in that does not ask the
	// reader, that does not go again once they sign in again, or that goes
	// again after they cancel; and asking for any other refusal, or for a read.
	test("a change that needs a fresh sign-in asks for one, then goes again", async () => {
		const fresh = {
			error_code: "fresh_sign_in",
			message:
				"This needs a browser session signed in within the last 10 minutes",
		};
		const answering = (...answers: [number, unknown][]) => {
			const sent: (RequestInit | undefined)[] = [];
			const fetch = async (
				_url: string | URL | Request,
				init?: RequestInit,
			) => {
				const [status, body] = answers[
					Math.min(sent.length, answers.length - 1)
				] ?? [500, {}];
				sent.push(init);
				return new Response(JSON.stringify(body), { status });
			};
			return { fetch, sent };
		};
		const client = (fetch: typeof globalThis.fetch, signedIn: boolean) => {
			let asked = 0;
			const api = createApi({
				fetch,
				onUnauthorized: () => {},
				signInAgain: async () => {
					asked += 1;
					return signedIn;
				},
			});
			return { api, asked: () => asked };
		};

		const made = answering([403, fresh], [201, { name: "ci" }]);
		const signs = client(made.fetch, true);
		const { data } = await signs.api.send<{ name: string }>(
			"POST",
			"/v0/users/ada/keys",
			{ name: "ci" },
		);
		expect(data).toEqual({ name: "ci" });
		expect(signs.asked()).toBe(1);
		expect(made.sent.map((init) => [init?.method, init?.body])).toEqual([
			["POST", '{"name":"ci"}'],
			["POST", '{"name":"ci"}'],
		]);

		const cancelled = answering([403, fresh], [201, {}]);
		const cancels = client(cancelled.fetch, false);
		const error = await cancels.api
			.send("DELETE", "/v0/projects/hashbrown")
			.catch((error: ApiError) => error);
		expect((error as ApiError).status).toBe(403);
		expect((error as ApiError).message).toContain("fresh_sign_in");
		expect(cancels.asked()).toBe(1);
		expect(cancelled.sent).toHaveLength(1);

		for (const [status, body] of [
			[403, { error_code: "cross_origin", message: "no" }],
			[403, { message: "Only a user session can create a user API key." }],
			[429, fresh],
		] as const) {
			const refused = answering([status, body]);
			const asks = client(refused.fetch, true);
			await asks.api.send("POST", "/v0/x", {}).catch(() => undefined);
			expect(asks.asked(), JSON.stringify(body)).toBe(0);
		}
		const read = answering([403, fresh]);
		const reads = client(read.fetch, true);
		await reads.api.get("/v0/x").catch(() => undefined);
		expect(reads.asked()).toBe(0);
	});

	// Kills a change that asks again when the API refuses it for want of a
	// fresh sign-in right after the reader signed in again.
	test("a change asks for a fresh sign-in once", async () => {
		const fresh = { error_code: "fresh_sign_in", message: "Sign in again" };
		const sent: (RequestInit | undefined)[] = [];
		let asked = 0;
		const api = createApi({
			fetch: async (_url: string | URL | Request, init?: RequestInit) => {
				sent.push(init);
				return new Response(JSON.stringify(fresh), { status: 403 });
			},
			onUnauthorized: () => {},
			signInAgain: async () => {
				asked += 1;
				return true;
			},
		});
		const error = await api
			.send("POST", "/v0/users/ada/keys", { name: "ci" })
			.catch((error: ApiError) => error);
		expect((error as ApiError).status).toBe(403);
		expect((error as ApiError).message).toContain("fresh_sign_in");
		expect(asked).toBe(1);
		expect(sent).toHaveLength(2);
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
