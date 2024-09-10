import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import {
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import consoleConfig from "../../../config/console";
import {
	Operation,
	type BencherResource,
	resourceSingular,
} from "../../../config/types";
import { authUser } from "../../../util/auth";
import { httpGet } from "../../../util/http";
import { NotifyKind, pageNotify } from "../../../util/notify";
import { pathname, useSearchParams } from "../../../util/url";
import { validJwt } from "../../../util/valid";
import Deck, { type DeckConfig } from "./hand/Deck";
import DeckHeader, { type DeckHeaderConfig } from "./header/DeckHeader";
import * as Sentry from "@sentry/astro";

interface Props {
	apiUrl: string;
	params: Params;
	resource: BencherResource;
}
export interface DeckPanelConfig {
	operation: Operation;
	header: DeckHeaderConfig;
	deck: DeckConfig;
}

const DeckPanel = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, _setSearchParams] = useSearchParams();
	const user = authUser();
	const config = createMemo<DeckPanelConfig>(
		() => consoleConfig[props.resource]?.[Operation.VIEW],
	);
	const path = createMemo(() =>
		config()?.deck?.url(props.params, searchParams),
	);

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
		if (!fetcher.bencher_valid || !validJwt(fetcher.token)) {
			return EMPTY_OBJECT;
		}
		return await httpGet(props.apiUrl, path(), fetcher.token)
			.then((resp) => {
				// console.log(resp?.data);
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
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
				apiUrl={props.apiUrl}
				params={props.params}
				user={user}
				config={config()?.header}
				path={path}
				data={deckData}
				handleRefresh={refetch}
			/>
			<section class="section">
				<div class="container">
					<div class="columns is-mobile">
						<div class="column">
							<Deck
								isConsole={true}
								apiUrl={props.apiUrl}
								params={props.params}
								user={user}
								config={config()?.deck}
								path={path}
								data={deckData}
								handleRefresh={refetch}
								handleLoopback={setLoopback}
							/>
						</div>
					</div>
				</div>
			</section>
		</>
	);
};

export default DeckPanel;
