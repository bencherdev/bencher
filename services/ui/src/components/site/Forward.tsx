import { useNavigate } from "solid-app-router";

const Forward = (props: {
	href: string;
	search_params: null | string[][];
}) => {
	const navigate = useNavigate();
	let params = new URLSearchParams(window.location.search);
	props.search_params?.forEach((search_param) => {
		params.set(search_param[0], search_param[1]);
	});
	let params_str = params.toString();
	// console.log(`${props.href}?${params_str}`);
	navigate(`${props.href}?${params_str}`);
};

export default Forward;
