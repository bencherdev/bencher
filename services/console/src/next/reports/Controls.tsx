import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import Sheet from "@bencherdev/ui/Sheet";
import TextInput from "@bencherdev/ui/TextInput";
import { useQueryClient } from "@tanstack/solid-query";
import {
	For,
	type JSX,
	Show,
	createSignal,
	createUniqueId,
	onCleanup,
} from "solid-js";
import type { JsonBranch, JsonTestbed } from "../../types/bencher";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { nameKey } from "./query";
import {
	ADAPTERS,
	type FilterNames,
	type ReportsSearch,
	type ReportsWindow,
	WINDOWS,
	customWindow,
	dateValue,
	filterCount,
	windowPhrase,
	withFilter,
} from "./search";

const DAY = 24 * 60 * 60 * 1_000;
/** How many branches or testbeds a filter menu lists at once; search finds the rest. */
const OPTIONS = 32;
const SEARCH_DELAY = 150;

type WindowChoice = "1w" | "4w" | "3m" | "custom" | "all";

const choiceOf = (window: ReportsWindow): WindowChoice | undefined => {
	if (window.kind !== "rolling") {
		return window.kind;
	}
	return WINDOWS.find((preset) => preset.days === window.days)?.label;
};

type CustomWindow = Extract<ReportsWindow, { kind: "custom" }>;

const customOf = (window: ReportsWindow) =>
	window.kind === "custom" ? window : undefined;

const CHOICES: { value: WindowChoice; label: string; long: string }[] = [
	...WINDOWS.map(({ label, long }) => ({ value: label, label, long })),
	{ value: "custom", label: "Custom", long: "Custom range" },
	{ value: "all", label: "All", long: "All time" },
];

interface ControlsProps {
	search: ReportsSearch;
	names: FilterNames;
	onSearch: (search: ReportsSearch) => void;
	now: number;
}

