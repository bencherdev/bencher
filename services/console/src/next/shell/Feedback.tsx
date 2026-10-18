import Icon from "@bencherdev/ui/Icon";
import { Show, createSignal } from "solid-js";
import type { FeedbackAbout } from "../feedback/FeedbackDialog";

type View = typeof import("../feedback/FeedbackDialog").default;

let loaded: View | undefined;
let loading: Promise<View> | undefined;
const load = () => {
	loading ??= import("../feedback/FeedbackDialog").then(
		({ default: view }) => {
			loaded = view;
			return view;
		},
		(error: unknown) => {
			loading = undefined;
			throw error;
		},
	);
	return loading;
};
const preload = () => {
	load().catch(() => {});
};

/** The bar's Feedback control; the dialog's code loads on first intent. */
const Feedback = (props: { about: () => FeedbackAbout }) => {
	const [view, setView] = createSignal(loaded);
	const [open, setOpen] = createSignal(false);
	// A build with no Sentry DSN, as a self-hosted install is, has nowhere to send feedback.
	return (
		<Show when={import.meta.env.PUBLIC_SENTRY_DSN}>
			<button
				type="button"
				class="fbbtn"
				aria-haspopup="dialog"
				aria-expanded={open()}
				aria-label="Feedback"
				onPointerEnter={preload}
				onFocus={preload}
				onClick={() =>
					load()
						.then((view) => {
							setView(() => view);
							setOpen(true);
						})
						.catch(() => {})
				}
			>
				<Icon name="feedback" />
				<span class="fb-label">Feedback</span>
			</button>
			<Show when={open() && view()}>
				{(view) => {
					const Dialog = view();
					return (
						<Dialog about={props.about()} onClose={() => setOpen(false)} />
					);
				}}
			</Show>
		</Show>
	);
};

export default Feedback;
