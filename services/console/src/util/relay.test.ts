import {
	type IncomingMessage,
	type Server,
	type ServerResponse,
	createServer,
} from "node:http";
import type { AddressInfo } from "node:net";
import { gzipSync } from "node:zlib";
import { afterEach, describe, expect, test, vi } from "vitest";
import { type RelayConfig, relay } from "./relay";

/** What the API received. */
interface Received {
	method: string;
	url: string;
	/** Every header as sent, a repeated one once per copy. */
	headers: [string, string][];
	body: string;
	/** Resolves once the relay's connection to the API closes. */
	closed: Promise<void>;
}

type Answer = (request: IncomingMessage, response: ServerResponse) => void;

const servers: Server[] = [];

afterEach(async () => {
	vi.restoreAllMocks();
	await Promise.all(
		servers
			.splice(0)
			.map(
				(server) =>
					new Promise((resolve) => server.close(resolve).closeAllConnections()),
			),
	);
});

/** An API on a free port that answers with `answer` and records what it received. */
const api = async (answer: Answer = (_, response) => response.end()) => {
	const received: Received[] = [];
	const server = createServer((request, response) => {
		const chunks: Buffer[] = [];
		const closed = new Promise<void>((resolve) =>
			response.once("close", resolve),
		);
		request.on("data", (chunk: Buffer) => chunks.push(chunk));
		request.on("end", () => {
			const headers: [string, string][] = [];
			for (let i = 0; i < request.rawHeaders.length; i += 2) {
				headers.push([
					(request.rawHeaders[i] ?? "").toLowerCase(),
					request.rawHeaders[i + 1] ?? "",
				]);
			}
			received.push({
				method: request.method ?? "",
				url: request.url ?? "",
				headers,
				body: Buffer.concat(chunks).toString(),
				closed,
			});
			answer(request, response);
		});
	});
	servers.push(server);
	await new Promise<void>((resolve) =>
		server.listen(0, "127.0.0.1", () => resolve()),
	);
	const { port } = server.address() as AddressInfo;
	return { url: `http://127.0.0.1:${port}`, received };
};

/** A port nothing listens on. */
const closedPort = async () => {
	const server = createServer();
	await new Promise<void>((resolve) =>
		server.listen(0, "127.0.0.1", () => resolve()),
	);
	const { port } = server.address() as AddressInfo;
	await new Promise((resolve) => server.close(resolve));
	return port;
};

const CONSOLE = "https://bencher.example";

/** The browser's request to the console, as the adapter hands it to the relay; with no address, as Astro's throws. */
const reader = (
	path: string,
	init: RequestInit = {},
	clientAddress: string | null = "198.51.100.7",
) => ({
	request: new Request(`${CONSOLE}${path}`, init),
	get clientAddress(): string {
		if (clientAddress === null) {
			throw new Error("No client address");
		}
		return clientAddress;
	},
});

const to = (
	apiUrl: string,
	config: Partial<RelayConfig> = {},
): RelayConfig => ({
	internalApiUrl: undefined,
	apiUrl,
	secret: undefined,
	...config,
});

const valuesOf = (received: Received | undefined, name: string) =>
	(received?.headers ?? [])
		.filter(([key]) => key === name)
		.map(([, value]) => value);

const LISTED = {
	accept: "application/json",
	"content-type": "application/json",
	cookie: "__Host-bencher_session=a; other=b",
	origin: CONSOLE,
	"sec-fetch-dest": "empty",
	"sec-fetch-mode": "no-cors",
	"sec-fetch-site": "same-origin",
	"user-agent": "Mozilla/5.0 (X11; Linux x86_64) Chrome/140.0",
};

