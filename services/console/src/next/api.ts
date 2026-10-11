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

export interface Api {
	get<T>(path: string, signal?: AbortSignal): Promise<ApiResponse<T>>;
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
}): Api => ({
	async get<T>(path: string, signal?: AbortSignal) {
		// The API answers an expired token with a 400, so the client does not ask.
		if (now() >= expiration) {
			throw new ApiError(undefined, "unauthorized", "The session has ended");
		}
		let response: Response;
		try {
			response = await fetch(`${url}${path}`, {
				headers: { Authorization: `Bearer ${token}` },
				signal: signal ?? null,
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
		return { data: (await response.json()) as T, headers: response.headers };
	},
});

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
