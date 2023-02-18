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

	for (const [key, value] of current_params.entries()) {
		if (keep_params?.includes(key)) {
			params.set(key, value);
		}
	}
	set_params?.forEach((param) => {
		params.set(param[0], param[1]);
	});

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
	return navigate(
		forward_path(props.href, props.keep_params, props.set_params),
	);
};

export default Forward;
