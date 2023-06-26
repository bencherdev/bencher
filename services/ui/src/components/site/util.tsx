import axios from "axios";
import {
	is_valid_jwt,
	is_valid_plan_level,
	is_valid_card_number,
	is_valid_expiration_month,
	is_valid_expiration_year,
	is_valid_card_cvc,
	is_valid_boundary,
} from "bencher_valid";
import { Analytics } from "analytics";
import googleAnalytics from "@analytics/google-analytics";

import swagger from "../docs/api/swagger.json";

export const PLAN_PARAM = "plan";

// Either supply `VITE_BENCHER_API_URL` at build time,
// or default to the current protocol and hostname at port `61016`.
// If another endpoint is required, then the UI will need to be re-bundled.
export const BENCHER_API_URL: () => string = () => {
	const api_url = import.meta.env.VITE_BENCHER_API_URL;
	if (api_url) {
		return api_url;
	} else {
		const location = window.location;
		return `${location.protocol}//${location.hostname}:61016`;
	}
};

export const BENCHER_GITHUB_URL: string =
	"https://github.com/bencherdev/bencher";

export const BENCHER_CALENDLY_URL: string = "https://calendly.com/bencher/demo";

export const BENCHER_LOGO_URL: string =
	"https://s3.amazonaws.com/public.bencher.dev/bencher_navbar.png";

export const BENCHER_USER_KEY: string = "BENCHER_USER";

export const BENCHER_TITLE = "Bencher - Continuous Benchmarking";

export const BENCHER_VERSION = `${swagger?.info?.version}`;

export const site_analytics = () => {
	let plugins = [];

	const google_analytics_id = import.meta.env.VITE_GOOGLE_ANALYTICS_ID;
	if (google_analytics_id) {
		plugins.push(
			googleAnalytics({
				measurementIds: [google_analytics_id],
			}),
		);
	}

	return Analytics({
		app: "bencher.dev",
		plugins: plugins,
	});
};

export const pageTitle = (new_title: string) => {
	if (new_title && new_title.length > 0) {
		const page_title = `${new_title} - Bencher`;
		if (document.title === page_title) {
			return;
		} else {
			document.title = page_title;
		}
	} else {
		document.title = BENCHER_TITLE;
	}

	site_analytics()?.page();
};

export const validate_string = (
	input: string,
	validator: (input: string) => boolean,
): boolean => {
	if (typeof input === "string") {
		return validator(input.trim());
	} else {
		return false;
	}
};

export const validate_number = (
	input: string,
	validator: (input: number) => boolean,
): boolean => {
	if (input.length === 0) {
		return false;
	} else if (typeof input === "string") {
		const num = Number(input.trim());
		return validator(num);
	} else {
		return false;
	}
};

export const validate_jwt = (token: string): boolean => {
	return validate_string(token, is_valid_jwt);
};

export const validate_plan_level = (plan_level: string): boolean => {
	return validate_string(plan_level, is_valid_plan_level);
};

export const validate_card_number = (card_number: string): boolean => {
	return validate_string(card_number, (card_number) => {
		const number = clean_card_number(card_number);
		return is_valid_card_number(number);
	});
};

export const clean_card_number = (card_number: string): string => {
	return card_number
		.replace(new RegExp(" ", "g"), "")
		.replace(new RegExp("-", "g"), "");
};

export const validate_expiration = (expiration: string): boolean => {
	return validate_string(expiration, (expiration) => {
		if (!/^\d{1,2}\/\d{2,4}$/.test(expiration)) {
			return false;
		}

		if (clean_expiration(expiration) === null) {
			return false;
		} else {
			return true;
		}
	});
};

export const clean_expiration = (expiration: string): null | number[] => {
	const exp = expiration.split("/");
	if (exp.length !== 2) {
		return null;
	}

	const exp_month = Number.parseInt(exp?.[0]);
	if (Number.isInteger(exp_month)) {
		if (!is_valid_expiration_month(exp_month)) {
			return null;
		}
	} else {
		return null;
	}

	let exp_year = Number.parseInt(exp?.[1]);
	if (Number.isInteger(exp_year)) {
		if (exp_year < 100) {
			exp_year = 2000 + exp_year;
		}
		if (!is_valid_expiration_year(exp_year)) {
			return null;
		}
	} else {
		return null;
	}

	return [exp_month, exp_year];
};

export const validate_card_cvc = (card_cvc: string): boolean => {
	return validate_string(card_cvc, is_valid_card_cvc);
};

// TODO improve this validation
export const validate_user = (user: {}) => {
	return validate_jwt(user?.token);
};

export const validate_boundary = (boundary: string): boolean => {
	return validate_number(boundary, is_valid_boundary);
};

export const validate_u32 = (input: string) => {
	if (input.length === 0) {
		return false;
	}
	const num = Number(input);
	return Number.isInteger(num) && num >= 0 && num <= 4_294_967_295;
};

