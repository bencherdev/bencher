import type { Params } from "astro";
import bencher_valid_init, { type InitOutput } from "bencher_valid";
import { For, createMemo, createResource, createSignal } from "solid-js";
import { Button } from "../../config/types";
import type { JsonAlert } from "../../types/bencher";
import { authUser } from "../../util/auth";
import type { DeckConfig } from "../console/deck/hand/Deck";
import Deck from "../console/deck/hand/Deck";
import DeckHeaderButton from "../console/deck/header/DeckHeaderButton";

export interface Props {
	apiUrl: string;
	params: Params;
	deck: DeckConfig;
	data: JsonAlert;
}

const PublicAlert = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const user = authUser();
	const path = createMemo(() => props.apiUrl);
	const title = createMemo(() => props.data?.benchmark?.name);

	const getData = async (_bencher_valid: InitOutput) => {
		return props.data;
	};
	const [alertData, { refetch }] = createResource(bencher_valid, getData);
	const [_loopback, setLoopback] = createSignal(null);

	return (
		<section class="section">
			<div class="container">
				<div class="columns is-centered">
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
								<For each={[{ kind: Button.CONSOLE }, { kind: Button.PERF }]}>
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
							apiUrl={props.apiUrl}
							params={props.params}
							user={user}
							config={props.deck}
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