/** The window and the filters: a row of controls when wide, two chips and a sheet when narrow. */
const Controls = (props: ControlsProps) => {
	const pick = (choice: WindowChoice) => {
		const preset = WINDOWS.find(({ label }) => label === choice);
		const window: ReportsWindow | undefined = preset
			? { kind: "rolling", days: preset.days }
			: choice === "all"
				? { kind: "all" }
				: props.search.window.kind === "custom"
					? props.search.window
					: customWindow(dateValue(props.now - 27 * DAY), dateValue(props.now));
		if (window) {
			props.onSearch({ ...props.search, window });
		}
	};
	const [windowMenu, setWindowMenu] = createSignal(false);
	const [sheet, setSheet] = createSignal(false);
	let windowChip: HTMLButtonElement | undefined;
	const count = () => filterCount(props.search);
	const windowName = () =>
		CHOICES.find(
			({ value }) =>
				value === choiceOf(props.search.window) && value !== "custom",
		)?.long ??
		windowPhrase(props.search.window, props.now).replace(/^In the last /, "");

	return (
		<>
			<div class="toolbar reports-wide">
				<span class="eyebrow">Window</span>
				<Segmented
					name="reports-window"
					aria-label="Window"
					options={CHOICES}
					value={choiceOf(props.search.window)}
					onChange={pick}
				/>
				<span class="eyebrow">Filters</span>
				<Filters
					search={props.search}
					names={props.names}
					onSearch={props.onSearch}
				/>
			</div>
			<div class="chips reports-narrow">
				<span class="reports-anchor">
					<Chip
						ref={windowChip}
						label="window"
						caret
						aria-haspopup="menu"
						aria-expanded={windowMenu()}
						aria-label={`Window, ${windowName()}`}
						onClick={() => setWindowMenu(!windowMenu())}
					>
						{CHOICES.find(
							({ value }) => value === choiceOf(props.search.window),
						)?.label ?? windowName()}
					</Chip>
					<Show when={windowMenu()}>
						<Menu
							label="Window"
							anchor={windowChip}
							onClose={() => setWindowMenu(false)}
						>
							<For each={CHOICES}>
								{({ value, long }) => (
									<MenuItemRadio
										checked={choiceOf(props.search.window) === value}
										onSelect={() => {
											setWindowMenu(false);
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
				<Chip
					caret
					on={count() > 0}
					aria-haspopup="dialog"
					aria-label={
						count() > 0
							? `Filters, ${count()} applied`
							: "Filters, none applied"
					}
					onClick={() => setSheet(true)}
				>
					{count() > 0 ? `Filters (${count()})` : "Filters"}
				</Chip>
				<Sheet open={sheet()} onClose={() => setSheet(false)} title="Filters">
					<div class="reports-sheet-filters">
						<Filters
							search={props.search}
							names={props.names}
							onSearch={props.onSearch}
						/>
					</div>
				</Sheet>
			</div>
			<Show when={customOf(props.search.window)}>
				{(custom) => (
					<CustomRange
						window={custom()}
						onWindow={(window) => props.onSearch({ ...props.search, window })}
					/>
				)}
			</Show>
		</>
	);
};

export default Controls;

export const CustomRange = (props: {
	window: CustomWindow;
	onWindow: (window: ReportsWindow) => void;
}) => {
	const change = (from: string, to: string) => {
		const window = customWindow(from, to);
		if (window) {
			props.onWindow(window);
		}
	};
	const from = createUniqueId();
	const to = createUniqueId();
	return (
		<fieldset class="reports-range">
			<legend class="sr-only">Custom range</legend>
			<div class="reports-date">
				<label class="eyebrow" for={from}>
					From
				</label>
				<TextInput
					id={from}
					type="date"
					size="sm"
					value={dateValue(props.window.start)}
					max={dateValue(props.window.end)}
					onChange={(event) =>
						change(event.currentTarget.value, dateValue(props.window.end))
					}
				/>
			</div>
			<div class="reports-date">
				<label class="eyebrow" for={to}>
					To
				</label>
				<TextInput
					id={to}
					type="date"
					size="sm"
					value={dateValue(props.window.end)}
					min={dateValue(props.window.start)}
					onChange={(event) =>
						change(dateValue(props.window.start), event.currentTarget.value)
					}
				/>
			</div>
		</fieldset>
	);
};

const Filters = (props: {
	search: ReportsSearch;
	names: FilterNames;
	onSearch: (search: ReportsSearch) => void;
}) => {
	const { slug } = useProject();
	const client = useQueryClient();
	/** The picked option's name, so the chip names it without asking again. */
	const named = (
		resource: "branches" | "testbeds",
		value: string | undefined,
		label: string | undefined,
	) => {
		if (value !== undefined && label !== undefined) {
			client.setQueryData(nameKey(resource, slug(), value), label);
		}
		return value;
	};
	return (
		<>
			<FilterMenu
				name="branch"
				value={props.search.branch}
				label={props.names.branch}
				search
				options={(text) => useNamed<JsonBranch>("branches", text)}
				onPick={(branch, label) =>
					props.onSearch(
						withFilter(
							props.search,
							"branch",
							named("branches", branch, label),
						),
					)
				}
			/>
			<FilterMenu
				name="testbed"
				value={props.search.testbed}
				label={props.names.testbed}
				search
				options={(text) => useNamed<JsonTestbed>("testbeds", text)}
				onPick={(testbed, label) =>
					props.onSearch(
						withFilter(
							props.search,
							"testbed",
							named("testbeds", testbed, label),
						),
					)
				}
			/>
			<FilterMenu
				name="adapter"
				value={props.search.adapter}
				options={() => () =>
					ADAPTERS.map((adapter) => ({ value: adapter, label: adapter }))
				}
				onPick={(adapter) =>
					props.onSearch(
						withFilter(
							props.search,
							"adapter",
							ADAPTERS.find((listed) => listed === adapter),
						),
					)
				}
			/>
			<FilterMenu
				name="alerts"
				value={props.search.alerts ? "active" : undefined}
				anyLabel="Any"
				options={() => () => [{ value: "active", label: "With active alerts" }]}
				onPick={(alerts) =>
					props.onSearch({ ...props.search, alerts: alerts === "active" })
				}
			/>
		</>
	);
};

export interface Option {
	value: string;
	label: string;
}

/** A project's branches or testbeds whose name or slug matches `text`, by slug. */
const useNamed = <T extends { slug: string; name: string }>(
	resource: "branches" | "testbeds",
	text: () => string,
) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => ({
		queryKey: ["console", resource, slug(), text()],
		// The last options stay while the next search loads, so the list never blinks empty.
		placeholderData: (previous: T[] | undefined) => previous,
		queryFn: async ({ signal }: { signal: AbortSignal }) => {
			const params = new URLSearchParams({ per_page: String(OPTIONS) });
			if (text()) {
				params.set("search", text());
			}
			return (
				await api.get<T[]>(
					`/v0/projects/${encodeURIComponent(slug())}/${resource}?${params}`,
					signal,
				)
			).data;
		},
	}));
	return () =>
		(result().data ?? []).map(({ slug, name }) => ({
			value: slug,
			label: name,
		}));
};

export const FilterMenu = (props: {
	name: "branch" | "testbed" | "measure" | "adapter" | "alerts";
	value: string | undefined;
	/** What the chip shows for `value`, when that is not `value` itself. */
	label?: string | undefined;
	/** Offer a search box, for lists too long to show whole. */
	search?: boolean;
	anyLabel?: string;
	/** Called once the menu opens, so its options load only then. */
	options: (text: () => string) => () => Option[];
	onPick: (value: string | undefined, label?: string) => void;
}) => {
	const [open, setOpen] = createSignal(false);
	let chip: HTMLButtonElement | undefined;
	const shown = () => props.label ?? props.value ?? "any";
	return (
		<span class="reports-anchor">
			<Chip
				ref={chip}
				label={props.name}
				caret
				on={props.value !== undefined}
				aria-haspopup="menu"
				aria-expanded={open()}
				aria-label={`Filter by ${props.name}, ${shown()}`}
				onClick={() => setOpen(!open())}
			>
				{shown()}
			</Chip>
			<Show when={open()}>
				<FilterOptions
					{...props}
					anchor={chip}
					onClose={() => setOpen(false)}
				/>
			</Show>
		</span>
	);
};

const FilterOptions = (props: {
	name: string;
	value: string | undefined;
	search?: boolean;
	anyLabel?: string;
	options: (text: () => string) => () => Option[];
	onPick: (value: string | undefined, label?: string) => void;
	anchor: HTMLElement | undefined;
	onClose: () => void;
}) => {
	const [typed, setTyped] = createSignal("");
	const [text, setText] = createSignal("");
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	const options = props.options(text);
	const pick = (value: string | undefined, label?: string) => {
		props.onClose();
		props.onPick(value, label);
	};
	const header: JSX.Element = props.search ? (
		<TextInput
			type="search"
			size="sm"
			class="reports-search"
			aria-label={`Search ${props.name}${props.name === "branch" ? "es" : "s"}`}
			placeholder="Search"
			value={typed()}
			onInput={(event) => {
				const value = event.currentTarget.value;
				setTyped(value);
				clearTimeout(timer);
				timer = setTimeout(() => setText(value.trim()), SEARCH_DELAY);
			}}
		/>
	) : undefined;
	return (
		<Menu
			label={`Filter by ${props.name}`}
			anchor={props.anchor}
			onClose={props.onClose}
			header={header}
		>
			<MenuItemRadio
				checked={props.value === undefined}
				onSelect={() => pick(undefined)}
			>
				{props.anyLabel ?? `Any ${props.name}`}
			</MenuItemRadio>
			<For each={options()}>
				{(option) => (
					<MenuItemRadio
						checked={option.value === props.value}
						onSelect={() => pick(option.value, option.label)}
					>
						{option.label}
					</MenuItemRadio>
				)}
			</For>
		</Menu>
	);
};
