import { useNavigate } from "solid-app-router";

export const forward_path = (
	href: string,
	keep_params: null | string[],
	set_params: null | string[][],
) => {
	if (keep_params?.length === 0 && set_params?.length === 0) {
		return href;
	}

	let params = new URLSearchParams();
	let current_params = new URLSearchParams(window.location.search);

	if (Array.isArray(keep_params)) {
		for (const [key, value] of current_params.entries()) {
			console.log(`FOUND ${key} ${value}`);
			if (keep_params?.includes(key)) {
				console.log(`KEEP ${key} ${value}`);
				params.set(key, value);
			}
		}
	}

	console.log(`SET PARAMS ${set_params}`);
	if (Array.isArray(set_params)) {
		for (const [key, value] of set_params) {
			console.log(`SET ${key} ${value}`);
			params.set(key, value);
		}
	}

	let params_str = params.toString();
	console.log(`${href}?${params_str}`);

	if (params_str.length === 0) {
		return href;
	} else {
		return `${href}?${params_str}`;
	}
};

const Forward = (props: {
	href: string;
	keep_params: null | string[];
	set_params: null | string[][];
}) => {
	const navigate = useNavigate();
	navigate(forward_path(props.href, props.keep_params, props.set_params));
	return <></>;
};

export default Forward;
