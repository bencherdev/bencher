import Button from "@bencherdev/ui/Button";
import Segmented from "@bencherdev/ui/Segmented";
import Skeleton from "@bencherdev/ui/Skeleton";
import Table from "@bencherdev/ui/Table";
import { useQueryClient } from "@tanstack/solid-query";
import { For, type JSX, Show, createMemo, createSignal } from "solid-js";
import type {
	JsonBranch,
	JsonConsoleBranchRow,
	JsonConsoleRunSummary,
	JsonMeasure,
	JsonTestbed,
	JsonThreshold,
	JsonVariant,
} from "../../types/bencher";
import { PROJECTS_PATH, projectPath, runPath } from "../paths";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import { failureOf } from "../settings/failure";
import {
	type Detail,
	type ThresholdsOn,
	markVariantArchived,
	variantsQuery,
} from "./data";
import { archiveImpact } from "./impact";
import { Actions, Impact, focusRow } from "./RowTable";
import { modelText } from "../thresholds/text";
import { thresholdLinkName } from "./threshold";
import { shortDate, shortDay } from "./time";
import { parametersInUse, shownTotal, shownVariants, tagsOf } from "./variants";
import { formatCount } from "../plot/count";

export interface ThresholdsState {
	data: ThresholdsOn | undefined;
	failed: boolean;
	retry: () => void;
}

/** What every dimension carries: its slug, UUID, and when it was made. */
export const RecordCard = (props: { detail: Detail | undefined }) => (
	<Card id="dm-record" title="Record">
		<dl class="dm-kvs">
			<Kv k="slug">
				<span class="mono">{props.detail?.slug}</span>
			</Kv>
			<Kv k="uuid">
				<span class="mono dm-uuid">{props.detail?.uuid}</span>
			</Kv>
			<Kv k="created">
				{props.detail ? shortDay(props.detail.created, 0) : ""}
			</Kv>
		</dl>
	</Card>
);

export interface RunsState {
	data: JsonConsoleRunSummary[] | undefined;
	failed: boolean;
	retry: () => void;
}

export const BranchCards = (props: {
	slug: string;
	branch: JsonBranch | undefined;
	row: JsonConsoleBranchRow | null | undefined;
	runs: RunsState;
	thresholds: ThresholdsState;
}) => {
	const head = () => props.branch?.head;
	return (
		<>
			<Card
				id="dm-head"
				title="Head"
				aside={
					<Show when={head()}>
						{(shown) => (
							<span class="muted sm">since {shortDate(shown().created)}</span>
						)}
					</Show>
				}
			>
				<dl class="dm-kvs">
					<Kv k="version">
						<Show
							when={props.row?.hash ?? head()?.version?.hash}
							fallback={<span class="muted">none yet</span>}
						>
							{(hash) => <span class="mono">{shortHash(hash())}</span>}
						</Show>
					</Kv>
					<Kv k="start point">
						<Show
							when={head()?.start_point}
							fallback={<span class="muted">none</span>}
						>
							{(start) => (
								<>
									<a
										href={`${PROJECTS_PATH}/${props.slug}/branches/${start().branch}`}
									>
										{props.row?.start_point ?? "its start point"}
									</a>
									<span class="muted"> at </span>
									<span class="mono">
										{shortHash(start().version.hash) ??
											`number ${start().version.number}`}
									</span>
								</>
							)}
						</Show>
					</Kv>
				</dl>
			</Card>
			<RecentRuns
				slug={props.slug}
				on={{ key: "branch", of: props.branch }}
				state={props.runs}
			/>
			<ThresholdsCard
				slug={props.slug}
				one="branch"
				first={{ title: "Testbed", of: (t) => t.testbed.name }}
				second={{ title: "Measure", of: (t) => t.measure.name }}
				state={props.thresholds}
			/>
		</>
	);
};

