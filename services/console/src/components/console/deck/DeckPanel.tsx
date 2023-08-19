import bencher_valid_init, { InitOutput } from "bencher_valid";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import consoleConfig from "../../../config/console";
import { Operation, Resource, resourceSingular } from "../../../config/types";
import { authUser } from "../../../util/auth";
import { validJwt } from "../../../util/valid";
import { httpGet } from "../../../util/http";
import { pathname } from "../../../util/url";
import DeckHeader, { DeckHeaderConfig } from "./header/DeckHeader";
import Deck, { DeckConfig } from "./hand/Deck";
import type { Params } from "astro";
import { NotifyKind, pageNotify } from "../../../util/notify";

interface Props {
	params: Params;
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
	const user = authUser();
	const config = createMemo<DeckPanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.VIEW],
	);
	const url = createMemo(() => config()?.deck?.url(props.params));

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
				pageNotify(
					NotifyKind.ERROR,
					`Lettuce romaine calm! Failed to get ${resourceSingular(
						props.resource,
					)}. Please, try again.`,
				);
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
				params={props.params}
				user={user}
				config={config()?.header}
				url={url}
				data={deckData}
				handleRefresh={refetch}
			/>
			<Deck
				params={props.params}
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
