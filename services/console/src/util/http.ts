import axios from "axios";

enum HttpMethod {
	GET = "GET",
	POST = "POST",
	PUT = "PUT",
	PATCH = "PATCH",
	DELETE = "DELETE",
}

const apiHost = (hostname: string): string => {
	if (hostname) {
		return hostname;
	} else {
		const location = window.location;
		return `${location.protocol}//${location.hostname}:61016`;
	}
};

const apiUrl = (hostname: string, pathname: string): string =>
	`${apiHost(hostname)}${pathname}`;

export const httpGet = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => axios(getOptions(hostname, pathname, token));
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
	data: {},
) => axios(postOptions(hostname, pathname, token, data));
export const postOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: {},
) => {
	return dataOptions(hostname, pathname, HttpMethod.POST, token, data);
};

export const httpPut = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: {},
) => axios(putOptions(hostname, pathname, token, data));
export const putOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: {},
) => {
	return dataOptions(hostname, pathname, HttpMethod.PUT, token, data);
};

export const httpPatch = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: {},
) => axios(pathOptions(hostname, pathname, token, data));
export const pathOptions = (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
	data: Record<string, any>,
) => {
	return dataOptions(hostname, pathname, HttpMethod.PATCH, token, data);
};

export const httpDelete = async (
	hostname: string,
	pathname: string,
	token: undefined | null | string,
) => axios(deleteOptions(hostname, pathname, token));
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
	} else {
		return HEADERS_CONTENT_TYPE;
	}
};

export const dataOptions = (
	hostname: string,
	pathname: string,
	method: HttpMethod,
	token: undefined | null | string,
	data: {},
) => {
	return {
		url: apiUrl(hostname, pathname),
		method: method,
		headers: headers(token),
		data: data,
	};
};