enum HttpMethod {
	GET = "GET",
	POST = "POST",
	PUT = "PUT",
	PATCH = "PATCH",
	DELETE = "DELETE",
}

const HEADERS_CONTENT_TYPE = {
	"Content-Type": "application/json",
};

const get_headers = (token: null | string) => {
	if (validate_jwt(token)) {
		return {
			...HEADERS_CONTENT_TYPE,
			Authorization: `Bearer ${token}`,
		};
	} else {
		return HEADERS_CONTENT_TYPE;
	}
};

export const get_options = (url: string, token: null | string) => {
	return {
		url: url,
		method: HttpMethod.GET,
		headers: get_headers(token),
	};
};

export const data_options = (
	url: string,
	method: HttpMethod,
	token: null | string,
	data: {},
) => {
	return {
		url: url,
		method: method,
		headers: get_headers(token),
		data: data,
	};
};

export const post_options = (url: string, token: null | string, data: {}) => {
	return data_options(url, HttpMethod.POST, token, data);
};

export const put_options = (
	url: string,
	token: null | string,
	data: { any },
) => {
	return data_options(url, HttpMethod.PUT, token, data);
};

export const patch_options = (
	url: string,
	token: null | string,
	data: { any },
) => {
	return data_options(url, HttpMethod.PATCH, token, data);
};

export const getToken = () =>
	JSON.parse(window.localStorage.getItem(BENCHER_USER_KEY))?.token;

// export const isAllowedAdmin = async () => {
//   return is_allowed(`${BENCHER_API_URL()}/v0/admin/allowed`);
// };

export enum OrganizationPermission {
	VIEW = "view",
	CREATE = "create",
	EDIT = "edit",
	DELETE = "delete",
	MANAGE = "manage",
	VIEW_ROLE = "view_role",
	CREATE_ROLE = "create_role",
	EDIT_ROLE = "edit_role",
	DELETE_ROLE = "delete_role",
}

export const is_allowed_organization = async (
	path_params,
	permission: OrganizationPermission,
) => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/organizations/${
			path_params?.organization_slug
		}/allowed/${permission}`,
	);
};

export enum ProjectPermission {
	VIEW = "view",
	CREATE = "create",
	EDIT = "edit",
	DELETE = "delete",
	MANAGE = "manage",
	VIEW_ROLE = "view_role",
	CREATE_ROLE = "create_role",
	EDIT_ROLE = "edit_role",
	DELETE_ROLE = "delete_role",
}

export const isAllowedProject = async (
	path_params,
	permission: ProjectPermission,
) => {
	return is_allowed(
		`${BENCHER_API_URL()}/v0/projects/${
			path_params?.project_slug
		}/allowed/${permission}`,
	);
};

export const is_allowed = async (url: string) => {
	const token = getToken();
	if (!validate_jwt(token)) {
		return false;
	}
	return await axios(get_options(url, token))
		.then((resp) => resp?.data?.allowed)
		.catch((error) => {
			console.error(error);
			return false;
		});
};

export const NOTIFY_KIND_PARAM = "notify_kind";
export const NOTIFY_TEXT_PARAM = "notify_text";

export const isNotifyKind = (kind: any) => {
	switch (parseInt(kind)) {
		case NotifyKind.OK:
		case NotifyKind.ALERT:
		case NotifyKind.ERROR:
			return true;
		default:
			return false;
	}
};

export const isNotifyText = (text: any) =>
	typeof text === "string" && text.length > 0;

export enum NotifyKind {
	OK,
	ALERT,
	ERROR,
}

export const notifyParams = (
	pathname,
	notify_kind: NotifyKind,
	notify_text: string,
	search_params: null | string[][],
) => {
	let params = new URLSearchParams(window.location.search);
	params.set(NOTIFY_KIND_PARAM, notify_kind.toString());
	params.set(NOTIFY_TEXT_PARAM, notify_text);
	if (search_params) {
		search_params.forEach((search_param) => {
			params.set(search_param[0], search_param[1]);
		});
	}
	let params_str = params.toString();
	// console.log(`${pathname}?${params_str}`);
	return `${pathname}?${params_str}`;
};

export const usd_formatter = new Intl.NumberFormat("en-US", {
	style: "currency",
	currency: "USD",
});

export const concat_values = (data, key, keys, separator) => {
	if (!data) {
		return;
	} else if (key) {
		return data[key];
	} else if (keys) {
		return keys.reduce((accumulator, current, index) => {
			const value = nested_value(data, current);
			if (index === 0) {
				return value;
			} else {
				return accumulator + separator + value;
			}
		}, "");
	} else {
		return "Unknown Item";
	}
};

export const nested_value = (datum, keys) => {
	if (!datum) {
		return;
	}
	return keys.reduce((accumulator, current, index) => {
		if (index === 0) {
			return datum[current];
		} else {
			return accumulator[current];
		}
	}, "");
};
