import Banner from "@bencherdev/ui/Banner";
import Button from "@bencherdev/ui/Button";
import Heading from "@bencherdev/ui/Heading";
import Icon from "@bencherdev/ui/Icon";
import Skeleton from "@bencherdev/ui/Skeleton";
import {
	type QueryClient,
	type QueryObserverResult,
	useQueryClient,
} from "@tanstack/solid-query";
import {
	For,
	type JSX,
	Match,
	Show,
	Switch,
	createComputed,
	createSignal,
} from "solid-js";
import type {
	JsonBranch,
	JsonConsoleBranchRow,
	JsonTestbed,
	JsonMeasure,
} from "../../types/bencher";
import type { Api } from "../api";
import { ApiError } from "../api";
import { NEXT_PROJECTS, projectPath } from "../paths";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { failureOf } from "../settings/failure";
import {
	BenchmarkCards,
	BranchCards,
	MeasureCards,
	TestbedCards,
	type ThresholdsState,
} from "./Cards";
import {
	type Detail,
	afterArchive,
	archivePath,
	detailQuery,
	explorePath,
	heldOf,
	markArchived,
	markDetailArchived,
	recentReportsQuery,
	rowQuery,
	thresholdsOf,
	thresholdsQuery,
	variantsQuery,
} from "./data";
import { type Dimension, KINDS } from "./dimension";
import { archiveImpact, archivedNote, unarchivedNote } from "./impact";
import { Impact } from "./RowTable";
import { shortDate } from "./time";

/** Start a dimension page's first round of requests before its code asks for them. */
export const prefetchInspect = (
	client: QueryClient,
	api: Api,
	slug: string,
	dimension: Dimension,
	ref: string,
) => {
	const archived = Boolean(
		client.getQueryData<Detail>(detailQuery(api, slug, dimension, ref).queryKey)
			?.archived,
	);
	const queries = [
		detailQuery(api, slug, dimension, ref),
		rowQuery(api, slug, dimension, ref, archived),
		...(KINDS[dimension].threshold
			? [thresholdsQuery(api, slug, dimension, ref, archived)]
			: [
					variantsQuery(api, slug, ref, false),
					variantsQuery(api, slug, ref, true),
				]),
		...(dimension === "branches"
			? [recentReportsQuery(api, slug, ref, archived)]
			: []),
	];
	for (const query of queries) {
		client.prefetchQuery(query as Parameters<QueryClient["prefetchQuery"]>[0]);
	}
};

type Note = { archived: boolean; text: string };

