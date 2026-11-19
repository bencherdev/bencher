import { API_PATH } from "../next/api";

export interface RelayConfig {
	/** The API as the console's server reaches it; wins over `apiUrl`. */
	internalApiUrl: string | undefined;
	/** The API's public URL. */
	apiUrl: string;
	/** Shared with the API, which then trusts the client address the relay sends. */
	secret: string | undefined;
	fetch?: typeof globalThis.fetch;
}

/** The browser's request, as the adapter hands it over. */
interface Relayed {
	request: Request;
	/** The reader's address: `CF-Connecting-IP` on the Worker, the socket's peer on Node. */
	readonly clientAddress: string;
}

const RELAY_SECRET = "Bencher-Relay-Secret";
const RELAY_CLIENT_IP = "Bencher-Client-IP";

const FORWARDED = [
	"accept",
	"content-type",
	"cookie",
	"origin",
	"sec-fetch-dest",
	"sec-fetch-mode",
	"sec-fetch-site",
	"user-agent",
];

// https://www.rfc-editor.org/rfc/rfc9110#section-7.6.1
const HOP_BY_HOP = [
	"connection",
	"keep-alive",
	"proxy-authenticate",
	"proxy-authorization",
	"proxy-connection",
	"te",
	"trailer",
	"transfer-encoding",
	"upgrade",
];

/** Sends the browser's request on to the API, and the API's answer back. */
export const relay = async (
	relayed: Relayed,
	{
		internalApiUrl,
		apiUrl,
		secret,
		fetch = globalThis.fetch.bind(globalThis),
	}: RelayConfig,
): Promise<Response> => {
	const { request } = relayed;
	const headers = new Headers();
	for (const name of FORWARDED) {
		const value = request.headers.get(name);
		if (value !== null) {
			headers.set(name, value);
		}
	}
	if (secret) {
		headers.set(RELAY_SECRET, secret);
		const address = clientAddress(relayed);
		if (address) {
			headers.set(RELAY_CLIENT_IP, address);
		}
	}

	const init: RequestInit & { duplex: "half" } = {
		method: request.method,
		headers,
		body: request.body,
		redirect: "manual",
		cache: "no-store",
		signal: request.signal,
		duplex: "half",
	};
	let response: Response;
	try {
		response = await fetch(upstream(request, internalApiUrl || apiUrl), init);
	} catch (error) {
		if (!request.signal.aborted) {
			console.error("The API did not answer the relay", error);
		}
		return new Response(JSON.stringify({ message: "The API did not answer" }), {
			status: 502,
			headers: {
				"cache-control": "no-store",
				"content-type": "application/json",
			},
		});
	}

	const answer = new Headers();
	const hop = hopByHop(response.headers);
	for (const [name, value] of response.headers) {
		if (!hop.has(name)) {
			answer.append(name, value);
		}
	}
	// Fetch has already inflated the body.
	if (answer.has("content-encoding")) {
		answer.delete("content-encoding");
		answer.delete("content-length");
	}
	if (!answer.has("cache-control")) {
		answer.set("cache-control", "no-store");
	}
	return new Response(response.body, {
		status: response.status,
		statusText: response.statusText,
		headers: answer,
	});
};

/** Where `request` goes on the API, under any path the API's URL has. */
const upstream = (request: Request, apiUrl: string) => {
	const url = new URL(apiUrl);
	const from = new URL(request.url);
	url.pathname = `${url.pathname.replace(/\/$/, "")}${from.pathname.slice(API_PATH.length)}`;
	url.search = from.search;
	return url;
};

const clientAddress = (relayed: Relayed) => {
	try {
		return relayed.clientAddress;
	} catch {
		return undefined;
	}
};

/** The headers of one hop alone: the standard ones, and any `Connection` names. */
const hopByHop = (headers: Headers) =>
	new Set([
		...HOP_BY_HOP,
		...(headers.get("connection") ?? "")
			.split(",")
			.map((name) => name.trim().toLowerCase())
			.filter(Boolean),
	]);
