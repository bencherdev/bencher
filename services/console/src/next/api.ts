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

export type Method = "POST" | "PATCH" | "DELETE";

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
	url,
	token,
	expiration = Number.POSITIVE_INFINITY,
	now = Date.now,
	fetch = globalThis.fetch.bind(globalThis),
}: {
	url: string;
	token: string;
	/** When the session ends; past it nothing is sent. */
	expiration?: number;
	now?: () => number;
	fetch?: typeof globalThis.fetch;
}): Api => {
	const request = async <T>(
		path: string,
		init: { method?: Method; body?: unknown; signal?: AbortSignal | undefined },
	): Promise<ApiResponse<T>> => {
		// The API answers an expired token with a 400, so the client does not ask.
		if (now() >= expiration) {
			throw new ApiError(undefined, "unauthorized", "The session has ended");
		}
		const headers: Record<string, string> = {
			Authorization: `Bearer ${token}`,
		};
		if (init.body !== undefined) {
			headers["Content-Type"] = "application/json";
		}
		let response: Response;
		try {
			response = await fetch(`${url}${path}`, {
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
			throw new ApiError(response.status, kindOf(response.status, body), body);
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

// The API answers a token it cannot validate, badly signed or expired, with a
// 400 that says so, rather than a 401.
const BAD_TOKEN = "Failed to validate JSON Web Token";

const kindOf = (status: number, body: string): ApiErrorKind => {
	switch (status) {
		case 400:
			return isBadToken(body) ? "unauthorized" : "client";
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

const isBadToken = (body: string) => {
	try {
		const message = JSON.parse(body)?.message;
		return typeof message === "string" && message.startsWith(BAD_TOKEN);
	} catch {
		return false;
	}
};
