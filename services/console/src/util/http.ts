import type { AxiosError, InternalAxiosRequestConfig } from "axios";
import axios from "axios";
import { ApiState, setApiState } from "./connectivity";

export const X_TOTAL_COUNT = "x-total-count";

enum HttpMethod {
	GET = "GET",
	POST = "POST",
	PUT = "PUT",
	PATCH = "PATCH",
	DELETE = "DELETE",
}

const MAX_ATTEMPTS = 10;
const RETRY_INITIAL_DELAY_MS = 1000;
const RETRY_BACKOFF_MULTIPLIER = 2;
const RETRY_MAX_DELAY_MS = 5000;
const RETRYABLE_STATUSES = [502, 503, 504];

interface RetryConfig extends InternalAxiosRequestConfig {
	__retryCount?: number;
}

const isRetryableError = (error: AxiosError): boolean => {
	if (!error.response) {
		return true;
	}
	return RETRYABLE_STATUSES.includes(error.response.status);
};

const sleep = (ms: number): Promise<void> =>
	new Promise((resolve) => setTimeout(resolve, ms));

const api = axios.create();

api.interceptors.response.use(
	(response) => {
		setApiState(ApiState.CONNECTED);
		return response;
	},
	async (error: AxiosError) => {
		const config = error.config as undefined | RetryConfig;
		if (!config) {
			return Promise.reject(error);
		}

		const retryCount = config.__retryCount ?? 0;

		if (!isRetryableError(error) || retryCount >= MAX_ATTEMPTS - 1) {
			if (retryCount > 0) {
				setApiState(ApiState.DISCONNECTED);
			}
			return Promise.reject(error);
		}

		config.__retryCount = retryCount + 1;
		setApiState(ApiState.RECONNECTING);

		const delay = Math.min(
			RETRY_INITIAL_DELAY_MS *
				RETRY_BACKOFF_MULTIPLIER ** (config.__retryCount - 1),
			RETRY_MAX_DELAY_MS,
		);
		await sleep(delay);

		return api(config);
	},
);

export const apiUrl = (pathname: string): string => `/api${pathname}`;

export const httpGet = async (
	pathname: string,
	token: undefined | null | string,
) => api(getOptions(pathname, token));
export const getOptions = (
	pathname: string,
	token: undefined | null | string,
) => {
	return {
		url: apiUrl(pathname),
		method: HttpMethod.GET,
		headers: headers(token),
	};
};

export const httpPost = async (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(postOptions(pathname, token, data));
export const postOptions = (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(pathname, HttpMethod.POST, token, data);
};

export const httpPut = async (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(putOptions(pathname, token, data));
export const putOptions = (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(pathname, HttpMethod.PUT, token, data);
};

export const httpPatch = async (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(pathOptions(pathname, token, data));
export const pathOptions = (
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(pathname, HttpMethod.PATCH, token, data);
};

const HEADERS_CONTENT_TYPE = {
	"Content-Type": "application/json",
};

const headers = (token: undefined | null | string) => {
	if (token) {
		return {
			...HEADERS_CONTENT_TYPE,
			Authorization: `Bearer ${token}`,
		};
	}
	return HEADERS_CONTENT_TYPE;
};

export const dataOptions = (
	pathname: string,
	method: HttpMethod,
	token: undefined | null | string,
	data: object,
) => {
	return {
		url: apiUrl(pathname),
		method: method,
		headers: headers(token),
		data: data,
	};
};

const API_MESSAGE_MAX_LENGTH = 256;

/** The first line of the API's reason for refusing a request, capped for a notification, or `fallback` when it sent none. */
export const apiErrorMessage = (error: unknown, fallback: string): string => {
	if (!axios.isAxiosError<{ message?: unknown }>(error)) {
		return fallback;
	}
	const message = error.response?.data?.message;
	if (typeof message !== "string") {
		return fallback;
	}
	// A server error adds a report link on a second line, which a notification runs together with the first.
	const firstLine = (message.split("\n")[0] ?? "").trim();
	return firstLine.length === 0
		? fallback
		: Array.from(firstLine).slice(0, API_MESSAGE_MAX_LENGTH).join("");
};
