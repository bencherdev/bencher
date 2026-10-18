import { useNavigate } from "@solidjs/router";
import {
	For,
	type JSX,
	Show,
	createMemo,
	createSignal,
	createUniqueId,
	onCleanup,
	onMount,
} from "solid-js";
import { recentSlugs, rememberVersion } from "../memory";
import { useProject } from "../project";
import { consoleUserQuery } from "../queries";
import { useQueryResult } from "../query";
import { visibleRange } from "../reports/rows";
import {
	type Split,
	type SwitcherRow,
	searchRows,
	switcherRows,
} from "./model";

/** In a sheet, the sheet is the dialog. */
const DIALOG = { role: "dialog", "aria-label": "Switch project" } as const;
/** Each row's height and the gap under it, which the virtual list steps by. */
const STEP = 55;
/** The list's most height, the rows it can show at once. */
const HEIGHT = 440;
const OVERSCAN = 6;
/** The list's padding above its first row. */
const PADDING = 4;

/** Every project the reader belongs to, to search and open. */
const Switcher = (props: {
	reader: string;
	/** The project shown now. */
	slug: string;
	organization: { name: string; uuid: string } | undefined;
	crumb: HTMLElement | undefined;
	/** Closes it, giving the focus back to the crumb when `refocus`. */
	onClose: (refocus: boolean) => void;
	/** Held in a modal sheet, which is the dialog and closes it. */
	sheet?: boolean;
}) => {
	const { api } = useProject();
	const navigate = useNavigate();
	const user = useQueryResult(() => consoleUserQuery(api, props.reader));
	const rows = createMemo(() => {
		const data = user().data;
		return data
			? switcherRows(data, recentSlugs(localStorage), props.slug, Date.now())
			: undefined;
	});
	const [query, setQuery] = createSignal("");
	const matches = createMemo(() => searchRows(rows() ?? [], query()));
	const [active, setActive] = createSignal(-1);
	const [scroll, setScroll] = createSignal(0);
	const range = () =>
		visibleRange({
			top: -scroll(),
			viewport: HEIGHT,
			rowHeight: STEP,
			count: matches().length,
			overscan: OVERSCAN,
		});
	const id = createUniqueId();
	const optionId = (index: number) => `${id}-${index}`;
	let panel: HTMLDivElement | undefined;
	let input: HTMLInputElement | undefined;
	let list: HTMLDivElement | undefined;

	const search = (value: string) => {
		setQuery(value);
		// A search starts on its first match, so Enter opens it.
		setActive(value.trim() ? 0 : -1);
		moveTo(0);
	};
	const moveTo = (top: number) => {
		if (list) {
			list.scrollTop = top;
		}
		setScroll(list?.scrollTop ?? 0);
	};
	// The active row is scrolled into view, and so drawn, before anything names it.
	const activate = (index: number) => {
		const top = PADDING + index * STEP;
		if (top < scroll()) {
			moveTo(top - PADDING);
		} else if (top + STEP > scroll() + HEIGHT) {
			moveTo(top + STEP - HEIGHT);
		}
		setActive(index);
	};
	const go = (row: SwitcherRow) => {
		// A pick leads the next opening, even one whose data is all cached.
		rememberVersion(localStorage, row.slug, row.classic ? 0 : 1);
		props.onClose(true);
		if (row.classic) {
			location.assign(row.href);
		} else {
			navigate(row.href, { resolve: false });
		}
	};
	const onKeyDown = (event: KeyboardEvent) => {
		const count = matches().length;
		if (event.key === "ArrowDown" && count) {
			activate(Math.min(active() + 1, count - 1));
		} else if (event.key === "ArrowUp" && count) {
			activate(Math.max(active() - 1, 0));
		} else if (event.key === "Enter") {
			const match = matches()[active()];
			if (match) {
				go(match.row);
			}
		} else {
			return;
		}
		event.preventDefault();
	};

	onMount(() => {
		input?.focus();
		if (props.sheet) {
			return;
		}
		// A press anywhere else, or the focus moving on, closes it in place.
		const away = (event: Event) => {
			const target = event.target as Node | null;
			if (!(panel?.contains(target) || props.crumb?.contains(target))) {
				props.onClose(false);
			}
		};
		document.addEventListener("pointerdown", away);
		document.addEventListener("focusin", away);
		onCleanup(() => {
			document.removeEventListener("pointerdown", away);
			document.removeEventListener("focusin", away);
		});
	});

	return (
		<div
			ref={panel}
			class="pop sw"
			{...(props.sheet ? {} : DIALOG)}
			onKeyDown={(event) => {
				if (event.key === "Escape") {
					event.preventDefault();
					props.onClose(true);
				}
			}}
		>
			<div class="sw-search">
				<Glyph d="M11 4a7 7 0 1 0 0 14 7 7 0 0 0 0-14zM20 20l-3.5-3.5" />
				<input
					ref={input}
					type="text"
					role="combobox"
					aria-label="Search projects"
					aria-expanded={matches().length > 0}
					aria-controls={matches().length ? id : undefined}
					aria-autocomplete="list"
					aria-activedescendant={
						active() >= range().start && active() < range().end
							? optionId(active())
							: undefined
					}
					placeholder="Search projects"
					autocomplete="off"
					spellcheck={false}
					value={query()}
					onInput={(event) => search(event.currentTarget.value)}
					onKeyDown={onKeyDown}
				/>
			</div>
			<div class="sw-head">
				<span class="eyebrow">{query().trim() ? "Matches" : "Recent"}</span>
				<span class="sw-count">
					{count(
						rows(),
						user().data?.organizations.length ?? 0,
						matches().length,
						query(),
					)}
				</span>
			</div>
			<Show
				when={matches().length > 0}
				fallback={
					<div class="sw-empty" role="status">
						<Show
							when={rows()?.length}
							fallback={
								<b>
									{rows()
										? "No projects"
										: user().error
											? "Projects did not load"
											: ""}
								</b>
							}
						>
							<b>No matches</b>
							<button
								type="button"
								class="lnk"
								onClick={() => {
									search("");
									input?.focus();
								}}
							>
								Clear search
							</button>
						</Show>
					</div>
				}
			>
				<div
					ref={list}
					id={id}
					class="sw-list"
					role="listbox"
					aria-label="Projects"
					onScroll={(event) => setScroll(event.currentTarget.scrollTop)}
				>
					<div
						class="sw-rows"
						style={{
							"padding-top": `${range().start * STEP + PADDING}px`,
							"padding-bottom": `${(matches().length - range().end) * STEP + 6}px`,
						}}
					>
						<For each={matches().slice(range().start, range().end)}>
							{(match, offset) => {
								const index = () => range().start + offset();
								return (
									<a
										id={optionId(index())}
										class="sw-row"
										classList={{ "is-kbd": index() === active() }}
										role="option"
										tabindex="-1"
										aria-selected={index() === active()}
										aria-posinset={index() + 1}
										aria-setsize={matches().length}
										href={match.row.href}
										onClick={(event) => {
											// A click meant for another tab or window is the browser's.
											if (
												event.button ||
												event.metaKey ||
												event.ctrlKey ||
												event.shiftKey ||
												event.altKey
											) {
												return;
											}
											event.preventDefault();
											go(match.row);
										}}
									>
										<span class="sw-name">
											<span class="sw-title">
												<Marked split={match.name} />
												<Show when={match.row.current}>
													<span class="sw-cur">Current</span>
												</Show>
											</span>
											<Show when={match.organization}>
												{(organization) => (
													<span class="sw-org">
														<Marked split={organization()} />
													</span>
												)}
											</Show>
										</span>
										<span class="sw-run">
											<Show
												when={!match.row.classic}
												fallback={
													<span class="sw-classic">
														Classic
														<Glyph d="M7 17L17 7M8 7h9v9" />
													</span>
												}
											>
												<Show
													when={match.row.age}
													fallback={<span class="faint">No runs</span>}
												>
													{(age) => <span>{age()}</span>}
												</Show>
											</Show>
										</span>
										<span class="sw-alerts">
											<Show when={match.row.alerts > 0}>
												<span class="reports-active">
													<span class="reports-dot" aria-hidden="true" />
													{match.row.alerts}
													<span class="sr-only"> active</span>{" "}
													{match.row.alerts === 1 ? "alert" : "alerts"}
												</span>
											</Show>
										</span>
									</a>
								);
							}}
						</For>
					</div>
				</div>
			</Show>
			<Show when={props.organization}>
				{(organization) => (
					<div class="sw-foot">
						<a
							href={`/console/organizations/${organization().uuid}/projects/add`}
						>
							<Glyph d="M12 5v14M5 12h14" />
							New project
						</a>
						<span class="spacer" />
						<a href={`/console/organizations/${organization().uuid}/settings`}>
							<Glyph d="M4 21v-7M4 10V3M12 21v-9M12 8V3M20 21v-5M20 12V3M1 14h6M9 8h6M17 16h6" />
							{organization().name} settings
						</a>
					</div>
				)}
			</Show>
		</div>
	);
};

export default Switcher;

const count = (
	rows: readonly SwitcherRow[] | undefined,
	organizations: number,
	matched: number,
	query: string,
) => {
	if (!rows) {
		return "";
	}
	if (query.trim()) {
		return `${matched} of ${rows.length}`;
	}
	const projects = `${rows.length} ${rows.length === 1 ? "project" : "projects"}`;
	return organizations > 1
		? `${projects}, ${organizations} organizations`
		: projects;
};

const Marked = (props: { split: Split }) => (
	<span>
		{props.split[0]}
		<Show when={props.split[1]}>
			<mark>{props.split[1]}</mark>
		</Show>
		{props.split[2]}
	</span>
);

const Glyph = (props: { d: string }): JSX.Element => (
	<svg
		class="ui-icon"
		viewBox="0 0 24 24"
		fill="none"
		stroke="currentColor"
		stroke-width="2"
		stroke-linecap="round"
		stroke-linejoin="round"
		aria-hidden="true"
	>
		<path d={props.d} />
	</svg>
);
