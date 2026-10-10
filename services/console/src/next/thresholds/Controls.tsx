import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import Sheet from "@bencherdev/ui/Sheet";
import { useQueryClient } from "@tanstack/solid-query";
import { For, Show, createSignal, createUniqueId } from "solid-js";
import { useNarrow } from "../plot/narrow";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { CustomRange, FilterMenu } from "../reports/Controls";
import { type Dimension, nameKey } from "./query";
import {
	type Choice,
	type Filter,
	type ThresholdsSearch,
	WINDOW_CHOICES,
	filterCount,
	pickWindow,
	windowLabel,
	withFilter,
} from "./search";

/** The names the chips show for the filters, by dimension. */
export type FilterNames = { [K in Filter]?: string | undefined };

/** How many options a filter menu lists at once; search finds the rest. */
const OPTIONS = 32;

const RESOURCES: Record<Filter, Dimension> = {
	branch: "branches",
	testbed: "testbeds",
	measure: "measures",
};

/** The status, the filters, and the window: a row of controls when wide, a full-width status, two chips, and a sheet when narrow. */
const Controls = (props: {
	search: ThresholdsSearch;
	names: FilterNames;
	now: number;
	onSearch: (search: ThresholdsSearch) => void;
}) => {
	const narrow = useNarrow();
	const [windowMenu, setWindowMenu] = createSignal(false);
	const [sheet, setSheet] = createSignal(false);
	let windowChip: HTMLButtonElement | undefined;
	const windowId = createUniqueId();
	const choice = () => windowLabel(props.search.window);
	const long = () =>
		WINDOW_CHOICES.find(({ value }) => value === choice())?.long ?? "";
	const pick = (value: Choice) => {
		const window = pickWindow(value, props.search.window, props.now);
		if (window) {
			props.onSearch({ ...props.search, window });
		}
	};
	const count = () => filterCount(props.search);
	return (
		<>
			<div class="toolbar th-controls">
				<Segmented
					name="th-status"
					full={narrow()}
					aria-label="Status"
					options={[
						{ value: "active", label: "Active" },
						{ value: "archived", label: "Archived" },
					]}
					value={props.search.archived ? "archived" : "active"}
					onChange={(status) =>
						props.onSearch({ ...props.search, archived: status === "archived" })
					}
				/>
				<span class="eyebrow th-wide">Filters</span>
				<span class="th-wide th-filters">
					<Filters {...props} />
				</span>
				<span class="spacer th-wide" />
				<span class="eyebrow th-wide" id={windowId}>
					Alerts in
				</span>
				<Segmented
					name="th-window"
					class="th-wide"
					aria-labelledby={windowId}
					options={WINDOW_CHOICES}
					value={choice()}
					onChange={pick}
				/>
				<div class="chips th-narrow">
					<span class="reports-anchor">
						<Chip
							ref={windowChip}
							label="alerts in"
							caret
							aria-haspopup="menu"
							aria-expanded={windowMenu()}
							aria-label={`Alerts in ${long()}`}
							onClick={() => setWindowMenu(!windowMenu())}
						>
							{WINDOW_CHOICES.find(({ value }) => value === choice())?.label}
						</Chip>
						<Show when={windowMenu()}>
							<Menu
								label="Alerts in"
								anchor={windowChip}
								onClose={() => setWindowMenu(false)}
							>
								<For each={WINDOW_CHOICES}>
									{({ value, long }) => (
										<MenuItemRadio
											checked={choice() === value}
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
							<Filters {...props} />
						</div>
					</Sheet>
				</div>
			</div>
			<Show when={props.search.window.kind === "custom" && props.search.window}>
				{(custom) => (
					<CustomRange
						window={custom()}
						onWindow={(window) => {
							if (window.kind === "custom") {
								props.onSearch({ ...props.search, window });
							}
						}}
					/>
				)}
			</Show>
		</>
	);
};

export default Controls;

const Filters = (props: {
	search: ThresholdsSearch;
	names: FilterNames;
	onSearch: (search: ThresholdsSearch) => void;
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
					options={(text) =>
						useOptions(RESOURCES[filter], text, () => props.search.archived)
					}
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

/**
 * A project's branches, testbeds, or measures whose name or slug matches
 * `text`, by UUID; with `archived`, the archived ones too, since archiving one
 * archives its thresholds.
 */
const useOptions = (
	resource: Dimension,
	text: () => string,
	archived: () => boolean,
) => {
	const active = useListed(resource, text, false, () => true);
	const retired = useListed(resource, text, true, archived);
	return () =>
		(archived() ? [...active(), ...retired()].sort(byName) : active()).map(
			({ uuid, name }) => ({ value: uuid, label: name }),
		);
};

/** One page of a project's active or archived branches, testbeds, or measures. */
const useListed = (
	resource: Dimension,
	text: () => string,
	archived: boolean,
	enabled: () => boolean,
) => {
	const { api, slug } = useProject();
	const result = useQueryResult(() => ({
		// The active ones under the Reports list's key, since the list is the same.
		queryKey: archived
			? ["console", resource, slug(), text(), "archived"]
			: ["console", resource, slug(), text()],
		enabled: enabled(),
		// The last options stay while the next search loads, so the list never blinks empty.
		placeholderData: (previous: Named[] | undefined) => previous,
		queryFn: async ({ signal }: { signal: AbortSignal }) => {
			const params = new URLSearchParams({ per_page: String(OPTIONS) });
			if (text()) {
				params.set("search", text());
			}
			if (archived) {
				params.set("archived", "true");
			}
			return (
				await api.get<Named[]>(
					`/v0/projects/${encodeURIComponent(slug())}/${resource}?${params}`,
					signal,
				)
			).data;
		},
	}));
	return () => result().data ?? [];
};

const byName = (a: Named, b: Named) =>
	a.name < b.name ? -1 : a.name > b.name ? 1 : 0;

interface Named {
	uuid: string;
	name: string;
}
