import Chip from "@bencherdev/ui/Chip";
import Menu, { MenuItemRadio } from "@bencherdev/ui/Menu";
import Segmented from "@bencherdev/ui/Segmented";
import Sheet from "@bencherdev/ui/Sheet";
import { For, Show, createSignal, createUniqueId } from "solid-js";
import { useNarrow } from "../plot/narrow";
import { Clear, CustomRange } from "../reports/Controls";
import DimensionFilters, {
	type FilterNames,
} from "../reports/DimensionFilters";
import {
	type Choice,
	type ThresholdsSearch,
	WINDOW_CHOICES,
	filterCount,
	pickWindow,
	windowLabel,
	withFilter,
	withoutFilters,
} from "./search";

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
	const shown = () => WINDOW_CHOICES.find(({ value }) => value === choice());
	const pick = (value: Choice) => {
		const window = pickWindow(value, props.search.window, props.now);
		if (window) {
			props.onSearch({ ...props.search, window });
		}
	};
	const count = () => filterCount(props.search);
	const clear = () => props.onSearch(withoutFilters(props.search));
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
					<DimensionFilters
						values={props.search}
						names={props.names}
						archived={props.search.archived}
						onPick={(filter, uuid) =>
							props.onSearch(withFilter(props.search, filter, uuid))
						}
						clearable={count() > 0}
						onClear={clear}
					/>
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
							aria-label={`Alerts in ${shown()?.label}, ${shown()?.long}`}
							onClick={() => setWindowMenu(!windowMenu())}
						>
							{shown()?.label}
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
							<DimensionFilters
								values={props.search}
								names={props.names}
								archived={props.search.archived}
								onPick={(filter, uuid) =>
									props.onSearch(withFilter(props.search, filter, uuid))
								}
								clearable={count() > 0}
								onClear={clear}
							/>
						</div>
					</Sheet>
					<Clear
						when={count() > 0}
						to="[aria-haspopup=dialog]"
						onClear={clear}
					/>
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
