import * as Sentry from "@sentry/astro";
import { type Accessor, createEffect, createSignal } from "solid-js";
import type {
	JsonAuthUser,
	JsonProject,
	XAxis,
} from "../../../../types/bencher";
import { httpPatch } from "../../../../util/http";
import { NotifyKind, pageNotify } from "../../../../util/notify";

export interface Props {
	apiUrl: string;
	user: JsonAuthUser;
	project: Accessor<undefined | JsonProject>;
	isPlotInit: Accessor<boolean>;
	plot: Accessor<undefined | string>;
	lower_value: Accessor<boolean>;
	upper_value: Accessor<boolean>;
	lower_boundary: Accessor<boolean>;
	upper_boundary: Accessor<boolean>;
	x_axis: Accessor<XAxis>;
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

	const [submitting, setSubmitting] = createSignal(false);

	const handleSubmit = () => {
		const projectUuid = props.project()?.uuid;
		const plotUuid = props.plot();
		// An empty component list would leave a pinned plot that renders nothing.
		if (props.isPlotInit() || !projectUuid || !plotUuid) {
			return;
		}

		setSubmitting(true);

		// Title, window, and position are intentionally omitted so they stay
		// unchanged. Those are edited from the pinned plot's settings.
		const updatePlot = {
			lower_value: props.lower_value(),
			upper_value: props.upper_value(),
			lower_boundary: props.lower_boundary(),
			upper_boundary: props.upper_boundary(),
			x_axis: props.x_axis(),
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
		<div class={`modal ${props.update() && "is-active"}`}>
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
					<p>
						The plot's title, window, and position are left unchanged. Edit
						those from the pinned plot's settings.
					</p>
				</section>
				<footer class="modal-card-foot">
					<button
						class="button is-primary is-fullwidth"
						type="button"
						disabled={submitting() || props.isPlotInit()}
						onMouseDown={(e) => {
							e.preventDefault();
							handleSubmit();
						}}
					>
						Update
					</button>
				</footer>
			</div>
		</div>
	);
};

export default UpdateModal;
