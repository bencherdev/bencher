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
import type { JsonAlert } from "../../types/bencher";
import { authUser } from "../../util/auth";
import type { DeckConfig } from "../console/deck/hand/Deck";
import Deck from "../console/deck/hand/Deck";
import DeckHeaderButton from "../console/deck/header/DeckHeaderButton";
import { httpGet } from "../../util/http";
import alertsConfig from "../../config/project/alerts";
import { BACK_PARAM, decodePath, useSearchParams } from "../../util/url";
import * as Sentry from "@sentry/astro";

const deck = alertsConfig[Operation.VIEW]?.deck as DeckConfig;

export interface Props {
	apiUrl: string;
	params: Params;
	data: undefined | JsonAlert;
}

const PublicAlert = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const [searchParams, setSearchParams] = useSearchParams();

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
	const title = createMemo(() => props.data?.benchmark?.name);

	const getData = async (_bencher_valid: InitOutput) => {
		if (props.data) {
			return props.data;
		}

		const path = `/v0/projects/${props.params?.project}/alerts/${props.params?.alert}`;
		return await httpGet(props.apiUrl, path, null)
			.then((resp) => {
				return resp?.data as JsonAlert;
			})
			.catch((error) => {
				console.error(error);
				Sentry.captureException(error);
				return {};
			});
	};
	const [alertData, { refetch }] = createResource(bencher_valid, getData);
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
											resource: "alerts",
											param: "alert",
										},
										{ kind: Button.PERF },
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
												data={alertData}
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
							data={alertData}
							handleRefresh={refetch}
							handleLoopback={setLoopback}
						/>
					</div>
				</div>
			</div>
		</section>
	);
};

export default PublicAlert;