/** A branch's or a testbed's newest runs, each opening its run, under a link to all of them. */
const RecentRuns = (props: {
	slug: string;
	on: { key: "branch" | "testbed"; of: JsonBranch | JsonTestbed | undefined };
	state: RunsState;
}) => {
	const branch = () => props.on.key === "branch";
	// Runs lists the runs of active branches and testbeds only.
	const all = () => {
		const shown = props.on.of;
		return shown && !shown.archived
			? `${projectPath(props.slug, "runs")}?${props.on.key}=${encodeURIComponent(shown.slug)}&window=all`
			: undefined;
	};
	return (
		<Section
			id="dm-runs"
			title="Recent runs"
			aside={
				<Show when={all()}>
					{(href) => (
						<a class="lnk" href={href()}>
							All runs on {props.on.of?.name}
						</a>
					)}
				</Show>
			}
		>
			<Show
				when={props.state.data}
				fallback={
					<Show
						when={props.state.failed}
						fallback={<Skeleton size="card" class="dm-card-skel" />}
					>
						<Failed what="The runs" retry={props.state.retry} />
					</Show>
				}
			>
				{(runs) => (
					<Show
						when={runs().length > 0}
						fallback={<p class="dm-none">No runs</p>}
					>
						<Table fold={533} class="dm-inner" aria-labelledby="dm-runs">
							<thead>
								<tr>
									<th scope="col">When</th>
									<Show when={!branch()}>
										<th scope="col">Branch</th>
									</Show>
									<th scope="col">Head</th>
									<th scope="col" data-align="end">
										Reports
									</th>
									<Show when={branch()}>
										<th scope="col">Testbed</th>
									</Show>
									<th scope="col" data-align="end">
										Alerts
									</th>
								</tr>
							</thead>
							<tbody>
								<For each={runs()}>
									{(run) => (
										<tr class="rowlink">
											<td data-fold="l1">
												<a
													class="dm-link rowlink-name"
													href={runPath(props.slug, run.uuid)}
													aria-label={`${shortDate(run.start_time)}, run ${runHead(run)} on ${run.branch.name}, ${run.reports} ${run.reports === 1 ? "report" : "reports"}`}
												>
													{shortDate(run.start_time)}
												</a>
											</td>
											<Show when={!branch()}>
												<td data-fold="l2" class="ui-table-name">
													{run.branch.name}
												</td>
											</Show>
											<td data-fold="hide" class="mono muted">
												{runHead(run)}
											</td>
											<td data-fold="hide" data-align="end">
												{run.reports}
											</td>
											<Show when={branch()}>
												<td data-fold="l2" class="ui-table-name">
													{run.testbeds[0]}
													<Show when={run.testbed_count > 1}>
														<span
															class="rn-more rowlink-own"
															title={allNames(run.testbeds, run.testbed_count)}
														>
															+{run.testbed_count - 1}
														</span>
													</Show>
												</td>
											</Show>
											<td data-fold="n1" data-align="end">
												<Show
													when={run.alerts.active}
													fallback={<span class="muted">0</span>}
												>
													{(active) => (
														<span class="dm-alerts">
															{formatCount(active())} active
														</span>
													)}
												</Show>
											</td>
										</tr>
									)}
								</For>
							</tbody>
						</Table>
					</Show>
				)}
			</Show>
		</Section>
	);
};

export const TestbedCards = (props: {
	slug: string;
	testbed: JsonTestbed | undefined;
	runs: RunsState;
	thresholds: ThresholdsState;
}) => {
	const spec = () => props.testbed?.spec;
	return (
		<>
			<Card id="dm-spec" title="Spec">
				<Show
					when={spec()}
					fallback={<p class="dm-none">{props.testbed ? "No spec" : ""}</p>}
				>
					{(shown) => (
						<dl class="dm-kvs">
							<Kv k="name">{shown().name}</Kv>
							<Kv k="slug">
								<span class="mono">{shown().slug}</span>
							</Kv>
							<Kv k="os">{shown().os}</Kv>
							<Kv k="architecture">{shown().architecture}</Kv>
							<Kv k="cpu">{shown().cpu} cores</Kv>
							<Kv k="memory">{bytes(shown().memory)}</Kv>
							<Kv k="disk">{bytes(shown().disk)}</Kv>
							<Show when={shown().sandbox}>
								{(sandbox) => <Kv k="sandbox">{sandbox()}</Kv>}
							</Show>
							<Kv k="network">{shown().network ? "on" : "off"}</Kv>
						</dl>
					)}
				</Show>
			</Card>
			<RecentRuns
				slug={props.slug}
				on={{ key: "testbed", of: props.testbed }}
				state={props.runs}
			/>
			<ThresholdsCard
				slug={props.slug}
				one="testbed"
				first={{ title: "Branch", of: (t) => t.branch.name }}
				second={{ title: "Measure", of: (t) => t.measure.name }}
				state={props.thresholds}
			/>
		</>
	);
};

