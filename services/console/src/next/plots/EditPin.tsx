import { Show, createEffect, createSignal } from "solid-js";
import type { JsonConsolePlotTitle, JsonPlot } from "../../types/bencher";

type PinDialog = typeof import("./PinDialog").default;

let dialog: PinDialog | undefined;
let loading: Promise<PinDialog> | undefined;
const load = () => {
	loading ??= import("./PinDialog").then(
		({ default: view }) => {
			dialog = view;
			return view;
		},
		(error: unknown) => {
			loading = undefined;
			throw error;
		},
	);
	return loading;
};

/** The dialog's code loads outside any Suspense, so the page behind it stays put. */
const EditPin = (props: {
	plot: JsonPlot | undefined;
	order?: readonly JsonConsolePlotTitle[] | undefined;
	rename?: boolean;
	onClose: () => void;
	onSaved: (plot: JsonPlot, order: string[]) => void;
}) => {
	const [View, setView] = createSignal(dialog);
	createEffect(() => {
		if (props.plot && !View()) {
			load()
				.then((view) => setView(() => view))
				.catch(() => props.onClose());
		}
	});
	return (
		<Show when={View()}>
			{(view) => {
				const Loaded = view();
				return (
					<Show when={props.plot} keyed>
						{(plot) => (
							<Loaded
								plot={plot}
								{...(props.order ? { order: props.order } : {})}
								rename={props.rename ?? false}
								onClose={props.onClose}
								onSaved={props.onSaved}
							/>
						)}
					</Show>
				);
			}}
		</Show>
	);
};

export default EditPin;
