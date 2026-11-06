import type { AxiosError, InternalAxiosRequestConfig } from "axios";
import axios from "axios";
import { deleteCache } from "../next/idb";
import { forgetReader } from "../next/memory";
import { signInHref } from "../next/reader";
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
		if (endsSession(error)) {
			signInAgain();
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

/** A 401 from anything but signing in, which answers one for a sign in that ended. */
const endsSession = (error: AxiosError) =>
	error.response?.status === 401 &&
	!error.config?.url?.startsWith(apiUrl("/v0/auth/"));

/** The session ended: forget the reader and what they saw, and sign in again, back here. */
const signInAgain = () => {
	forgetReader(localStorage);
	deleteCache();
	location.replace(
		signInHref(`${location.pathname}${location.search}${location.hash}`),
	);
};

export const apiUrl = (pathname: string): string => `/api${pathname}`;

export const httpGet = async (pathname: string) => api(getOptions(pathname));
export const getOptions = (pathname: string) => {
	return {
		url: apiUrl(pathname),
		method: HttpMethod.GET,
		headers: HEADERS_CONTENT_TYPE,
	};
};

export const httpPost = async (pathname: string, data: object) =>
	api(postOptions(pathname, data));
export const postOptions = (pathname: string, data: object) => {
	return dataOptions(pathname, HttpMethod.POST, data);
};

export const httpPut = async (pathname: string, data: object) =>
	api(putOptions(pathname, data));
export const putOptions = (pathname: string, data: object) => {
	return dataOptions(pathname, HttpMethod.PUT, data);
};

export const httpPatch = async (pathname: string, data: object) =>
	api(pathOptions(pathname, data));
export const pathOptions = (pathname: string, data: object) => {
	return dataOptions(pathname, HttpMethod.PATCH, data);
};

const HEADERS_CONTENT_TYPE = {
	"Content-Type": "application/json",
};

export const dataOptions = (
	pathname: string,
	method: HttpMethod,
	data: object,
) => {
	return {
		url: apiUrl(pathname),
		method: method,
		headers: HEADERS_CONTENT_TYPE,
		data: data,
	};
};
