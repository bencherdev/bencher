import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import Sheet from "@bencherdev/ui/Sheet";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createSignal } from "solid-js";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { CustomRange, FilterMenu } from "../reports/Controls";
import {
	type ReportsWindow,
	WINDOWS,
	customWindow,
	dateValue,
	windowPhrase,
} from "../reports/search";
import { type Dimension, nameKey } from "./query";
import {
	type AlertsSearch,
	type AlertsStatus,
	type Filter,
	filterCount,
	withFilter,
} from "./search";

const DAY = 24 * 60 * 60 * 1_000;
/** How many options a filter menu lists at once; search finds the rest. */
const OPTIONS = 32;

type Choice = "1w" | "4w" | "3m" | "custom" | "all";

const CHOICES: { value: Choice; label: string; long: string }[] = [
	...WINDOWS.map(({ label, long }) => ({ value: label, label, long })),
	{ value: "custom", label: "Custom", long: "Custom range" },
	{ value: "all", label: "All", long: "All time" },
];

const STATUSES: { value: AlertsStatus; label: string }[] = [
	{ value: "active", label: "Active" },
	{ value: "dismissed", label: "Dismissed" },
	{ value: "all", label: "All" },
];

const RESOURCES: Record<Filter, Dimension> = {
	branch: "branches",
	testbed: "testbeds",
	measure: "measures",
};

/** The names the chips show for the filters, by dimension. */
export type FilterNames = { [K in Filter]?: string | undefined };

const choiceOf = (window: ReportsWindow): Choice | undefined =>
	window.kind === "rolling"
		? WINDOWS.find(({ days }) => days === window.days)?.label
		: window.kind;

/** The status, the window, and the filters: a row of controls when wide; the status, two chips, and a sheet when narrow. */
const Controls = (props: {
	search: AlertsSearch;
	names: FilterNames;
	now: number;
	onSearch: (search: AlertsSearch) => void;
}) => {
	const [windowMenu, setWindowMenu] = createSignal(false);
	const [sheet, setSheet] = createSignal(false);
	let windowChip: HTMLButtonElement | undefined;
	const pick = (choice: Choice) => {
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
	const short = () =>
		CHOICES.find(({ value }) => value === choiceOf(props.search.window))
			?.label ?? "Custom";
	const count = () => filterCount(props.search);
	return (
		<>
			<div class="toolbar al-controls">
				<span class="eyebrow al-wide">Status</span>
				<Segmented
					name="al-status"
					class="al-status"
					aria-label="Status"
					options={STATUSES}
					value={props.search.status}
					onChange={(status) => props.onSearch({ ...props.search, status })}
				/>
				<span class="eyebrow al-wide">Window</span>
				<Segmented
					name="al-window"
					class="al-wide"
					aria-label="Window"
					options={CHOICES}
					value={choiceOf(props.search.window)}
					onChange={pick}
				/>
				<span class="eyebrow al-wide">Filters</span>
				<span class="al-wide al-filters">
					<Filters {...props} />
				</span>
				<div class="chips al-narrow">
					<span class="reports-anchor">
						<Chip
							ref={windowChip}
							label="window"
							caret
							aria-haspopup="menu"
							aria-expanded={windowMenu()}
							aria-label={`Window ${short()}, ${windowPhrase(
								props.search.window,
								props.now,
							).replace(/^In the last /, "")}`}
							onClick={() => setWindowMenu(!windowMenu())}
						>
							{short()}
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
						onClick={() => setSheet(true)}
					>
						{count() > 0 ? `Filters (${count()})` : "Filters"}
					</Chip>
					<Sheet open={sheet()} onClose={() => setSheet(false)} title="Filters">
						<div class="reports-sheet-filters">
							<Filters {...props} />
						</div>
					</Sheet>
				</div>
			</div>
			<Show when={props.search.window.kind === "custom" && props.search.window}>
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

const Filters = (props: {
	search: AlertsSearch;
	names: FilterNames;
	onSearch: (search: AlertsSearch) => void;
}) => {
	const { slug } = useProject();
	const client = useQueryClient();
	return (
		<For each={["branch", "testbed", "measure"] as const}>
			{(filter) => (
				<FilterMenu
					name={filter}
					value={props.search[filter]}
					// A cold link knows the UUID before the name.
					label={
						props.names[filter] ??
						(props.search[filter] === undefined ? undefined : "…")
					}
					search
					options={(text) => useOptions(RESOURCES[filter], text)}
					onPick={(uuid, label) => {
						// The chip names the pick without asking the API again.
						if (uuid !== undefined && label !== undefined) {
							client.setQueryData(
								nameKey(RESOURCES[filter], slug(), uuid),
								label,
							);
						}
						props.onSearch(withFilter(props.search, filter, uuid));
					}}
				/>
			)}
		</For>
	);
};

/** A project's branches, testbeds, or measures whose name or slug matches `text`, by UUID. */
const useOptions = (resource: Dimension, text: () => string) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => ({
		queryKey: ["console", resource, slug(), text()],
		// The last options stay while the next search loads, so the list never blinks empty.
		placeholderData: (previous: { uuid: string; name: string }[] | undefined) =>
			previous,
		queryFn: async ({ signal }: { signal: AbortSignal }) => {
			const params = new URLSearchParams({ per_page: String(OPTIONS) });
			if (text()) {
				params.set("search", text());
			}
			return (
				await api.get<{ uuid: string; name: string }[]>(
					`/v0/projects/${encodeURIComponent(slug())}/${resource}?${params}`,
					signal,
				)
			).data;
		},
	}));
	return () =>
		(result().data ?? []).map(({ uuid, name }) => ({
			value: uuid,
			label: name,
		}));
};
