import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import TextInput from "@bencherdev/ui/TextInput";
import {
	For,
	Show,
	createComputed,
	createSignal,
	on,
	onCleanup,
} from "solid-js";
import { type ReportView, windowDays } from "./view";

const SEARCH_DELAY = 250;

type WindowChoice = "1w" | "4w" | "3m" | "custom";

const WINDOWS: readonly Option<WindowChoice>[] = [
	{ value: "1w", label: "1w", long: "1 week" },
	{ value: "4w", label: "4w", long: "4 weeks" },
	{ value: "3m", label: "3m", long: "3 months" },
	{ value: "custom", label: "Custom", long: "Custom" },
];
const GROUPS: readonly Option<ReportView["group"]>[] = [
	{ value: "benchmark", label: "Benchmark", long: "Benchmark" },
	{ value: "measure", label: "Measure", long: "Measure" },
];
const SORTS: readonly Option<ReportView["sort"]>[] = [
	{
		value: "name",
		label: "alerting first",
		long: "Alerting first, then by name",
	},
	{ value: "delta", label: "worst delta first", long: "Worst delta first" },
];

interface Option<T extends string> {
	value: T;
	label: string;
	long: string;
}

/** The window ending at the report, the grouping, the sort, and a filter over the lines. */
const Controls = (props: {
	view: ReportView;
	narrow: boolean;
	onView: (view: ReportView, replace?: boolean) => void;
}) => {
	const windowChoice = (): WindowChoice =>
		typeof props.view.window === "number" ? "custom" : props.view.window;
	const pickWindow = (choice: WindowChoice) =>
		props.onView({
			...props.view,
			window: choice === "custom" ? windowDays(props.view.window) : choice,
		});
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	// The box keeps what was typed; the URL holds it trimmed, and only a change made elsewhere, such as Back, comes back into the box.
	const [text, setText] = createSignal(props.view.search);
	createComputed(
		on(
			() => props.view.search,
			(search) => {
				if (search !== text().trim()) {
					clearTimeout(timer);
					setText(search);
				}
			},
			{ defer: true },
		),
	);
	const search = (typed: string) => {
		setText(typed);
		clearTimeout(timer);
		timer = setTimeout(
			() => props.onView({ ...props.view, search: typed.trim() }, true),
			SEARCH_DELAY,
		);
	};
	return (
		<div class={props.narrow ? "chips rp-controls" : "toolbar rp-controls"}>
			<Choice
				name="report-window"
				label="window"
				title="Window, ending at this report"
				options={WINDOWS}
				value={windowChoice()}
				narrow={props.narrow}
				onChange={pickWindow}
			/>
			<Show when={typeof props.view.window === "number"}>
				<TextInput
					type="number"
					min="1"
					max="366"
					size="sm"
					class="rp-days"
					aria-label="Window in days, ending at this report"
					value={windowDays(props.view.window)}
					onChange={(event) => {
						const days = Math.round(Number(event.currentTarget.value));
						if (days >= 1) {
							props.onView({ ...props.view, window: days });
						}
					}}
				/>
				<span class="muted sm">days</span>
			</Show>
			<Show when={!props.narrow}>
				<span class="muted sm">ending at this report</span>
			</Show>
			<Choice
				name="report-group"
				label="group"
				title="Group by"
				options={GROUPS}
				value={props.view.group}
				narrow={props.narrow}
				onChange={(group) => props.onView({ ...props.view, group })}
			/>
			<Choice
				name="report-sort"
				label="sort"
				title="Sort"
				options={SORTS}
				value={props.view.sort}
				narrow={props.narrow}
				onChange={(sort) => props.onView({ ...props.view, sort })}
			/>
			<TextInput
				type="search"
				class="rp-filter"
				placeholder="Filter lines"
				aria-label="Filter lines by benchmark, parameter, measure, or metric"
				value={text()}
				onInput={(event) => search(event.currentTarget.value)}
			/>
		</div>
	);
};

export default Controls;

/** One choice: joined positions when wide, a chip that opens a menu when narrow. */
const Choice = <T extends string>(props: {
	name: string;
	label: string;
	title: string;
	options: readonly Option<T>[];
	value: T;
	narrow: boolean;
	onChange: (value: T) => void;
}) => {
	const [open, setOpen] = createSignal(false);
	let chip: HTMLButtonElement | undefined;
	const current = () =>
		props.options.find((option) => option.value === props.value);
	return (
		<Show
			when={props.narrow}
			fallback={
				<>
					<span class="eyebrow">{props.title.split(",")[0]}</span>
					<Segmented
						name={props.name}
						aria-label={props.title}
						options={props.options}
						value={props.value}
						onChange={props.onChange}
					/>
				</>
			}
		>
			<span class="rp-anchor">
				<Chip
					ref={chip}
					label={props.label}
					caret
					aria-haspopup="menu"
					aria-expanded={open()}
					aria-label={`${props.title}, ${current()?.long ?? ""}`}
					onClick={() => setOpen(!open())}
				>
					{current()?.label}
				</Chip>
				<Show when={open()}>
					<Menu
						label={props.title}
						anchor={chip}
						onClose={() => setOpen(false)}
					>
						<For each={props.options}>
							{(option) => (
								<MenuItemRadio
									checked={option.value === props.value}
									onSelect={() => {
										setOpen(false);
										props.onChange(option.value);
									}}
								>
									{option.long}
								</MenuItemRadio>
							)}
						</For>
					</Menu>
				</Show>
			</span>
		</Show>
	);
};
