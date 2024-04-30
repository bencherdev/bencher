import { hiddenRedirect, pathname, useNavigate, useSearchParams } from "./url";

export const NOTIFY_KIND_PARAM = "notify_kind";
export const NOTIFY_TEXT_PARAM = "notify_text";
export const NOTIFY_TIMEOUT_PARAM = "notify_timeout";
export const NOTIFY_LINK_URL_PARAM = "notify_link_url";
export const NOTIFY_LINK_TEXT_PARAM = "notify_link_text";

export const NOTIFY_TIMEOUT = 4321;
export const MAX_NOTIFY_TIMEOUT = 2147483647;

export enum NotifyKind {
	OK = "ok",
	ALERT = "alert",
	ERROR = "error",
}

export const isNotifyKind = (kind: undefined | string) => {
	switch (kind) {
		case NotifyKind.OK:
		case NotifyKind.ALERT:
		case NotifyKind.ERROR:
			return true;
		default:
			return false;
	}
};

export const isNotifyText = (text: undefined | string) =>
	typeof text === "string" && text.length > 0;

export const isNotifyTimeout = (timeout: undefined | string) =>
	typeof timeout === "string" &&
	timeout.length > 0 &&
	Number.isInteger(parseInt(timeout));

export const forwardParams = (
	pathname: string,
	keepParams: null | string[],
	setParams: null | [string, string][],
) => {
	if (
		(keepParams === null && setParams === null) ||
		(keepParams?.length === 0 && setParams?.length === 0)
	) {
		return pathname;
	}

	let searchParams = new URLSearchParams();
	let currentParams = new URLSearchParams(window.location.search);

	if (Array.isArray(keepParams)) {
		for (const [key, value] of currentParams.entries()) {
			// console.log(`FOUND ${key} ${value}`);
			if (keepParams?.includes(key)) {
				// console.log(`KEEP ${key} ${value}`);
				searchParams.set(key, value);
			}
		}
	}

	// console.log(`SET PARAMS ${set_params}`);
	if (Array.isArray(setParams)) {
		for (const [key, value] of setParams) {
			// console.log(`SET ${key} ${value}`);
			searchParams.set(key, value);
		}
	}

	let params_str = searchParams.toString();
	// console.log(`${href}?${params_str}`);
	if (params_str.length === 0) {
		return pathname;
	} else {
		// console.log(params_str);
		return `${pathname}?${params_str}`;
	}
};

export const notifyPath = (
	notifyKind: NotifyKind,
	notifyText: string,
	pathname: string,
	keepParams: null | string[],
	setParams: null | [string, string][],
) => {
	if (setParams === null) {
		setParams = [];
	}
	setParams = [
		[NOTIFY_KIND_PARAM, notifyKind.toString()],
		[NOTIFY_TEXT_PARAM, notifyText],
		...setParams,
	];
	return forwardParams(pathname, keepParams, setParams);
};

export const navigateNotify = (
	notifyKind: NotifyKind,
	notifyText: string,
	to: null | string,
	keepParams: null | string[],
	setParams: null | [string, string][],
	hidden?: boolean,
) => {
	if (to === null) {
		to = pathname();
	}
	const path = notifyPath(notifyKind, notifyText, to, keepParams, setParams);
	if (hidden) {
		hiddenRedirect(path);
	} else {
		const navigate = useNavigate();
		navigate(path);
	}
};

export const pageNotify = (
	notifyKind: NotifyKind,
	notifyText: string,
	notifyOptions?: { [NOTIFY_TIMEOUT_PARAM]: undefined | number },
) => {
	const [_searchParams, setSearchParams] = useSearchParams();
	setSearchParams(
		{
			[NOTIFY_KIND_PARAM]: notifyKind,
			[NOTIFY_TEXT_PARAM]: notifyText,
			...(notifyOptions ?? {}),
		},
		{ replace: true },
	);
	window.scrollTo({
		top: 0,
		behavior: "smooth",
	});
};
