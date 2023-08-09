// import axios from "axios";
// import {
// 	createSignal,
// 	createResource,
// 	createMemo,
// 	createEffect,
// } from "solid-js";

import bencher_valid_init, { InitOutput } from "bencher_valid";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import consoleConfig from "../../../config/console";
import { Operation, Resource } from "../../../config/types";
import { authUser } from "../../../util/auth";
import { validJwt } from "../../../util/valid";
import { httpGet } from "../../../util/http";
import { pathname, useParams } from "../../../util/url";
import DeckHeader, { DeckHeaderConfig } from "./header/DeckHeader";
import Deck, { DeckConfig } from "./hand/Deck";

// import DeckHeader from "./DeckHeader";
// import Deck from "./Deck";
// import { get_options, validate_jwt } from "../../../site/util";
// import { useLocation } from "solid-app-router";

interface Props {
	path: string;
	resource: Resource;
}

interface DeckPanelConfig {
	operation: Operation;
	header: DeckHeaderConfig;
	deck: DeckConfig;
}

const DeckPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const pathParams = useParams(props.path);
	const user = authUser();
	const config = createMemo<DeckPanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.VIEW],
	);

	// const location = useLocation();
	// const pathname = createMemo(() => location.pathname);

	const url = createMemo(() => config()?.deck?.url(pathParams));

	// const [refresh, setRefresh] = createSignal(0);
	// const handleRefresh = () => {
	// 	setRefresh(refresh() + 1);
	// };

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			token: authUser()?.token,
		};
	});

	const getData = async (fetcher: {
		bencher_valid: InitOutput;
		token: string;
	}) => {
		const EMPTY_OBJECT = {};
		if (!fetcher.bencher_valid) {
			return EMPTY_OBJECT;
		}
		if (!validJwt(fetcher.token)) {
			return EMPTY_OBJECT;
		}
		return await httpGet(url(), fetcher.token)
			.then((resp) => resp?.data)
			.catch((error) => {
				console.error(error);
				return EMPTY_OBJECT;
			});
	};

	const [deckData, { refetch }] = createResource(fetcher, getData);

	// Redirect to the updated path before refreshing the page
	const [loopback, setLoopback] = createSignal(null);
	createEffect(() => {
		const path = loopback();
		if (path && path === pathname()) {
			setLoopback(null);
			refetch();
		}
	});

	return (
		<>
			<DeckHeader
				pathParams={pathParams}
				user={user}
				config={config()?.header}
				url={url}
				data={deckData}
				handleRefresh={refetch}
			/>
			<Deck
				pathParams={pathParams}
				user={user}
				config={config()?.deck}
				url={url}
				data={deckData}
				// refresh={refresh}
				handleRefresh={refetch}
				handleLoopback={setLoopback}
			/>
		</>
	);
};

export default DeckPanel;
