import axios from "axios";
import {
	createSignal,
	createResource,
	createMemo,
	createEffect,
} from "solid-js";

import DeckHeader from "./DeckHeader";
import Deck from "./Deck";
import { get_options, validate_jwt } from "../../../site/util";
import { useLocation } from "solid-app-router";

const DeckPanel = (props) => {
	const location = useLocation();
	const pathname = createMemo(() => location.pathname);

	const url = createMemo(() => props.config?.deck?.url(props.path_params));

	const [refresh, setRefresh] = createSignal(0);
	const handleRefresh = () => {
		setRefresh(refresh() + 1);
	};

	// Redirect to the updated path before refreshing the page
	const [loopback, setLoopback] = createSignal(null);
	createEffect(() => {
		const path = loopback();
		if (path && path === pathname()) {
			setLoopback(null);
			handleRefresh();
		}
	});

	const getOne = async () => {
		const EMPTY_OBJECT = {};
		const token = props.user?.token;
		if (!validate_jwt(token)) {
			return EMPTY_OBJECT;
		}
		return await axios(get_options(url(), token))
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_OBJECT;
			});
	};

	const [deck_data] = createResource(refresh, getOne);

	return (
		<>
			<DeckHeader
				user={props.user}
				config={props.config?.header}
				data={deck_data}
				url={url}
				path_params={props.path_params}
				handleRefresh={handleRefresh}
			/>
			<Deck
				user={props.user}
				config={props.config?.deck}
				data={deck_data}
				url={url}
				path_params={props.path_params}
				refresh={refresh}
				handleRefresh={handleRefresh}
				handleLoopback={setLoopback}
			/>
		</>
	);
};

export default DeckPanel;
