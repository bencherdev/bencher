import { debounce } from "@solid-primitives/scheduled";
import {
	type Accessor,
	Show,
	createMemo,
	createSignal,
	createResource,
} from "solid-js";
import { embedHeight } from "../../../../config/types";
import type {
	JsonAuthUser,
	JsonPerfQuery,
	JsonProject,
} from "../../../../types/bencher";
import { apiUrl } from "../../../../util/http";
import Field from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { DEBOUNCE_DELAY } from "../../../../util/valid";
import { useSearchParams } from "../../../../util/url";
import {
	EMBED_TITLE_PARAM,
	PERF_PLOT_EMBED_PARAMS,
	PERF_PLOT_PARAMS,
	PERF_PLOT_PIN_PARAMS,
} from "../PerfPanel";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	perfQuery: Accessor<JsonPerfQuery>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	isPlotInit: Accessor<boolean>;
	project: Accessor<undefined | JsonProject>;
	share: Accessor<boolean>;
	setShare: (share: boolean) => void;
}

const PinModal = (props: Props) => {
	const location = window.location;
	const [searchParams, _setSearchParams] = useSearchParams();

	const [title, setTitle] = createSignal(null);

	const handle_title = debounce(
		(_key, value, _valid) => setTitle(value),
		DEBOUNCE_DELAY,
	);

	const perfPlotPinParams = createMemo(() => {
		const newParams = new URLSearchParams();
		for (const [key, value] of Object.entries(searchParams)) {
			if (value && PERF_PLOT_PIN_PARAMS.includes(key)) {
				newParams.set(key, value);
			}
		}
		return newParams;
	});

	const pinFetcher = createMemo(() => {
		return {
			perfQuery: props.perfQuery(),
			lower_value: props.lower_value(),
			upper_value: props.upper_value(),
			lower_boundary: props.lower_boundary(),
			upper_boundary: props.upper_boundary(),
			token: props.user?.token,
		};
	});
	const pinned = createResource(pinFetcher, (fetcher) => {});

	return (
		<div class={`modal ${props.share() && "is-active"}`}>
			<div
				class="modal-background"
				onClick={(e) => {
					e.preventDefault();
					props.setShare(false);
				}}
				onKeyDown={(e) => {
					e.preventDefault();
					props.setShare(false);
				}}
			/>
			<div class="modal-card">
				<header class="modal-card-head">
					<p class="modal-card-title">
						Pin to {props.project()?.name} dashboard
					</p>
					<button
						class="delete"
						type="button"
						aria-label="close"
						onClick={(e) => {
							e.preventDefault();
							props.setShare(false);
						}}
					/>
				</header>
				<section class="modal-card-body">
					<Field
						kind={FieldKind.INPUT}
						fieldKey="title"
						label="Title"
						value={title()}
						valid={true}
						config={{
							type: "text",
							placeholder: props.project()?.name,
							icon: "fas fa-chart-line",
							validate: (_input: string) => true,
						}}
						handleField={handle_title}
					/>
				</section>

				<footer class="modal-card-foot">
					<button
						class="button is-primary is-fullwidth"
						type="button"
						onClick={(e) => {
							e.preventDefault();
							props.setShare(false);
						}}
					>
						Close
					</button>
				</footer>
			</div>
		</div>
	);
};

export default PinModal;
