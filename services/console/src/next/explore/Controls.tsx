import Segmented from "@bencherdev/ui/Segmented";
import { type JSX, Show } from "solid-js";
import type { ExploreQuery } from "../query/query";
import CustomRange from "./CustomRange";
import { LINE_CAP, lineCapWarning, measuresLayout } from "./layout";
import { PRESETS, type WindowChoice, pickWindow, windowChoice } from "./window";
import { formatCount } from "../plot/count";

const X_AXES = [
	{ value: "date", label: "Report date" },
	{ value: "version", label: "Version" },
] as const;
const Y_SCALES = [
	{ value: "auto", label: "Auto" },
	{ value: "linear", label: "Linear" },
	{ value: "log", label: "Log" },
] as const;
const WINDOWS: { value: WindowChoice; label: string }[] = [
	...PRESETS.map(({ value }) => ({ value, label: value })),
	{ value: "custom", label: "Custom" },
];
const LAYOUTS = [
	{ value: "dual", label: "Dual axis" },
	{ value: "stacked", label: "Stacked" },
] as const;

/** The x axis, the y scale, the window, and the layout once two or more measures are chosen. */
const Controls = (props: {
	query: ExploreQuery;
	/** The lines the query names, from the plot's last answer. */
	total: number | undefined;
	now: () => number;
	onQuery: (query: ExploreQuery) => void;
	/** For a reader who has no custom window: the presets end now. */
	presetsOnly?: boolean;
	/** What ends the row. */
	children?: JSX.Element;
}) => {
	const layout = () =>
		measuresLayout(props.query.measures.length, props.query.layout);
	const cap = () =>
		props.total === undefined ? undefined : lineCapWarning(props.total);
	const set = (change: Partial<ExploreQuery>) =>
		props.onQuery({ ...props.query, ...change });
	const pick = (choice: WindowChoice) => {
		const { end: _end, ...unended } = props.query.window;
		set({
			window: pickWindow(
				props.presetsOnly ? unended : props.query.window,
				choice,
				props.now(),
			),
		});
	};
	return (
		<>
			<div class="ex-ctl">
				<span class="ex-pair">
					<span class="eyebrow" aria-hidden="true">
						X axis
					</span>
					<Segmented
						name="explore-x"
						aria-label="X axis"
						options={X_AXES}
						value={props.query.xAxis}
						onChange={(xAxis) => set({ xAxis })}
					/>
				</span>
				<span class="ex-pair">
					<span class="eyebrow" aria-hidden="true">
						Y scale
					</span>
					<Segmented
						name="explore-y"
						aria-label="Y scale"
						options={Y_SCALES}
						value={props.query.yScale}
						onChange={(yScale) => set({ yScale })}
					/>
				</span>
				<span class="ex-pair">
					<span class="eyebrow" aria-hidden="true">
						Window
					</span>
					<Segmented
						name="explore-window"
						aria-label="Window"
						options={props.presetsOnly ? WINDOWS.slice(0, -1) : WINDOWS}
						value={
							props.presetsOnly && props.query.window.end !== undefined
								? undefined
								: windowChoice(props.query.window)
						}
						onChange={pick}
					/>
				</span>
				<Show when={"start" in props.query.window && props.query.window}>
					{(window) => (
						<CustomRange
							window={window()}
							now={props.now()}
							onWindow={(next) => set({ window: next })}
						/>
					)}
				</Show>
				<Show when={layout().control === "choice"}>
					<span class="ex-pair">
						<span class="eyebrow" aria-hidden="true">
							Measures
						</span>
						<Segmented
							name="explore-layout"
							aria-label="Measures layout"
							options={LAYOUTS}
							value={layout().layout}
							onChange={(next) => set({ layout: next })}
						/>
					</span>
				</Show>
				{props.children}
			</div>
			<Show when={cap()}>
				<p class="ex-cap" role="status">
					<svg
						viewBox="0 0 24 24"
						width="16"
						height="16"
						fill="none"
						stroke="currentColor"
						stroke-width="2"
						stroke-linecap="round"
						stroke-linejoin="round"
						aria-hidden="true"
					>
						<path d="M12 4l9 16H3z" />
						<path d="M12 10v4M12 17v.5" />
					</svg>
					<b>
						{formatCount(props.total)} of {LINE_CAP} lines
					</b>
				</p>
			</Show>
		</>
	);
};

export default Controls;
