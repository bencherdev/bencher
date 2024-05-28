import { debounce } from "@solid-primitives/scheduled";
import bencher_valid_init from "bencher_valid";
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
	JsonNewPlot,
	JsonPerfQuery,
	JsonProject,
} from "../../../../types/bencher";
import { apiUrl, httpPost } from "../../../../util/http";
import Field, { type FieldHandler } from "../../../field/Field";
import FieldKind from "../../../field/kind";
import { DEBOUNCE_DELAY, validResourceName } from "../../../../util/valid";
import { useNavigate, useSearchParams } from "../../../../util/url";
import {
	EMBED_TITLE_PARAM,
	PERF_PLOT_EMBED_PARAMS,
	PERF_PLOT_PARAMS,
	PERF_PLOT_PIN_PARAMS,
} from "../PerfPanel";
import { createStore } from "solid-js/store";
import { NotifyKind, pageNotify } from "../../../../util/notify";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Accessor<undefined | JsonProject>;
	perfQuery: Accessor<JsonPerfQuery>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	isPlotInit: Accessor<boolean>;
	pin: Accessor<boolean>;
	setPin: (share: boolean) => void;
}

const PinModal = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);

	const navigate = useNavigate();
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

	const [form, setForm] = createStore(initForm());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(false);

	const isSendable = (): boolean => {
		return !submitting() && valid();
	};

	const handleField: FieldHandler = (key, value, valid) => {
		setForm({
			...form,
			[key]: {
				value,
				valid,
			},
		});

		setValid(validateForm());
	};

	const validateForm = () =>
		(form?.name?.valid && form?.window?.valid && form?.rank?.valid) ?? false;

	const handleSubmit = () => {
		console.log("submitting");
		if (!bencher_valid()) {
			return;
		}
		setSubmitting(true);

		const newPlot: JsonNewPlot = {
			name: form?.name?.value?.trim(),
			rank: Number.parseInt(form?.rank?.value),
			window: Number.parseInt(form?.window?.value),
		};

		httpPost(
			props.apiUrl,
			`/v0/projects/${props.project()?.uuid}/plot`,
			props.user?.token,
			newPlot,
		)
			.then((resp) => {
				setSubmitting(false);
				navigate(
					`/console/projects/${props.project()?.slug}/dashboard#${
						resp?.data?.uuid
					}`,
				);
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				pageNotify(
					NotifyKind.ERROR,
					"Failed to pin plot to dashboard. Please, try again.",
				);
			});
	};

	return (
		<form
			class={`modal ${props.pin() && "is-active"}`}
			onSubmit={(e) => {
				e.preventDefault();
				handleSubmit();
			}}
		>
			<div
				class="modal-background"
				onClick={(e) => {
					e.preventDefault();
					props.setPin(false);
				}}
				onKeyDown={(e) => {
					e.preventDefault();
					props.setPin(false);
				}}
			/>
			<div class="modal-card">
				<header class="modal-card-head">
					<p class="modal-card-title">
						Pin to {props.project()?.name} Dashboard
					</p>
					<button
						class="delete"
						type="button"
						aria-label="close"
						onClick={(e) => {
							e.preventDefault();
							props.setPin(false);
						}}
					/>
				</header>
				<section class="modal-card-body">
					<Field
						kind={FieldKind.INPUT}
						fieldKey="name"
						label="Plot Name"
						value={form?.name?.value}
						valid={form?.name?.valid}
						config={{
							type: "text",
							placeholder: props.project()?.name,
							icon: "fas fa-chart-line",
							validate: validResourceName,
						}}
						handleField={handleField}
					/>
				</section>

				<footer class="modal-card-foot">
					<button
						class="button is-primary is-fullwidth"
						type="button"
						disabled={!isSendable()}
						onClick={(e) => {
							e.preventDefault();
							handleSubmit();
						}}
					>
						Pin
					</button>
				</footer>
			</div>
		</form>
	);
};

const initForm = () => {
	return {
		name: {
			value: "",
			valid: null,
		},
		rank: {
			value: 0,
			valid: true,
		},
		window: {
			value: 2_419_200,
			valid: true,
		},
	};
};

export default PinModal;
