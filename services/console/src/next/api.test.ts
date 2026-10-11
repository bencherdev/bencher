import { describe, expect, test } from "vitest";
import { ApiError, createApi } from "./api";

const respond =
	(status: number, body: unknown, headers: Record<string, string> = {}) =>
	async (url: string | URL | Request, init?: RequestInit) => {
		calls.push({ url: String(url), init });
		return new Response(JSON.stringify(body), { status, headers });
	};
let calls: { url: string; init: RequestInit | undefined }[] = [];

const api = (fetch: typeof globalThis.fetch) =>
	createApi({ url: "https://api.example.com", token: "tok", fetch });

describe("createApi", () => {
	// Kills a request sent to the wrong host or without the reader's token.
	test("sends the bearer token to the API", async () => {
		calls = [];
		const { data } = await api(respond(200, { name: "Hashbrown" })).get<{
			name: string;
		}>("/v0/projects/hashbrown");
		expect(data).toEqual({ name: "Hashbrown" });
		expect(calls[0]?.url).toBe("https://api.example.com/v0/projects/hashbrown");
		expect(new Headers(calls[0]?.init?.headers).get("authorization")).toBe(
			"Bearer tok",
		);
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

	// Kills a badly signed or expired token left signed in because the API
	// answers it with a 400, and every other 400 sending the reader to sign in.
	test("a 400 for a token the API cannot validate reads as refused", async () => {
		const refused = await api(
			respond(400, {
				request_id: "x",
				message:
					"Failed to validate JSON Web Token: InvalidSignature\nExpected format is `Authorization: Bearer <bencher.api.token>`.",
			}),
		)
			.get("/v0/x")
			.catch((error: ApiError) => error);
		expect((refused as ApiError).kind).toBe("unauthorized");

		const other = await api(
			respond(400, { message: "bad parameter in URL path" }),
		)
			.get("/v0/x")
			.catch((error: ApiError) => error);
		expect((other as ApiError).kind).toBe("client");

		const unreadable = await api(
			async () => new Response("<html>", { status: 400 }),
		)
			.get("/v0/x")
			.catch((error: ApiError) => error);
		expect((unreadable as ApiError).kind).toBe("client");
	});

	// Kills a session that runs past its end with every request refused.
	test("an ended session sends nothing and reads as refused", async () => {
		calls = [];
		const error = await createApi({
			url: "https://api.example.com",
			token: "tok",
			expiration: 1_000,
			now: () => 1_000,
			fetch: respond(200, {}),
		})
			.get("/v0/x")
			.catch((error: ApiError) => error);
		expect((error as ApiError).kind).toBe("unauthorized");
		expect(calls).toEqual([]);
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
});
