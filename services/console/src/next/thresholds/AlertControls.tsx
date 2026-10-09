import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import { For, Show, createSignal } from "solid-js";
import { useNarrow } from "../plot/narrow";
import { CustomRange } from "../reports/Controls";
import {
	type Choice,
	type ThresholdView,
	WINDOW_CHOICES,
	pickWindow,
	windowLabel,
} from "./search";

const STATUSES = [
	{ value: "active", label: "Active" },
	{ value: "dismissed", label: "Dismissed" },
	{ value: "all", label: "All" },
] as const;

/** The window and the status of a threshold's alerts: two segmented controls when wide, a full-width status and a chip when narrow. */
const AlertControls = (props: {
	view: ThresholdView;
	now: number;
	onView: (view: ThresholdView) => void;
}) => {
	const narrow = useNarrow();
	const [menu, setMenu] = createSignal(false);
	let chip: HTMLButtonElement | undefined;
	const choice = () => windowLabel(props.view.window);
	const pick = (value: Choice) => {
		const window = pickWindow(value, props.view.window, props.now);
		if (window) {
			props.onView({ ...props.view, window });
		}
	};
	const long = () =>
		WINDOW_CHOICES.find(({ value }) => value === choice())?.long ?? "";
	return (
		<>
			<div class="toolbar th-controls">
				<span class="eyebrow th-wide">Window</span>
				<Segmented
					name="th-alerts-window"
					class="th-wide"
					aria-label="Window"
					options={WINDOW_CHOICES}
					value={choice()}
					onChange={pick}
				/>
				<Segmented
					name="th-alerts-status"
					full={narrow()}
					aria-label="Status"
					options={STATUSES}
					value={props.view.status}
					onChange={(status) => props.onView({ ...props.view, status })}
				/>
				<span class="reports-anchor th-narrow">
					<Chip
						ref={chip}
						label="window"
						caret
						aria-haspopup="menu"
						aria-expanded={menu()}
						aria-label={`Window, ${long()}`}
						onClick={() => setMenu(!menu())}
					>
						{WINDOW_CHOICES.find(({ value }) => value === choice())?.label}
					</Chip>
					<Show when={menu()}>
						<Menu label="Window" anchor={chip} onClose={() => setMenu(false)}>
							<For each={WINDOW_CHOICES}>
								{({ value, long }) => (
									<MenuItemRadio
										checked={choice() === value}
										onSelect={() => {
											setMenu(false);
											pick(value);
										}}
									>
										{long}
									</MenuItemRadio>
								)}
							</For>
						</Menu>
					</Show>
				</span>
			</div>
			<Show when={props.view.window.kind === "custom" && props.view.window}>
				{(custom) => (
					<CustomRange
						window={custom()}
						onWindow={(window) => {
							if (window.kind === "custom") {
								props.onView({ ...props.view, window });
							}
						}}
					/>
				)}
			</Show>
		</>
	);
};

export default AlertControls;
