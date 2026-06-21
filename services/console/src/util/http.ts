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

// Due to how Astro works, this hostname, that is the `apiUrl`
// must be explicitly passed down from each page.
// You can't get `BENCHER_API_URL` anywhere other than the page frontmatter.
export const apiHost = (hostname: string): string => {
	if (hostname) {
		return hostname;
	}
	const location = window.location;
	return `${location.protocol}//${location.hostname}:6610`;
};

export const apiUrl = (hostname: string, pathname: string): string =>
	`${apiHost(hostname)}${pathname}`;

export const httpGet = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => api(getOptions(hostname, pathname, token));
export const getOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => {
	return {
		url: apiUrl(hostname, pathname),
		method: HttpMethod.GET,
		headers: headers(token),
	};
};

export const httpPost = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(postOptions(hostname, pathname, token, data));
export const postOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(hostname, pathname, HttpMethod.POST, token, data);
};

export const httpPut = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(putOptions(hostname, pathname, token, data));
export const putOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(hostname, pathname, HttpMethod.PUT, token, data);
};

export const httpPatch = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => api(pathOptions(hostname, pathname, token, data));
export const pathOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: object,
) => {
	return dataOptions(hostname, pathname, HttpMethod.PATCH, token, data);
};

export const httpDelete = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => api(deleteOptions(hostname, pathname, token));
export const deleteOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => {
	return {
		url: apiUrl(hostname, pathname),
		method: HttpMethod.DELETE,
		headers: headers(token),
	};
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
	hostname: string,
	pathname: string,
	method: HttpMethod,
	token: undefined | null | string,
	data: object,
) => {
	return {
		url: apiUrl(hostname, pathname),
		method: method,
		headers: headers(token),
		data: data,
	};
};
