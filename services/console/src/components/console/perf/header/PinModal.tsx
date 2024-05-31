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
	XAxis,
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
import { plotFields } from "../../../../config/project/plot";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Accessor<undefined | JsonProject>;
	isPlotInit: Accessor<boolean>;
	perfQuery: Accessor<JsonPerfQuery>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	measures: Accessor<string[]>;
	pin: Accessor<boolean>;
	setPin: (share: boolean) => void;
}

const PinModal = (props: Props) => {
	const [bencher_valid] = createResource(
		async () => await bencher_valid_init(),
	);
	const navigate = useNavigate();

	const [form, setForm] = createStore(initForm());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(true);

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
		((form?.title?.valid ?? true) &&
			form?.window?.valid &&
			form?.index?.valid) ??
		false;

	const handleSubmit = () => {
		if (!bencher_valid()) {
			return;
		}
		// This should never be possible, but just double check.
		if (props.isPlotInit()) {
			return;
		}

		setSubmitting(true);

		const newPlot: JsonNewPlot = {
			title: form?.title?.value ? form?.title?.value?.trim() : undefined,
			index: Number.parseInt(form?.index?.value - 1),
			lower_value: props.lower_value(),
			upper_value: props.upper_value(),
			lower_boundary: props.lower_boundary(),
			upper_boundary: props.upper_boundary(),
			x_axis: props.x_axis(),
			window: Number.parseInt(form?.window?.value),
			branches: props.branches(),
			testbeds: props.testbeds(),
			benchmarks: props.benchmarks(),
			measures: props.measures(),
		};

		httpPost(
			props.apiUrl,
			`/v0/projects/${props.project()?.uuid}/plots`,
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
						fieldKey="title"
						label={
							<>
								Title <small>(optional)</small>
							</>
						}
						value={form?.title?.value}
						valid={form?.title?.valid}
						config={plotFields(props.project()).title}
						handleField={handleField}
					/>

					<Field
						kind={FieldKind.PLOT_RANK}
						fieldKey="index"
						label="Insert Location"
						value={form?.index?.value}
						valid={form?.index?.valid}
						config={{
							bottom: "Insert at bottom",
							top: "Insert at top",
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
		title: {
			value: "",
			valid: null,
		},
		index: {
			value: 1,
			valid: true,
		},
		window: {
			value: "2419200",
			valid: true,
		},
	};
};

export default PinModal;
