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
import { Button, Operation } from "../../config/types";
import type { JsonThreshold } from "../../types/bencher";
import { authUser } from "../../util/auth";
import type { DeckConfig } from "../console/deck/hand/Deck";
import Deck from "../console/deck/hand/Deck";
import DeckHeaderButton from "../console/deck/header/DeckHeaderButton";
import { httpGet } from "../../util/http";
import { BACK_PARAM, decodePath, useSearchParams } from "../../util/url";
import thresholdsConfig from "../../config/project/thresholds";
import * as Sentry from "@sentry/astro";

const MODEL_PARAM = "model";

const deck = thresholdsConfig[Operation.VIEW]?.deck as DeckConfig;

export interface Props {
	apiUrl: string;
	params: Params;
	data: undefined | JsonThreshold;
}

const PublicThreshold = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();

	createEffect(() => {
		const initParams: Record<string, null | string> = {};
		if (typeof searchParams[BACK_PARAM] !== "string") {
			initParams[BACK_PARAM] = null;
		}
		if (typeof searchParams[MODEL_PARAM] !== "string") {
			initParams[MODEL_PARAM] = null;
		}
		if (Object.keys(initParams).length !== 0) {
			setSearchParams(initParams, { replace: true });
		}
	});

	const back = createMemo(() => searchParams[BACK_PARAM]);
	const model = createMemo(() => searchParams[MODEL_PARAM]);

	const user = authUser();
	const path = createMemo(() => props.apiUrl);
	const title = createMemo(
		() =>
			`${props.data?.branch?.name} | ${props.data?.testbed?.name} | ${props.data?.measure?.name}`,
	);

	const fetcher = createMemo(() => {
		return {
			bencher_valid: bencher_valid(),
			model: model(),
		};
	});
	const getData = async (fetcher: {
		bencher_valid: InitOutput;
		model: undefined | null | string;
	}) => {
		if (props.data && !fetcher.model) {
			return props.data;
		}

		const searchParams = new URLSearchParams();
		if (fetcher.model) {
			searchParams.set(MODEL_PARAM, fetcher.model);
		}
		const path = `/v0/projects/${props.params?.project}/thresholds/${
			props.params?.threshold
		}?${searchParams.toString()}`;
		return await httpGet(props.apiUrl, path, null)
			.then((resp) => {
				return resp?.data as JsonThreshold;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return {};
			});
	};
	const [thresholdData, { refetch }] = createResource(fetcher, getData);
	const [_loopback, setLoopback] = createSignal(null);

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
								<For
									each={[
										{
											kind: Button.CONSOLE,
											resource: "thresholds",
											param: "threshold",
										},
									]}
								>
									{(button) => (
										<div class="level-item">
											<DeckHeaderButton
												isConsole={false}
												apiUrl={props.apiUrl}
												params={props.params}
												user={user}
												button={button}
												path={path}
												data={thresholdData}
												title={title}
												handleRefresh={refetch}
											/>
										</div>
									)}
								</For>
							</div>
						</nav>
					</div>
				</div>
				<div class="columns is-mobile">
					<div class="column">
						<Deck
							isConsole={false}
							apiUrl={props.apiUrl}
							params={props.params}
							user={user}
							config={deck}
							path={path}
							data={thresholdData}
							handleRefresh={refetch}
							handleLoopback={setLoopback}
						/>
					</div>
				</div>
			</div>
		</section>
	);
};

export default PublicThreshold;
