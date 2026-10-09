import { useLocation, useNavigate } from "@solidjs/router";
import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Icon from "@bencherdev/ui/Icon";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import TextInput from "@bencherdev/ui/TextInput";
import {
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import {
	For,
	Match,
	Show,
	Switch,
	createComputed,
	createEffect,
	createMemo,
	createSignal,
	mapArray,
	on,
	onCleanup,
	untrack,
} from "solid-js";
import { DIMENSIONS, NEXT_PROJECTS } from "../paths";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { failureOf } from "../settings/failure";
import { hasMore, loadMore } from "../reports/rows";
import type { Range } from "../reports/rows";
import {
	type KeptRow,
	type Row,
	type RowBatch,
	afterArchive,
	archivePath,
	heldOf,
	listQuery,
	markArchived,
	thresholdsOf,
	withArchived,
	withKept,
} from "./data";
import { type Dimension, KINDS } from "./dimension";
import { archivedNote, unarchivedNote } from "./impact";
import {
	type ListSearch,
	type Sort,
	decodeSearch,
	directionText,
	encodeSearch,
	flipDirection,
	pickSort,
} from "./search";
import RowTable from "./RowTable";

const SEARCH_DELAY = 200;
const SKELETON_ROWS = 8;

type Batch = QueryObserverResult<RowBatch>;

const SORTS: { value: Sort; label: string }[] = [
	{ value: "name", label: "Name" },
	{ value: "created", label: "Created" },
	{ value: "last_used", label: "Last used" },
];

/** One dimension's rows, a batch at a time, with Archive and Unarchive on each. */
const List = (props: {
	dimension: Dimension;
	/** The rows in a batch, fixed for the page's life so batches line up. */
	perPage: number;
	edit: boolean;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const location = useLocation();
	const navigate = useNavigate();
	const kind = () => KINDS[props.dimension];
	const search = createMemo(() =>
		decodeSearch(new URLSearchParams(location.search)),
	);
	const setSearch = (next: ListSearch) => {
		const query = encodeSearch(next);
		navigate(`/${slug()}/${props.dimension}${query ? `?${query}` : ""}`, {
			scroll: false,
		});
	};

	const [count, setCount] = createSignal(1);
	createComputed(on(search, () => setCount(1), { defer: true }));
	// Changes on their way to the API: a next batch waits until they land, so
	// the rows it counts as still in view are the rows the API counts too.
	let sending = 0;
	const waiting: (() => void)[] = [];
	const settled = () =>
		sending === 0
			? Promise.resolve()
			: new Promise<void>((resolve) => {
					waiting.push(resolve);
				});
	const landed = () => {
		sending -= 1;
		if (sending === 0) {
			for (const resolve of waiting.splice(0)) {
				resolve();
			}
		}
	};
	/** Where a later batch starts: after the loaded rows still in the list's view. */
	const offsetOf = (ordinal: number, archived: boolean) => async () => {
		await settled();
		return untrack(() => distinct(loaded().slice(0, ordinal - 1))).filter(
			(row) => Boolean(row.archived) === archived,
		).length;
	};
	const queries = createMemo(
		mapArray(
			() => Array.from({ length: count() }, (_, index) => index + 1),
			(ordinal) =>
				useQueryResult(() =>
					listQuery(
						api,
						slug(),
						props.dimension,
						search(),
						{ ordinal, perPage: props.perPage },
						ordinal === 1 ? undefined : offsetOf(ordinal, search().archived),
					),
				),
		),
	);
	const batches = () => queries().map((batch) => batch());
	const loaded = createMemo(() => {
		const ready: Batch[] = [];
		for (const batch of batches()) {
			if (batch.data === undefined) {
				break;
			}
			ready.push(batch);
		}
		return ready;
	});
	const next = () => batches()[loaded().length];
	const listed = createMemo(() => distinct(loaded()));
	const view = createMemo(() => encodeSearch(search()));
	/** Rows changed on this page, drawn where they were under the view they changed in until the reader leaves. */
	const [kept, setKept] = createSignal<
		ReadonlyMap<string, KeptRow & { view: string }>
	>(new Map());
	const rows = createMemo(() =>
		withKept(
			listed(),
			[...kept().values()].filter((each) => each.view === view()),
		),
	);
	const first = () => loaded()[0]?.data;
	// The controls and rows paint with the first batch, so nothing on the page
	// moves when it answers; a later refine keeps them.
	const [painted, setPainted] = createSignal(false);
	createComputed(() => {
		if (first()) {
			setPainted(true);
		}
	});
	const busy = () => batches()[0]?.isPlaceholderData === true;

	const [range, setRange] = createSignal<Range>();
	createEffect(() => {
		const shown = range();
		const last = loaded().at(-1)?.data;
		if (
			shown &&
			last &&
			next() === undefined &&
			hasMore(
				{ reports: last.rows, total: last.total, batch: last.batch },
				listed().filter((row) => Boolean(row.archived) === search().archived)
					.length,
			) &&
			loadMore({ end: shown.end, loaded: rows().length, batch: props.perPage })
		) {
			setCount(batches().length + 1);
		}
	});
	// A change cancels the reads in flight, which would land over it; a next
	// batch cancelled that way asks again.
	createEffect(() => {
		const asked = next();
		if (asked?.isPending && !asked.isFetching && !asked.isError) {
			asked.refetch();
		}
	});

	const [arming, setArming] = createSignal<string>();
	const [note, setNote] = createSignal("");
	const [failure, setFailure] = createSignal("");
	createComputed(
		on(
			() => [props.dimension, search().archived],
			() => {
				setArming(undefined);
				setNote("");
				setFailure("");
			},
			{ defer: true },
		),
	);
	// Each row's changes reach the API in the order they were made, and a
	// refusal returns the row to what the API last confirmed.
	const queue = new Map<string, Promise<unknown>>();
	const confirmed = new Map<string, number | undefined>();
	const change = async (row: Row, archive: boolean) => {
		sending += 1;
		if (!confirmed.has(row.uuid)) {
			confirmed.set(row.uuid, row.archived);
		}
		setArming(undefined);
		setFailure("");
		const name = row.name;
		setNote(
			archive
				? archivedNote(name, thresholdsOf(row))
				: unarchivedNote(name, thresholdsOf(row), heldOf(row)),
		);
		const dimension = props.dimension;
		const project = slug();
		// A read in flight would land over the change.
		await client.cancelQueries({
			queryKey: ["console", dimension, project, "list"],
		});
		const at = archive ? Date.now() : undefined;
		const drawn = rows();
		const after =
			drawn[drawn.findIndex(({ uuid }) => uuid === row.uuid) - 1]?.uuid;
		markArchived(
			client,
			project,
			dimension,
			row.uuid,
			at,
			Boolean(row.archived),
		);
		setKept(
			new Map(kept()).set(row.uuid, {
				row: withArchived(row, at),
				after,
				view: view(),
			}),
		);
		const sent: Promise<unknown> = (queue.get(row.uuid) ?? Promise.resolve())
			.catch(() => {})
			.then(async () => {
				try {
					await api.send("PATCH", archivePath(project, dimension, row.uuid), {
						archived: archive,
					});
					confirmed.set(row.uuid, at);
					afterArchive(client, project, dimension);
				} catch (error) {
					// A later change on the row is on its way and says what it shows.
					if (queue.get(row.uuid) === sent) {
						const shown = kept().get(row.uuid);
						const back = confirmed.get(row.uuid);
						markArchived(
							client,
							project,
							dimension,
							row.uuid,
							back,
							shown && Boolean(shown.row.archived),
						);
						if (shown) {
							setKept(
								new Map(kept()).set(row.uuid, {
									...shown,
									row: withArchived(shown.row, back),
								}),
							);
						}
						setNote("");
					}
					setFailure(
						failureOf(
							error,
							`Bencher did not ${archive ? "archive" : "unarchive"} ${name}`,
							`The Bencher API did not answer, so ${name} is ${archive ? "still active" : "still archived"}.`,
						),
					);
					throw error;
				}
			});
		queue.set(row.uuid, sent);
		sent.then(landed, landed);
	};

	const [text, setText] = createSignal(search().search);
	let timer: ReturnType<typeof setTimeout> | undefined;
	onCleanup(() => clearTimeout(timer));
	createComputed(
		on(
			() => search().search,
			(typed) => {
				if (typed !== text().trim()) {
					clearTimeout(timer);
					setText(typed);
				}
			},
			{ defer: true },
		),
	);
	const filter = (typed: string) => {
		setText(typed);
		clearTimeout(timer);
		timer = setTimeout(
			() => setSearch({ ...search(), search: typed.trim() }),
			SEARCH_DELAY,
		);
	};

	return (
		<>
			<nav class="dm-lists" aria-label="Dimension lists">
				<For each={DIMENSIONS}>
					{(dimension) => (
						<a
							href={`${NEXT_PROJECTS}/${slug()}/${dimension}`}
							aria-current={dimension === props.dimension ? "page" : undefined}
						>
							{KINDS[dimension].label}
						</a>
					)}
				</For>
			</nav>
			<Show
				when={painted()}
				fallback={
					<Switch>
						<Match when={batches()[0]?.isLoadingError}>
							<Banner status="error" role="alert" class="load-error">
								<span class="grow">
									{kind().label} did not load: the Bencher API did not answer.
								</span>
								<Button size="sm" onClick={() => batches()[0]?.refetch()}>
									Retry
								</Button>
							</Banner>
						</Match>
						<Match when={true}>
							<div class="ui-table-scroll dm-skeleton" aria-busy="true">
								<span class="sr-only">
									Loading {kind().label.toLowerCase()}
								</span>
								<For each={Array.from({ length: SKELETON_ROWS })}>
									{() => <Skeleton size="text" class="dm-skeleton-row" />}
								</For>
							</div>
						</Match>
					</Switch>
				}
			>
				<div class="toolbar dm-toolbar">
					<TextInput
						type="search"
						size="sm"
						class="dm-filter"
						placeholder={`Filter ${kind().label.toLowerCase()}`}
						aria-label={`Filter ${kind().label.toLowerCase()}`}
						value={text()}
						onInput={(event) => filter(event.currentTarget.value)}
					/>
					<span class="eyebrow" id="dm-sort">
						Sort
					</span>
					<Segmented
						name="dm-sort"
						aria-labelledby="dm-sort"
						value={search().sort}
						onChange={(sort) => setSearch(pickSort(search(), sort))}
						options={SORTS}
					/>
					<Button
						size="sm"
						class="dm-direction"
						aria-label={`Sort direction: ${directionText(search().sort, search().direction)}`}
						onClick={() => setSearch(flipDirection(search()))}
					>
						<svg
							class="ui-icon"
							viewBox="0 0 24 24"
							fill="none"
							stroke="currentColor"
							stroke-width="2.2"
							stroke-linecap="round"
							stroke-linejoin="round"
							aria-hidden="true"
						>
							<path
								d={
									search().direction === "desc"
										? "M12 5v14M6 13l6 6 6-6"
										: "M12 19V5M6 11l6-6 6 6"
								}
							/>
						</svg>
						{directionText(search().sort, search().direction)}
					</Button>
					<span class="spacer" />
					<Segmented
						name="dm-status"
						aria-label="Status"
						value={search().archived ? "archived" : "active"}
						onChange={(status) =>
							setSearch({ ...search(), archived: status === "archived" })
						}
						options={[
							{
								value: "active",
								label: (
									<>
										Active <span class="count">{first()?.active}</span>
									</>
								),
							},
							{
								value: "archived",
								label: (
									<>
										Archived <span class="count">{first()?.archived}</span>
									</>
								),
							},
						]}
					/>
				</div>
				<p class="set-status dm-note" role="status">
					<Show when={note()}>
						<Icon name="check" />
						{note()}
					</Show>
				</p>
				<Show when={failure()}>
					<p class="ferror" role="alert">
						{failure()}
					</p>
				</Show>
				<Switch>
					<Match when={first() && rows().length === 0 && !busy()}>
						<div class="ui-table-scroll dm-empty">
							<p>
								{search().search
									? `No ${kind().label.toLowerCase()} match "${search().search}".`
									: search().archived
										? `No archived ${kind().label.toLowerCase()}.`
										: `No ${kind().label.toLowerCase()} yet. A run creates them.`}
							</p>
							<Show when={search().search}>
								<Button
									size="sm"
									onClick={() => setSearch({ ...search(), search: "" })}
								>
									Clear the filter
								</Button>
							</Show>
						</div>
					</Match>
					<Match when={rows().length > 0}>
						<RowTable
							slug={slug()}
							dimension={props.dimension}
							archived={search().archived}
							rows={rows()}
							total={
								(first()?.total ?? listed().length) +
								rows().length -
								listed().length
							}
							edit={props.edit}
							arming={arming()}
							busy={busy()}
							loading={next()?.isFetching ?? false}
							onRetry={
								next()?.isLoadingError ? () => next()?.refetch() : undefined
							}
							onRange={setRange}
							onArm={setArming}
							onChange={change}
						/>
					</Match>
					<Match when={batches()[0]?.isLoadingError}>
						<Banner status="error" role="alert" class="load-error">
							<span class="grow">
								{kind().label} did not load: the Bencher API did not answer.
							</span>
							<Button size="sm" onClick={() => batches()[0]?.refetch()}>
								Retry
							</Button>
						</Banner>
					</Match>
				</Switch>
			</Show>
		</>
	);
};

export default List;

/** The rows of each batch in order, each once: a row that moves between batches would otherwise show twice. */
const distinct = (batches: Batch[]) => {
	const seen = new Set<string>();
	const all: Row[] = [];
	for (const batch of batches) {
		for (const row of batch.data?.rows ?? []) {
			if (!seen.has(row.uuid)) {
				seen.add(row.uuid);
				all.push(row);
			}
		}
	}
	return all;
};
