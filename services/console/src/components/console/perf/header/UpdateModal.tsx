import * as Sentry from "@sentry/astro";
import {
	type Accessor,
	type Resource,
	createEffect,
	createMemo,
	createResource,
	createSignal,
} from "solid-js";
import { createStore } from "solid-js/store";
import { PLOT_FIELDS } from "../../../../config/project/plot";
import type {
	JsonAuthUser,
	JsonPlot,
	JsonProject,
	XAxis,
	YAxis,
} from "../../../../types/bencher";
import { httpGet, httpPatch } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";
import { init_valid } from "../../../../util/valid";
import Field, { type FieldHandler } from "../../../field/Field";
import FieldKind from "../../../field/kind";

// Same cap as the Plots page; a project can have at most 64 pinned plots.
const MAX_PLOTS = 64;

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Accessor<undefined | JsonProject>;
	isPlotInit: Accessor<boolean>;
	plot: Accessor<undefined | string>;
	plot_selected: Resource<JsonPlot[]>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
	y_axis: Accessor<YAxis>;
	branches: Accessor<string[]>;
	testbeds: Accessor<string[]>;
	benchmarks: Accessor<string[]>;
	measures: Accessor<string[]>;
	update: Accessor<boolean>;
	setUpdate: (update: boolean) => void;
	handleRefresh: () => void;
}