/** One branch, testbed, benchmark, or measure: what it carries, read only, with Archive. */
const Inspect = (props: {
	dimension: Dimension;
	/** The slug or UUID its path names. */
	entry: string;
	edit: boolean;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const kind = () => KINDS[props.dimension];
	const detail = useQueryResult(() =>
		detailQuery(api, slug(), props.dimension, props.entry),
	);
	// The public API writes an active dimension's archived time as null.
	const archived = () => Boolean(detail().data?.archived);
	const row = useQueryResult(() =>
		rowQuery(api, slug(), props.dimension, props.entry, archived()),
	);
	const thresholds = useQueryResult(() => ({
		...thresholdsQuery(api, slug(), props.dimension, props.entry, archived()),
		enabled: kind().threshold !== undefined,
	}));
	const reports = useQueryResult(() => ({
		...recentReportsQuery(api, slug(), props.entry, archived()),
		enabled: props.dimension === "branches",
	}));
	const variants = [false, true].map((listed) =>
		useQueryResult(() => ({
			...variantsQuery(api, slug(), props.entry, listed),
			enabled: props.dimension === "benchmarks",
		})),
	);
	// The page paints once its first round has answered, so nothing on it moves
	// as the answers arrive; later changes keep it painted.
	const [painted, setPainted] = createSignal(false);
	createComputed(() => {
		const settled = (result: QueryObserverResult<unknown>) =>
			!result.isEnabled ||
			((result.data !== undefined || result.isError) &&
				!result.isPlaceholderData);
		if (
			detail().isLoadingError ||
			(settled(detail()) &&
				[
					row(),
					thresholds(),
					reports(),
					...variants.map((each) => each()),
				].every(settled))
		) {
			setPainted(true);
		}
	});

	/** The thresholds that archive, or come back, with it; a benchmark has none. */
	const counts = () => {
		if (kind().threshold === undefined) {
			return { back: undefined, held: 0 };
		}
		const found = row().data;
		if (found) {
			return { back: thresholdsOf(found), held: heldOf(found) };
		}
		const total = thresholds().data?.total ?? 0;
		return { back: total, held: 0 };
	};

	const [arming, setArming] = createSignal(false);
	let archiveButton: HTMLButtonElement | undefined;
	let undoButton: HTMLButtonElement | undefined;
	// The control that replaces the one used keeps the focus nearby.
	const focus = (target: () => HTMLElement | undefined) =>
		requestAnimationFrame(() => target()?.focus());
	const [note, setNote] = createSignal<Note>();
	const [failure, setFailure] = createSignal("");
	// Changes reach the API in the order they were made, and a refusal returns
	// the page to what the API last confirmed.
	let queue: Promise<unknown> = Promise.resolve();
	let confirmed: { archived: string | undefined } | undefined;
	const change = async (archive: boolean) => {
		const shown = detail().data;
		if (!shown) {
			return;
		}
		confirmed ??= { archived: shown.archived ?? undefined };
		setArming(false);
		setFailure("");
		const { back, held } = counts();
		setNote({
			archived: archive,
			text: archive
				? archivedNote(shown.name, back)
				: unarchivedNote(shown.name, back, held),
		});
		const dimension = props.dimension;
		const project = slug();
		// A read in flight would land over the change.
		await Promise.all([
			client.cancelQueries({
				queryKey: detailQuery(api, project, dimension, props.entry).queryKey,
			}),
			client.cancelQueries({
				queryKey: ["console", dimension, project, "list"],
			}),
		]);
		const at = archive ? new Date().toISOString() : undefined;
		const show = (archived: string | undefined) => {
			const was = markDetailArchived(
				client,
				project,
				dimension,
				props.entry,
				archived,
			);
			markArchived(
				client,
				project,
				dimension,
				shown.uuid,
				archived === undefined ? undefined : Date.parse(archived),
				was,
			);
		};
		show(at);
		const sent: Promise<unknown> = queue
			.catch(() => {})
			.then(async () => {
				try {
					await api.send("PATCH", archivePath(project, dimension, shown.uuid), {
						archived: archive,
					});
					confirmed = { archived: at };
					afterArchive(client, project, dimension);
				} catch (error) {
					// A later change is on its way and says what the page shows.
					if (queue === sent) {
						show(confirmed?.archived);
						setNote(undefined);
					}
					setFailure(
						failureOf(
							error,
							`Bencher did not ${archive ? "archive" : "unarchive"} ${shown.name}`,
							`The Bencher API did not answer, so ${shown.name} is ${archive ? "still active" : "still archived"}.`,
						),
					);
				}
			});
		queue = sent;
	};

	const explore = () => {
		const uuid = detail().data?.uuid;
		return uuid
			? explorePath(slug(), props.dimension, uuid)
			: projectPath(slug(), "explore");
	};
	const list = () => `${NEXT_PROJECTS}/${slug()}/${props.dimension}`;
	const thresholdsState = (): ThresholdsState => ({
		data: thresholds().data,
		failed: thresholds().isLoadingError,
		retry: () => thresholds().refetch(),
	});

	return (
		<Switch>
			<Match
				when={
					detail().isLoadingError &&
					detail().error instanceof ApiError &&
					(detail().error as ApiError).kind === "not_found"
				}
			>
				<div class="pagehead">
					<div class="crumb-line">
						<a href={list()}>Dimensions</a>
						<span aria-hidden="true">/</span>
						<a href={list()}>{kind().label}</a>
						<span aria-hidden="true">/</span>
					</div>
					<Heading level={1} size="xl">
						No {kind().one} {props.entry}
					</Heading>
					<p class="lede">
						This project has no {kind().one} at this address.{" "}
						<a href={list()}>See every {kind().one}</a>.
					</p>
				</div>
			</Match>
			<Match when={detail().isLoadingError}>
				<Banner status="error" role="alert" class="load-error">
					<span class="grow">
						This {kind().one} did not load: the Bencher API did not answer.
					</span>
					<Button size="sm" onClick={() => detail().refetch()}>
						Retry
					</Button>
				</Banner>
			</Match>
			<Match when={!painted()}>
				<div class="pagehead" aria-busy="true">
					<Skeleton size="text" class="title-skel" />
					<span class="sr-only">Loading the {kind().one}</span>
				</div>
				<Skeleton size="card" />
			</Match>
			<Match when={true}>
				<div class="sethead dm-head">
					<div class="ph-title">
						<div class="crumb-line">
							<a href={list()}>Dimensions</a>
							<span aria-hidden="true">/</span>
							<a href={list()}>{kind().label}</a>
							<span aria-hidden="true">/</span>
						</div>
						<div class="dm-title">
							<Show when={detail().data}>
								{(shown) => (
									<>
										<Heading level={1} size="xl">
											{shown().name}
										</Heading>
										<Show when={archived()}>
											<span class="dm-pill">Archived</span>
										</Show>
									</>
								)}
							</Show>
						</div>
						<p class="ph-sub dm-sub-line">
							<Subtitle
								dimension={props.dimension}
								detail={detail().data}
								row={row().data ?? undefined}
							/>
						</p>
					</div>
					<div class="ph-actions dm-actions">
						<a
							class="ui-button"
							data-variant="secondary"
							data-size="md"
							href={explore()}
						>
							Open in Explore
						</a>
						<Show
							when={props.edit}
							fallback={
								<p class="ro-note">
									<Icon name="read-only" />
									Read only. Ask a project Maintainer for access.
								</p>
							}
						>
							<Show
								when={archived()}
								fallback={
									<Button
										ref={(element: HTMLButtonElement) => {
											archiveButton = element;
										}}
										aria-expanded={arming()}
										disabled={!detail().data}
										onClick={() => setArming(!arming())}
									>
										Archive
									</Button>
								}
							>
								<Button
									onClick={() => {
										change(false);
										focus(() => undoButton);
									}}
								>
									Unarchive
								</Button>
							</Show>
						</Show>
					</div>
				</div>
				<Show when={arming() && detail().data}>
					{(shown) => (
						<Impact
							name={shown().name}
							text={archiveImpact(shown().name, counts().back)}
							confirm={`Archive ${shown().name}`}
							onConfirm={() => {
								change(true);
								focus(() => undoButton);
							}}
							onCancel={() => {
								setArming(false);
								focus(() => archiveButton);
							}}
						/>
					)}
				</Show>
				<Show when={note()}>
					{(shown) => (
						<div class="dm-done" role="status">
							<Icon name="check" />
							<span class="grow">{shown().text}</span>
							<button
								ref={(element: HTMLButtonElement) => {
									undoButton = element;
								}}
								type="button"
								class="lnk"
								onClick={() => change(!shown().archived)}
							>
								Undo
							</button>
						</div>
					)}
				</Show>
				<Show when={failure()}>
					<p class="ferror" role="alert">
						{failure()}
					</p>
				</Show>
				<Switch>
					<Match when={props.dimension === "branches"}>
						<BranchCards
							slug={slug()}
							branch={detail().data as JsonBranch | undefined}
							row={row().data as JsonConsoleBranchRow | null | undefined}
							reports={reports().data}
							reportsFailed={reports().isLoadingError}
							reportsRetry={() => reports().refetch()}
							thresholds={thresholdsState()}
						/>
					</Match>
					<Match when={props.dimension === "testbeds"}>
						<TestbedCards
							slug={slug()}
							testbed={detail().data as JsonTestbed | undefined}
							thresholds={thresholdsState()}
						/>
					</Match>
					<Match when={props.dimension === "measures"}>
						<MeasureCards
							slug={slug()}
							measure={detail().data as JsonMeasure | undefined}
							thresholds={thresholdsState()}
						/>
					</Match>
					<Match when={props.dimension === "benchmarks"}>
						<BenchmarkCards
							benchmark={props.entry}
							name={detail().data?.name}
							edit={props.edit}
						/>
					</Match>
				</Switch>
			</Match>
		</Switch>
	);
};

export default Inspect;

const Subtitle = (props: {
	dimension: Dimension;
	detail: Detail | undefined;
	row: { last_report?: number; start_point?: string } | undefined;
}): JSX.Element => {
	const parts = () => {
		const shown: string[] = [];
		const from = props.row?.start_point;
		if (from) {
			shown.push(`from ${from}`);
		}
		if (props.dimension === "measures" && props.detail) {
			shown.push((props.detail as JsonMeasure).units);
		}
		const last = props.row?.last_report;
		if (last !== undefined) {
			shown.push(`last report ${shortDate(last)}`);
		} else if (props.row) {
			shown.push("never reported");
		}
		return shown;
	};
	return (
		<For each={parts()}>
			{(part, index) => (
				<>
					<Show when={index() > 0}>
						<span class="dotsep" aria-hidden="true">
							·
						</span>
					</Show>
					{part}
				</>
			)}
		</For>
	);
};