const GIB = 1024 ** 3;

/** A size in bytes, in GiB when it is at least one. */
const bytes = (size: number) =>
	size >= GIB
		? `${Math.round((size / GIB) * 10) / 10} GiB`
		: `${formatCount(size)} bytes`;

export const MeasureCards = (props: {
	slug: string;
	measure: JsonMeasure | undefined;
	thresholds: ThresholdsState;
}) => (
	<>
		<Card id="dm-units" title="Units">
			<dl class="dm-kvs">
				<Kv k="units">{props.measure?.units}</Kv>
			</dl>
		</Card>
		<ThresholdsCard
			slug={props.slug}
			one="measure"
			first={{ title: "Branch", of: (t) => t.branch.name }}
			second={{ title: "Testbed", of: (t) => t.testbed.name }}
			state={props.thresholds}
		/>
	</>
);

/** A benchmark's parameters in use and its variants, each of which archives on its own. */
export const BenchmarkCards = (props: {
	benchmark: string;
	name: string | undefined;
	edit: boolean;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const [archived, setArchived] = createSignal(false);
	const active = useQueryResult(() =>
		variantsQuery(api, slug(), props.benchmark, false),
	);
	const archivedList = useQueryResult(() =>
		variantsQuery(api, slug(), props.benchmark, true),
	);
	const listed = () => (archived() ? archivedList() : active());
	const shown = createMemo(() => shownVariants(listed().data?.variants ?? []));
	const inUse = createMemo(() =>
		parametersInUse(
			shownVariants(active().data?.variants ?? []).filter(
				(variant) => !variant.archived,
			),
		),
	);
	const [arming, setArming] = createSignal<string>();
	const [failure, setFailure] = createSignal("");
	const label = (variant: JsonVariant) =>
		[props.name ?? props.benchmark, ...tagsOf(variant)].join(" ");
	// Each variant's changes reach the API in the order they were made, and a
	// refusal returns the variant to what the API last confirmed.
	const queue = new Map<string, Promise<unknown>>();
	const confirmed = new Map<string, string | undefined>();
	const change = async (variant: JsonVariant, archive: boolean) => {
		if (!confirmed.has(variant.uuid)) {
			confirmed.set(variant.uuid, variant.archived ?? undefined);
		}
		setArming(undefined);
		setFailure("");
		const project = slug();
		// A read in flight would land over the change.
		await client.cancelQueries({
			queryKey: ["console", "benchmarks", project, "variants", props.benchmark],
		});
		const at = archive ? new Date().toISOString() : undefined;
		markVariantArchived(client, project, props.benchmark, variant.uuid, at);
		const path = `/v0/projects/${encodeURIComponent(project)}/benchmarks/${encodeURIComponent(props.benchmark)}/variants/${variant.uuid}`;
		const sent: Promise<unknown> = (
			queue.get(variant.uuid) ?? Promise.resolve()
		)
			.catch(() => {})
			.then(async () => {
				try {
					await api.send("PATCH", path, { archived: archive });
					confirmed.set(variant.uuid, at);
					client.invalidateQueries({
						queryKey: [
							"console",
							"benchmarks",
							project,
							"variants",
							props.benchmark,
							!archived(),
						],
					});
				} catch (error) {
					// A later change on the variant is on its way and says what it shows.
					if (queue.get(variant.uuid) === sent) {
						markVariantArchived(
							client,
							project,
							props.benchmark,
							variant.uuid,
							confirmed.get(variant.uuid),
						);
					}
					setFailure(
						failureOf(
							error,
							`Bencher did not ${archive ? "archive" : "unarchive"} the variant`,
						),
					);
				}
			});
		queue.set(variant.uuid, sent);
	};
	let body: HTMLTableSectionElement | undefined;
	// The counts are of the variants the table shows, each in its shown state.
	const activeCount = () => {
		const list = active().data;
		return list && shownTotal(list.variants, list.total);
	};
	const archivedCount = () => {
		const list = archivedList().data;
		return list && shownTotal(list.variants, list.total);
	};
	return (
		<>
			<Card id="dm-params" title="Parameters in use">
				<Show
					when={active().data}
					fallback={<Skeleton size="card" class="dm-card-skel" />}
				>
					<Show
						when={inUse().length > 0}
						fallback={<p class="dm-none">No parameters</p>}
					>
						<div class="dm-params">
							<For each={inUse()}>
								{(parameter) => (
									<div class="dm-param">
										<b class="mono sm">{parameter.key}</b>
										<span class="dm-values">
											<For each={parameter.values}>
												{(value) => (
													<span class="dm-value">
														<span class="dm-tag">{value.value}</span>
														{value.variants === 1
															? "1 variant"
															: `${formatCount(value.variants)} variants`}
													</span>
												)}
											</For>
										</span>
									</div>
								)}
							</For>
						</div>
					</Show>
				</Show>
			</Card>
			<Section
				id="dm-variants"
				title="Variants"
				aside={
					<Segmented
						name="dm-variant-status"
						aria-label="Variant status"
						value={archived() ? "archived" : "active"}
						onChange={(status) => {
							setArming(undefined);
							setArchived(status === "archived");
						}}
						options={[
							{
								value: "active",
								label: (
									<>
										Active{" "}
										<span class="count">{formatCount(activeCount())}</span>
									</>
								),
							},
							{
								value: "archived",
								label: (
									<>
										Archived{" "}
										<span class="count">{formatCount(archivedCount())}</span>
									</>
								),
							},
						]}
					/>
				}
			>
				<Show when={failure()}>
					<p class="ferror dm-inset" role="alert">
						{failure()}
					</p>
				</Show>
				<Show
					when={listed().data}
					fallback={<Skeleton size="card" class="dm-card-skel" />}
				>
					<Show
						when={shown().length > 0}
						fallback={<p class="dm-none">No variants</p>}
					>
						<Table
							fold={215}
							class="dm-inner"
							aria-label={archived() ? "Archived variants" : "Active variants"}
						>
							<thead>
								<tr>
									<th scope="col">Variant</th>
									<th scope="col">
										<span class="sr-only">Actions</span>
									</th>
								</tr>
							</thead>
							<tbody ref={body}>
								<For each={shown()}>
									{(variant) => (
										<>
											<tr
												classList={{
													"dm-dim": !archived() && Boolean(variant.archived),
												}}
												data-uuid={variant.uuid}
											>
												<td data-fold="l1" class="ui-table-name">
													<span class="dm-tags">
														<For each={tagsOf(variant)}>
															{(tag) => <span class="dm-tag">{tag}</span>}
														</For>
														<Show when={tagsOf(variant).length === 0}>
															<span class="muted">no parameters</span>
														</Show>
													</span>
												</td>
												<td data-fold="act" class="dm-act">
													<Actions
														name={label(variant)}
														archived={variant.archived}
														listArchived={archived()}
														changed={Boolean(variant.archived) !== archived()}
														edit={props.edit}
														arming={arming() === variant.uuid}
														onArm={() =>
															setArming(
																arming() === variant.uuid
																	? undefined
																	: variant.uuid,
															)
														}
														onChange={(archive) => {
															change(variant, archive);
															focusRow(body, variant.uuid);
														}}
													/>
												</td>
											</tr>
											<Show when={arming() === variant.uuid}>
												<tr class="dm-impact-row">
													<td colSpan={2} data-fold="full">
														<Impact
															name={label(variant)}
															text={archiveImpact(undefined)}
															confirm="Archive variant"
															onConfirm={() => {
																change(variant, true);
																focusRow(body, variant.uuid);
															}}
															onCancel={() => {
																setArming(undefined);
																focusRow(body, variant.uuid);
															}}
														/>
													</td>
												</tr>
											</Show>
										</>
									)}
								</For>
							</tbody>
						</Table>
					</Show>
				</Show>
			</Section>
		</>
	);
};

/** A card of facts. */
const Card = (props: {
	id: string;
	title: string;
	aside?: JSX.Element;
	children: JSX.Element;
}) => (
	<section class="setcard" aria-labelledby={props.id}>
		<div class="setcard-head">
			<h2 class="seclabel" id={props.id}>
				{props.title}
			</h2>
			<span class="spacer" />
			{props.aside}
		</div>
		<div class="setcard-body dm-card-body">{props.children}</div>
	</section>
);

/** A titled table under its title row. */
const Section = (props: {
	id: string;
	title: string;
	aside?: JSX.Element;
	children: JSX.Element;
}) => (
	<section class="dm-section" aria-labelledby={props.id}>
		<div class="dm-section-head">
			<h2 class="seclabel" id={props.id}>
				{props.title}
			</h2>
			<span class="spacer" />
			{props.aside}
		</div>
		{props.children}
	</section>
);

const Kv = (props: { k: string; children: JSX.Element }) => (
	<div class="kv">
		<dt>{props.k}</dt>
		<dd>{props.children}</dd>
	</div>
);

/**
 * The thresholds on a branch, testbed, or measure, each opening its own page.
 * `first` and `second` name the threshold's other two dimensions.
 */
const ThresholdsCard = (props: {
	slug: string;
	one: string;
	first: { title: string; of: (threshold: JsonThreshold) => string };
	second: { title: string; of: (threshold: JsonThreshold) => string };
	state: ThresholdsState;
}) => {
	const total = () => props.state.data?.total;
	return (
		<Section
			id="dm-thresholds"
			title="Thresholds"
			aside={
				<Show when={total() !== undefined}>
					<span class="muted sm">
						{formatCount(total())} on this {props.one}
					</span>
				</Show>
			}
		>
			<Show
				when={props.state.data}
				fallback={
					<Show
						when={props.state.failed}
						fallback={<Skeleton size="card" class="dm-card-skel" />}
					>
						<Failed what="The thresholds" retry={props.state.retry} />
					</Show>
				}
			>
				{(on) => (
					<Show
						when={on().thresholds.length > 0}
						fallback={<p class="dm-none">No thresholds</p>}
					>
						<Table fold={615} class="dm-inner" aria-labelledby="dm-thresholds">
							<thead>
								<tr>
									<th scope="col">{props.first.title}</th>
									<th scope="col">{props.second.title}</th>
									<th scope="col">Metric</th>
									<th scope="col">Parameters</th>
									<th scope="col">Model</th>
								</tr>
							</thead>
							<tbody>
								<For each={on().thresholds}>
									{(threshold) => (
										<tr class="rowlink">
											<td data-fold="l1" class="ui-table-name">
												<a
													class="dm-link rowlink-name"
													href={thresholdPath(props.slug, threshold.uuid)}
													aria-label={thresholdLinkName(
														props.first.of(threshold),
														threshold,
													)}
												>
													{props.first.of(threshold)}
												</a>
											</td>
											<td data-fold="l2" class="ui-table-name">
												{props.second.of(threshold)}
											</td>
											<td data-fold="hide" class="mono">
												{threshold.metric ?? "value"}
											</td>
											<td data-fold="hide">
												<Sets threshold={threshold} />
											</td>
											<td
												data-fold="hide"
												class="mono ui-table-name"
												title={modelText(threshold.model)}
											>
												{modelText(threshold.model)}
											</td>
										</tr>
									)}
								</For>
							</tbody>
						</Table>
					</Show>
				)}
			</Show>
		</Section>
	);
};

const thresholdPath = (slug: string, uuid: string) =>
	`${PROJECTS_PATH}/${slug}/thresholds/${uuid}`;

/** The parameters filter as tags, a set per row, or every variant. */
const Sets = (props: { threshold: JsonThreshold }) => (
	<Show
		when={props.threshold.parameters?.length}
		fallback={<span class="muted">every variant</span>}
	>
		<span class="dm-sets">
			<For each={props.threshold.parameters}>
				{(set, index) => (
					<>
						<Show when={index() > 0}>
							<span class="dm-or">or</span>
						</Show>
						<span class="dm-tags">
							<For each={Object.keys(set).sort()}>
								{(key) => (
									<span class="dm-tag">
										{key}={String(set[key])}
									</span>
								)}
							</For>
						</span>
					</>
				)}
			</For>
		</span>
	</Show>
);

const Failed = (props: { what: string; retry: () => void }) => (
	<div class="dm-failed" role="alert">
		<span>{props.what} did not load</span>
		<Button size="sm" onClick={() => props.retry()}>
			Retry
		</Button>
	</div>
);

const shortHash = (hash: string | undefined) => hash?.slice(0, 7);

const runHead = (run: JsonConsoleRunSummary) =>
	shortHash(run.hash) ?? `v${run.number}`;

const allNames = (names: readonly string[], count: number) =>
	count > names.length
		? `${names.join(", ")}, and ${formatCount(count - names.length)} more`
		: names.join(", ");