describe("relay", () => {
	// Kills a relay that drops or rewrites the method, the path, its escapes, the
	// query, the body, or a listed header, or that lets any other header through,
	// such as one the API reads a client address from, or a credential.
	test("forwards the method, path, query, body, and only the listed headers", async () => {
		const { url, received } = await api();
		const handed: Headers[] = [];
		const fetch: typeof globalThis.fetch = (input, init) => {
			handed.push(new Headers(init?.headers));
			return globalThis.fetch(input, init);
		};
		const body = JSON.stringify({ name: "x".repeat(1 << 17) });
		const response = await relay(
			reader("/api/v0/projects/a%2Fb/reports?branch=main&q=%2F%20", {
				method: "POST",
				body,
				headers: {
					...LISTED,
					authorization: "Bearer bencher_user_key",
					"x-forwarded-for": "203.0.113.1",
					"fly-client-ip": "203.0.113.2",
					"cf-connecting-ip": "203.0.113.3",
					"x-real-ip": "203.0.113.4",
					forwarded: "for=203.0.113.5",
					"x-bencher-other": "unlisted",
					referer: `${CONSOLE}/console`,
				},
			}),
			to(url, { fetch }),
		);

		expect(response.status).toBe(200);
		const [sent] = received;
		expect(sent?.method).toBe("POST");
		expect(sent?.url).toBe("/v0/projects/a%2Fb/reports?branch=main&q=%2F%20");
		expect(sent?.body).toBe(body);
		for (const [name, value] of Object.entries(LISTED)) {
			expect(handed[0]?.get(name), name).toBe(value);
			// Node's fetch puts its own Sec-Fetch-Mode on the wire.
			if (name !== "sec-fetch-mode") {
				expect(valuesOf(sent, name), name).toEqual([value]);
			}
		}
		for (const name of [
			"authorization",
			"x-forwarded-for",
			"fly-client-ip",
			"cf-connecting-ip",
			"x-real-ip",
			"forwarded",
			"x-bencher-other",
			"referer",
		]) {
			expect(valuesOf(sent, name), name).toEqual([]);
		}
	});

	// Kills a relay that sends a self-hosted console's Basic credentials, meant
	// for the proxy in front of it, on to the API, or any other credential: the
	// console signs in with its session cookie alone.
	test("never forwards `Authorization`", async () => {
		const { url, received } = await api();
		for (const authorization of [
			"Basic dXNlcjpwYXNz",
			"bearer bencher_user_key",
			"Digest username=x",
			'Digest username="bearer x"',
		]) {
			await relay(
				reader("/api/v0/user", { headers: { authorization } }),
				to(url),
			);
		}
		expect(received.map((sent) => valuesOf(sent, "authorization"))).toEqual([
			[],
			[],
			[],
			[],
		]);
	});

	// Kills an upstream URL built from the path, where a path that reads like a
	// host would send the request, and its credentials, to that host; and a
	// relay that drops the path an API served under a prefix lives at.
	test("keeps every request on the API's own URL", async () => {
		const { url, received } = await api();
		const internal = await relay(
			reader("/api//evil.example/v0/user", {
				headers: { authorization: "Bearer k" },
			}),
			to(url),
		);
		const prefixed = await relay(
			reader("/api/v0/user?x=1"),
			to(`${url}/bencher/`),
		);
		const bare = await relay(reader("/api"), to(url));

		expect([internal.status, prefixed.status, bare.status]).toEqual([
			200, 200, 200,
		]);
		expect(received.map((sent) => sent.url)).toEqual([
			"//evil.example/v0/user",
			"/bencher/v0/user?x=1",
			"/",
		]);
	});

	// Kills a relay that loses a status, a second `Set-Cookie`, the sign in's
	// `Bencher-Tries-Left` or `Retry-After`, or any byte of the body.
	test("returns the API's status, headers, and body unchanged", async () => {
		const body = JSON.stringify({ message: "Wrong code", items: [1, 2, 3] });
		const { url } = await api((_, response) => {
			response.writeHead(403, {
				"content-type": "application/json",
				"set-cookie": [
					"__Host-bencher_session=s; HttpOnly; Secure; SameSite=Strict; Path=/",
					"__Host-bencher_sign_in=; Max-Age=0; Path=/",
				],
				"bencher-tries-left": "4",
				"retry-after": "60",
				"x-total-count": "3",
				"x-request-id": "r1",
			});
			response.end(body);
		});
		const response = await relay(
			reader("/api/v0/auth/code", { method: "POST", body: "{}" }),
			to(url),
		);

		expect(response.status).toBe(403);
		expect(response.headers.getSetCookie()).toEqual([
			"__Host-bencher_session=s; HttpOnly; Secure; SameSite=Strict; Path=/",
			"__Host-bencher_sign_in=; Max-Age=0; Path=/",
		]);
		for (const [name, value] of [
			["content-type", "application/json"],
			["bencher-tries-left", "4"],
			["retry-after", "60"],
			["x-total-count", "3"],
			["x-request-id", "r1"],
		]) {
			expect(response.headers.get(name as string), name).toBe(value);
		}
		expect(await response.text()).toBe(body);
	});

	// Kills a relay that copies the hop between the API and itself onto the
	// hop to the browser: a `Transfer-Encoding` it did not apply garbles the
	// body, and a `Connection` or a header it names belongs to that hop alone.
	test("drops hop-by-hop headers from the answer", async () => {
		const { url } = await api((_, response) => {
			response.writeHead(200, {
				connection: "keep-alive, x-hop",
				"keep-alive": "timeout=5",
				"x-hop": "1",
				"content-type": "application/json",
			});
			response.write('{"a":');
			response.end("1}");
		});
		const response = await relay(reader("/api/v0/x"), to(url));

		for (const name of [
			"connection",
			"keep-alive",
			"transfer-encoding",
			"x-hop",
		]) {
			expect(response.headers.get(name), name).toBeNull();
		}
		expect(await response.json()).toEqual({ a: 1 });
	});

	// Kills a relay that names a compression it already undid: the browser
	// would try to inflate a body that is no longer compressed.
	test("answers with the body the API compressed, decoded", async () => {
		const body = JSON.stringify({ name: "y".repeat(4096) });
		const { url } = await api((_, response) => {
			const gzipped = gzipSync(body);
			response.writeHead(200, {
				"content-type": "application/json",
				"content-encoding": "gzip",
				"content-length": gzipped.length,
			});
			response.end(gzipped);
		});
		const response = await relay(reader("/api/v0/x"), to(url));

		expect(response.headers.get("content-encoding")).toBeNull();
		expect(response.headers.get("content-length")).toBeNull();
		expect(await response.text()).toBe(body);
	});

	// Kills a relay that follows the API's redirect itself, so the browser
	// never sees where the API sent it, or that breaks on an answer with no body.
	test("hands a redirect and an empty answer to the browser as they are", async () => {
		const { url, received } = await api((request, response) => {
			if (request.url === "/v0/moved") {
				response.writeHead(302, { location: "/v0/elsewhere" });
			} else {
				response.writeHead(204);
			}
			response.end();
		});
		const moved = await relay(reader("/api/v0/moved"), to(url));
		const empty = await relay(
			reader("/api/v0/thing", { method: "DELETE" }),
			to(url),
		);

		expect(moved.status).toBe(302);
		expect(moved.headers.get("location")).toBe("/v0/elsewhere");
		expect(empty.status).toBe(204);
		expect(received.map((sent) => sent.url)).toEqual([
			"/v0/moved",
			"/v0/thing",
		]);
	});

	// Kills a relay that passes on a browser's own relay secret or client
	// address, takes the address from a header the browser sets, or sends
	// either header without a secret.
	test("sets its own relay secret and client address, and only with a secret", async () => {
		const { url, received } = await api();
		const forged = () => {
			const headers = new Headers();
			headers.append("Bencher-Relay-Secret", "forged");
			headers.append("Bencher-Relay-Secret", "forged-again");
			headers.append("Bencher-Client-IP", "203.0.113.9");
			headers.append("Bencher-Client-IP", "203.0.113.10");
			headers.set("X-Forwarded-For", "203.0.113.11");
			headers.set("CF-Connecting-IP", "203.0.113.12");
			return headers;
		};

		await relay(
			reader("/api/v0/x", { headers: forged() }),
			to(url, { secret: "relay-secret" }),
		);
		await relay(reader("/api/v0/x", { headers: forged() }), to(url));
		await relay(
			reader("/api/v0/x", { headers: forged() }, null),
			to(url, { secret: "relay-secret" }),
		);

		const [secret, none, nowhere] = received;
		expect(valuesOf(secret, "bencher-relay-secret")).toEqual(["relay-secret"]);
		expect(valuesOf(secret, "bencher-client-ip")).toEqual(["198.51.100.7"]);
		expect(valuesOf(none, "bencher-relay-secret")).toEqual([]);
		expect(valuesOf(none, "bencher-client-ip")).toEqual([]);
		expect(valuesOf(nowhere, "bencher-relay-secret")).toEqual(["relay-secret"]);
		expect(valuesOf(nowhere, "bencher-client-ip")).toEqual([]);
	});

	// Kills a relay that prefers the API's public URL when the console's server
	// has its own way to reach it, as a container does.
	test("`INTERNAL_API_URL` wins over `BENCHER_API_URL`", async () => {
		const internal = await api();
		const external = await api();

		await relay(
			reader("/api/v0/x"),
			to(external.url, { internalApiUrl: internal.url }),
		);
		await relay(reader("/api/v0/y"), to(external.url));

		expect(internal.received.map((sent) => sent.url)).toEqual(["/v0/x"]);
		expect(external.received.map((sent) => sent.url)).toEqual(["/v0/y"]);
	});

	// Kills a failure that tells the browser where the API lives, or that
	// throws instead of answering.
	test("an unreachable API is a 502 that names no upstream", async () => {
		const error = vi.spyOn(console, "error").mockImplementation(() => {});
		const port = await closedPort();
		const internal = `http://127.0.0.1:${port}`;
		const response = await relay(
			reader("/api/v0/x"),
			to("https://api.bencher.example", {
				internalApiUrl: internal,
				secret: "relay-secret",
			}),
		);

		expect(response.status).toBe(502);
		const text = await response.text();
		const answer = `${[...response.headers].flat().join("\n")}\n${text}`;
		for (const upstream of [
			"127.0.0.1",
			String(port),
			"api.bencher.example",
			"relay-secret",
		]) {
			expect(answer).not.toContain(upstream);
		}
		expect(JSON.parse(text)).toEqual({ message: expect.any(String) });
		expect(response.headers.get("cache-control")).toBe("no-store");
		expect(error).toHaveBeenCalledOnce();
		expect(error.mock.calls.flat().join(" ")).not.toContain("relay-secret");
	});

	// Kills a relay that throws on an API URL that is no URL, which the adapter
	// answers with its 500 page in place of the 502 every other failure to
	// reach the API gets.
	test("an API URL that is empty or no URL is the same 502", async () => {
		const error = vi.spyOn(console, "error").mockImplementation(() => {});
		for (const apiUrl of ["", "api.bencher.example"]) {
			const response = await relay(reader("/api/v0/x"), to(apiUrl));
			expect(response.status, apiUrl).toBe(502);
			expect(await response.json(), apiUrl).toEqual({
				message: "The API did not answer",
			});
		}
		expect(error).toHaveBeenCalledTimes(2);
	});

	// Kills a cache in front of the relay or behind it serving one reader's
	// answer to another: nothing may store an answer the API left unmarked,
	// with a cookie or a bearer credential, and the subrequest asks no cache
	// to answer it.
	test("caches nothing, and marks every answer the API left unmarked `no-store`", async () => {
		const { url } = await api((request, response) => {
			if (request.url === "/v0/metadata") {
				response.setHeader("cache-control", "public, max-age=300");
			}
			response.end("{}");
		});
		const modes: (RequestCache | undefined)[] = [];
		const fetch: typeof globalThis.fetch = (input, init) => {
			modes.push(init?.cache);
			return globalThis.fetch(input, init);
		};

		const unmarked = await relay(
			reader("/api/v0/user", {
				headers: { cookie: "__Host-bencher_session=a" },
			}),
			to(url, { fetch }),
		);
		const bearer = await relay(
			reader("/api/v0/user", { headers: { authorization: "Bearer k" } }),
			to(url, { fetch }),
		);
		const marked = await relay(reader("/api/v0/metadata"), to(url, { fetch }));

		expect(unmarked.headers.get("cache-control")).toBe("no-store");
		expect(bearer.headers.get("cache-control")).toBe("no-store");
		expect(marked.headers.get("cache-control")).toBe("public, max-age=300");
		expect(modes).toEqual(["no-store", "no-store", "no-store"]);
	});

	// Kills a relay that keeps the API working for a reader who has gone, and
	// one that reports the reader leaving as the API failing.
	test("a reader who leaves ends the API's request, and logs nothing", async () => {
		const error = vi.spyOn(console, "error").mockImplementation(() => {});
		const { url, received } = await api(() => {});
		const leave = new AbortController();
		const relayed = relay(
			reader("/api/v0/slow", { signal: leave.signal }),
			to(url),
		);
		await vi.waitFor(() => expect(received).toHaveLength(1));

		leave.abort();
		await received[0]?.closed;

		expect((await relayed).status).toBe(502);
		expect(error).not.toHaveBeenCalled();
	});
});