const UpdateModal = (props: Props) => {
	createEffect(() => {
		if (props.update()) {
			document.documentElement.classList.add("is-clipped");
		} else {
			document.documentElement.classList.remove("is-clipped");
		}
	});

	const [bencher_valid] = createResource(init_valid);

	const [form, setForm] = createStore(initForm());
	const [submitting, setSubmitting] = createSignal(false);
	const [valid, setValid] = createSignal(true);

	// The loaded pinned plot provides the current title and window.
	const plot = createMemo(() => props.plot_selected()?.[0]);

	// The full plots list (rank order) provides the current position and total.
	const plotsFetcher = createMemo(() => {
		return {
			project: props.project()?.uuid,
			plot: props.plot(),
			update: props.update(),
			token: props.user?.token,
		};
	});
	const [plots_list] = createResource<JsonPlot[]>(
		plotsFetcher,
		async (fetcher) => {
			if (!fetcher.project || !fetcher.plot) {
				return [];
			}
			return await httpGet(
				props.apiUrl,
				`/v0/projects/${fetcher.project}/plots?per_page=${MAX_PLOTS}`,
				fetcher.token,
			)
				.then((resp) => resp?.data as JsonPlot[])
				.catch((error) => {
					console.error(error);
					Sentry.captureException(error);
					return [];
				});
		},
	);
	const position = createMemo(() => {
		const index = (plots_list() ?? []).findIndex(
			(list_plot) => list_plot.uuid === props.plot(),
		);
		return index >= 0 ? index + 1 : null;
	});
	const total = createMemo(() => plots_list()?.length ?? 0);

	// Seed the form with the plot's current title, window, and position.
	// Reseed whenever fresh data arrives while the modal is closed, and seed
	// late if the modal was opened before the plot data first arrived.
	// Never reseed while the modal is open and seeded, to keep in-progress edits.
	const [seeded, setSeeded] = createSignal(false);
	createEffect(() => {
		if (props.update() && seeded()) {
			return;
		}
		const current = plot();
		const currentPosition = position();
		if (!current || currentPosition === null) {
			return;
		}
		setForm({
			title: {
				value: current.title ?? "",
				valid: true,
			},
			window: {
				value: current.window,
				valid: true,
			},
			index: {
				value: currentPosition,
				valid: true,
			},
		});
		setValid(true);
		setSeeded(true);
	});

	const isSendable = (): boolean =>
		!submitting() &&
		valid() &&
		// `isPlotInit` is `true` whenever any of the branches, testbeds,
		// benchmarks, or measures lists is empty. An empty component list is
		// rejected by the API, since the plot would never render anything.
		!props.isPlotInit() &&
		plot() !== undefined &&
		position() !== null;

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
		(form?.title?.valid !== false &&
			form?.window?.valid &&
			form?.index?.valid) ??
		false;

	const handleSubmit = () => {
		if (!bencher_valid()) {
			return;
		}
		const projectUuid = props.project()?.uuid;
		const plotUuid = props.plot();
		if (!isSendable() || !projectUuid || !plotUuid) {
			return;
		}

		setSubmitting(true);

		const title = form?.title?.value?.trim();
		const updatePlot = {
			// An empty title clears the current title.
			title: title ? title : null,
			index: Number.parseInt(form?.index?.value?.toString()) - 1,
			window: Number.parseInt(form?.window?.value?.toString()),
			lower_value: props.lower_value(),
			upper_value: props.upper_value(),
			lower_boundary: props.lower_boundary(),
			upper_boundary: props.upper_boundary(),
			x_axis: props.x_axis(),
			y_axis: props.y_axis(),
			branches: props.branches(),
			testbeds: props.testbeds(),
			benchmarks: props.benchmarks(),
			measures: props.measures(),
		};

		httpPatch(
			props.apiUrl,
			`/v0/projects/${projectUuid}/plots/${plotUuid}`,
			props.user?.token,
			updatePlot,
		)
			.then(() => {
				setSubmitting(false);
				props.setUpdate(false);
				props.handleRefresh();
				pageNotify(NotifyKind.OK, "Pinned plot updated!");
			})
			.catch((error) => {
				setSubmitting(false);
				console.error(error);
				Sentry.captureException(error);
				pageNotify(
					NotifyKind.ERROR,
					`Failed to update plot: ${error?.response?.data?.message}`,
				);
			});
	};

	return (
		<form
			class={`modal ${props.update() && "is-active"}`}
			onSubmit={(e) => {
				e.preventDefault();
				handleSubmit();
			}}
		>
			<div
				class="modal-background"
				onMouseDown={(e) => {
					e.preventDefault();
					props.setUpdate(false);
				}}
				onKeyDown={(e) => {
					e.preventDefault();
					props.setUpdate(false);
				}}
			/>
			<div class="modal-card">
				<header class="modal-card-head">
					<p class="modal-card-title">Update pinned plot</p>
					<button
						class="delete"
						type="button"
						aria-label="close"
						onMouseDown={(e) => {
							e.preventDefault();
							props.setUpdate(false);
						}}
					/>
				</header>
				<section class="modal-card-body">
					<p>
						Update this pinned plot to match your current branches, testbeds,
						benchmarks, measures, and display settings.
					</p>
					<br />
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
						config={PLOT_FIELDS.title}
						handleField={handleField}
					/>
					<hr />
					<Field
						kind={FieldKind.PLOT_WINDOW}
						fieldKey="window"
						label={
							<>
								Time Window <small>(seconds)</small>
							</>
						}
						value={form?.window?.value}
						valid={form?.window?.valid}
						config={{
							help: PLOT_FIELDS.window.help,
						}}
						handleField={handleField}
					/>
					<hr />
					<Field
						kind={FieldKind.PLOT_RANK}
						fieldKey="index"
						label="Move Plot"
						value={form?.index?.value}
						valid={form?.index?.valid}
						config={{
							bottom: "Move to bottom",
							top: "Move to top",
							total: total() > 0 ? total() : undefined,
							help: PLOT_FIELDS.index.help,
						}}
						handleField={handleField}
					/>
				</section>
				<footer class="modal-card-foot">
					<button
						class="button is-primary is-fullwidth"
						type="button"
						disabled={!isSendable()}
						onMouseDown={(e) => {
							e.preventDefault();
							handleSubmit();
						}}
					>
						Update
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
		window: {
			value: 2419200,
			valid: true,
		},
		index: {
			value: 1,
			valid: true,
		},
	};
};

export default UpdateModal;
