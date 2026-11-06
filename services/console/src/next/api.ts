import { MAX_URL_LENGTH, URI_TOO_LONG } from "./url";

/** Where the browser reaches the API: the console's own origin, which relays it. */
export const API_PATH = "/api";

/** Why a request failed, which decides whether to retry or send the reader to sign in. */
export type ApiErrorKind =
	| "unauthorized"
	| "forbidden"
	| "not_found"
	| "client"
	| "server"
	| "network";

export class ApiError extends Error {
	readonly status: number | undefined;
	readonly kind: ApiErrorKind;

	constructor(status: number | undefined, kind: ApiErrorKind, message: string) {
		super(message);
		this.name = "ApiError";
		this.status = status;
		this.kind = kind;
	}
}

interface ApiResponse<T> {
	data: T;
	headers: Headers;
}

export type Method = "POST" | "PUT" | "PATCH" | "DELETE";

export interface Api {
	get<T>(path: string, signal?: AbortSignal): Promise<ApiResponse<T>>;
	/** A change, with its body as JSON; an empty answer resolves to `undefined`. */
	send<T>(
		method: Method,
		path: string,
		body?: unknown,
	): Promise<ApiResponse<T>>;
}

export const createApi = ({
	fetch = globalThis.fetch.bind(globalThis),
	onUnauthorized,
}: {
	fetch?: typeof globalThis.fetch;
	/** The session ended: the API refused a read or a change with a 401. */
	onUnauthorized: () => void;
}): Api => {
	const request = async <T>(
		path: string,
		init: { method?: Method; body?: unknown; signal?: AbortSignal | undefined },
	): Promise<ApiResponse<T>> => {
		const target = `${API_PATH}${path}`;
		// A request past what every hop passes on would be refused, so it is never sent.
		if (target.length > MAX_URL_LENGTH) {
			throw new ApiError(URI_TOO_LONG, "client", "URI Too Long");
		}
		const headers: Record<string, string> = {};
		if (init.body !== undefined) {
			headers["Content-Type"] = "application/json";
		}
		let response: Response;
		try {
			response = await fetch(target, {
				...(init.method ? { method: init.method } : {}),
				headers,
				...(init.body === undefined ? {} : { body: JSON.stringify(init.body) }),
				signal: init.signal ?? null,
			});
		} catch (error) {
			if (error instanceof DOMException && error.name === "AbortError") {
				throw error;
			}
			throw new ApiError(undefined, "network", String(error));
		}
		if (!response.ok) {
			const body = await response.text().catch(() => "");
			const kind = kindOf(response.status);
			if (kind === "unauthorized") {
				onUnauthorized();
			}
			throw new ApiError(response.status, kind, body);
		}
		const text = await response.text();
		return {
			data: (text ? JSON.parse(text) : undefined) as T,
			headers: response.headers,
		};
	};
	return {
		get: (path, signal) => request(path, { signal }),
		send: (method, path, body) => request(path, { method, body }),
	};
};

const kindOf = (status: number): ApiErrorKind => {
	switch (status) {
		case 401:
			return "unauthorized";
		case 403:
			return "forbidden";
		case 404:
			return "not_found";
		default:
			return status >= 500 ? "server" : "client";
	}
};
