import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import {
	For,
	Show,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { authUser } from "../../util/auth";
import Deck from "../console/deck/hand/Deck";
import DeckHeaderButton from "../console/deck/header/DeckHeaderButton";
import { httpGet } from "../../util/http";
import { BACK_PARAM, decodePath, useSearchParams } from "../../util/url";
import * as Sentry from "@sentry/astro";
import { fmtDateTime } from "../../config/util";
import { Display, type Button } from "../../config/types";
import { fmtValues } from "../../util/resource";
import type CardConfig from "../console/deck/hand/card/CardConfig";
import type { PubResourceKind } from "./util";
import RawButton from "../console/deck/hand/RawButton";
import HeadReplacedButton from "../console/deck/hand/HeadReplacedButton";
import ModelReplacedButton from "../console/deck/hand/ModelReplacedButton";
import ArchivedButton from "../console/deck/hand/ArchivedButton";
import { set } from "astro/zod";

export interface Props {
	apiUrl: string;
	path: string;
	params: Params;
	config: PubDeckPanelConfig;
	data: undefined | object;
}

export interface PubDeckPanelConfig {
	resource: PubResourceKind;
	header: PubDeckHeaderConfig;
	deck: PubDeckConfig;
}

export interface PubDeckHeaderConfig {
	key: string;
	keys?: string[][];
	display?: Display;
	buttons: PublDeckButtons;
}

export type PublDeckButtons = PubDeckButton[];

export interface PubDeckButton {
	kind: Button;
	resource: string;
	param: string;
}

export interface PubDeckConfig {
	url: (params: Params, search?: Params) => string;
	cards: CardConfig[];
}

const PublicDeck = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();
	const [refresh, setRefresh] = createSignal(false);

	createEffect(() => {
		const initParams: Record<string, null | string> = {};
		if (typeof searchParams[BACK_PARAM] !== "string") {
			initParams[BACK_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams, { replace: true });
		}
	});

	const back = createMemo(() => searchParams[BACK_PARAM]);

	const user = authUser();
	const path = createMemo(() => props.apiUrl);

	const getData = async (_bencher_valid: InitOutput) => {
		if (props.data && !refresh()) {
			return props.data;
		}
		setRefresh(false);

		return await httpGet(props.apiUrl, props.path, null)
			.then((resp) => {
				return resp?.data;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return {};
			});
	};
	const [data, { refetch }] = createResource(bencher_valid, getData);
	const [_loopback, setLoopback] = createSignal(null);

	const title = createMemo(() => {
		switch (props.config?.header?.display) {
			case Display.DATE_TIME:
				return fmtDateTime(data()?.[props.config?.header?.key] ?? "");
			default:
				return fmtValues(
					data(),
					props.config?.header?.key,
					props.config?.header?.keys,
					" | ",
				);
		}
	});

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-centered">
					<Show when={back()}>
						<div class="column is-narrow">
							<a
								class="button is-fullwidth"
								title="Go back"
								href={decodePath(`/perf/${props.params?.project}`)}
							>
								<span class="icon">
									<i class="fas fa-chevron-left" />
								</span>
								<span>Back</span>
							</a>
						</div>
					</Show>

					<div class="column">
						<div class="content has-text-centered">
							<h3 class="title is-3" style="word-break: break-word;">
								{title()}
							</h3>
						</div>
					</div>

					<div class="column is-narrow">
						<nav class="level">
							<div class="level-right">
								<For each={props.config?.header?.buttons}>
									{(button) => (
										<div class="level-item">
											<DeckHeaderButton
												isConsole={false}
												apiUrl={props.apiUrl}
												params={props.params}
												user={user}
												button={button}
												path={path}
												data={data}
												title={title}
												handleRefresh={() => {
													setRefresh(true);
													refetch();
												}}
											/>
										</div>
									)}
								</For>
							</div>
						</nav>
					</div>
				</div>

				<ArchivedButton resource={props.config?.resource} data={data} />
				<HeadReplacedButton data={data} />
				<ModelReplacedButton data={data} />

				<div class="columns is-mobile">
					<div class="column">
						<Deck
							isConsole={false}
							apiUrl={props.apiUrl}
							params={props.params}
							user={user}
							config={props.config?.deck}
							path={path}
							data={data}
							handleRefresh={() => {
								setRefresh(true);
								refetch();
							}}
							handleLoopback={setLoopback}
						/>
					</div>
				</div>

				<div class="columns">
					<div class="column">
						<form
							onSubmit={(e) => {
								e.preventDefault();
							}}
						>
							<div class="field">
								<p class="control">
									<RawButton data={data} />
								</p>
							</div>
						</form>
					</div>
				</div>
			</div>
		</section>
	);
};

export default PublicDeck;
