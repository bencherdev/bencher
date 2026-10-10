import Segmented from "@bencherdev/ui/Segmented";
import TextInput from "@bencherdev/ui/TextInput";
import { Match, Show, Switch, createUniqueId } from "solid-js";
import type { ExploreQuery, QueryWindow } from "../query/query";
import { LINE_CAP, lineCapWarning, measuresLayout } from "./layout";
import {
	PRESETS,
	type WindowChoice,
	customRange,
	dateValue,
	pickWindow,
	windowChoice,
} from "./window";

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
}) => {
	const layout = () =>
		measuresLayout(props.query.measures.length, props.query.layout);
	const cap = () =>
		props.total === undefined ? undefined : lineCapWarning(props.total);
	const set = (change: Partial<ExploreQuery>) =>
		props.onQuery({ ...props.query, ...change });
	return (
		<>
			<div class="ex-ctl">
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
				<span class="eyebrow" aria-hidden="true">
					Window
				</span>
				<Segmented
					name="explore-window"
					aria-label="Window"
					options={WINDOWS}
					value={windowChoice(props.query.window)}
					onChange={(choice) =>
						set({ window: pickWindow(props.query.window, choice, props.now()) })
					}
				/>
				<Show when={"start" in props.query.window && props.query.window}>
					{(window) => (
						<CustomRange
							window={window()}
							now={props.now()}
							onWindow={(next) => set({ window: next })}
						/>
					)}
				</Show>
				<Switch>
					<Match when={layout().control === "choice"}>
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
					</Match>
					<Match when={layout().control === "forced"}>
						<span class="muted sm">three or more measures always stack</span>
					</Match>
				</Switch>
			</div>
			<Show when={cap()}>
				{(warning) => (
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
						<span>
							<b>
								{warning() === "over"
									? `${props.total} lines, over the cap of ${LINE_CAP}.`
									: `${props.total} of ${LINE_CAP} lines.`}
							</b>{" "}
							{warning() === "over"
								? `The plot draws the first ${LINE_CAP}. Remove a value or a set to narrow the query.`
								: "The query is near the line cap; narrow it before adding more."}
						</span>
					</p>
				)}
			</Show>
		</>
	);
};

export default Controls;

const CustomRange = (props: {
	window: QueryWindow & { start: number };
	now: number;
	onWindow: (window: QueryWindow) => void;
}) => {
	const from = createUniqueId();
	const to = createUniqueId();
	const end = () => props.window.end ?? props.now;
	const change = (first: string, last: string) => {
		const window = customRange(first, last);
		if (window) {
			props.onWindow(window);
		}
	};
	return (
		<fieldset class="ex-range">
			<legend class="sr-only">Custom window</legend>
			<label class="eyebrow" for={from}>
				From
			</label>
			<TextInput
				id={from}
				type="date"
				size="sm"
				value={dateValue(props.window.start)}
				max={dateValue(end())}
				onChange={(event) =>
					change(event.currentTarget.value, dateValue(end()))
				}
			/>
			<label class="eyebrow" for={to}>
				To
			</label>
			<TextInput
				id={to}
				type="date"
				size="sm"
				value={dateValue(end())}
				min={dateValue(props.window.start)}
				onChange={(event) =>
					change(dateValue(props.window.start), event.currentTarget.value)
				}
			/>
		</fieldset>
	);
};
