import { useQueryClient } from "@tanstack/solid-query";
import {
	type Accessor,
	For,
	Index,
	Show,
	createMemo,
	mapArray,
} from "solid-js";
import type { JsonConsolePerf } from "../../types/bencher";
import { useProject } from "../project";
import { useQueryResult } from "../query";
import {
	type ExploreQuery,
	MAX_ENTRIES,
	type Parameters,
	type ParameterValue,
} from "../query/query";
import Box, {
	type BoxValue,
	type Choices,
	Cross,
	type Option,
	choicesOf,
} from "./Box";
import { exactFirst, metricNames, withValue, withoutValue } from "./edit";
import { measuresLayout } from "./layout";
import { matchedVariants, parameterRows } from "./parameters";
import {
	type Dimension,
	nameQuery,
	searchQuery,
	variantsQuery,
} from "./queries";
import { formatCount } from "../plot/count";
import Tag from "../tag";

const plural = (count: number, one: string, many: string) =>
	`${formatCount(count)} ${count === 1 ? one : many}`;

const tag = ([key, value]: [string, ParameterValue]) =>
	`${key}=${String(value)}`;

const tagsOf = (set: Parameters) => Object.entries(set).map(tag);

const BOXES: { dimension: Dimension; title: string; noun: string }[] = [
	{ dimension: "branches", title: "Branches", noun: "branch" },
	{ dimension: "testbeds", title: "Testbeds", noun: "testbed" },
	{ dimension: "benchmarks", title: "Benchmarks", noun: "benchmark" },
];

/** The six boxes, in the ruled order. */
const Editor = (props: {
	query: ExploreQuery;
	/** The answer for the query shown, which names what it drew. */
	perf: JsonConsolePerf | undefined;
	/** The last answer the plot had, which still names values while the query cannot draw. */
	last?: JsonConsolePerf | undefined;
	/** The plot query has answered for the query shown. */
	settled: boolean;
	readOnly?: boolean | undefined;
	/** The project is on BMF version 0, whose variants have no parameters to filter by. */
	versionZero?: boolean | undefined;
	onQuery: (query: ExploreQuery) => void;
	/** The reader reached for a benchmark: the plot is likely next. */
	onBenchmarks?: (() => void) | undefined;
}) => {
	const { api, slug } = useProject();
	const client = useQueryClient();
	const uuids = (dimension: Dimension): readonly string[] =>
		dimension === "branches" || dimension === "testbeds"
			? props.query[dimension].map(({ uuid }) => uuid)
			: props.query[dimension];
	const remember = (dimension: Dimension, option: Option) =>
		client.setQueryData(
			nameQuery(api, slug(), dimension, option.value).queryKey,
			{
				name: option.label,
				...(option.detail === undefined ? {} : { units: option.detail }),
			},
		);
	const search =
		(dimension: Dimension) =>
		(text: () => string): (() => Choices) => {
			const result = useQueryResult(() =>
				searchQuery(api, slug(), dimension, text()),
			);
			return () => {
				const chosen = new Set(uuids(dimension));
				const rows = result().data?.rows ?? [];
				return {
					options: rows
						.filter(({ uuid }) => !chosen.has(uuid))
						.map(({ uuid, name, units }) => ({
							value: uuid,
							label: name,
							detail: units,
						})),
					more: Math.max(0, (result().data?.total ?? 0) - rows.length),
				};
			};
		};
	const add = (dimension: Dimension, option: Option) => {
		remember(dimension, option);
		props.onQuery(withValue(props.query, dimension, option.value));
	};

	const measureLabels = useLabels(
		"measures",
		() => props.query.measures,
		props,
	);
	const measures = () => props.query.measures.length;
	const layout = () => measuresLayout(measures(), props.query.layout);
	return (
		<div class="ex-editor">
			<For each={BOXES}>
				{({ dimension, title, noun }) => {
					const labels = useLabels(dimension, () => uuids(dimension), props);
					return (
						<Box
							title={title}
							noun={noun}
							values={labels()}
							readOnly={props.readOnly}
							full={uuids(dimension).length >= MAX_ENTRIES}
							onRemove={(index) =>
								props.onQuery(withoutValue(props.query, dimension, index))
							}
							options={search(dimension)}
							onAdd={(option) => add(dimension, option)}
							onOpen={
								dimension === "benchmarks" ? props.onBenchmarks : undefined
							}
						/>
					);
				}}
			</For>
			<Sets
				query={props.query}
				perf={props.perf}
				readOnly={props.readOnly}
				versionZero={props.versionZero}
				onQuery={props.onQuery}
			/>
			<Box
				title="Measures"
				noun="measure"
				values={measureLabels()}
				twoLine
				hint={
					layout().control === "none"
						? undefined
						: layout().layout === "stacked"
							? "stacked"
							: "two y axes"
				}
				readOnly={props.readOnly}
				full={measures() >= MAX_ENTRIES}
				onRemove={(index) =>
					props.onQuery(withoutValue(props.query, "measures", index))
				}
				options={search("measures")}
				onAdd={(option) => add("measures", option)}
			/>
			<Box
				title="Metrics"
				noun="metric"
				values={props.query.metrics.map((label) => ({ label }))}
				readOnly={props.readOnly}
				full={props.query.metrics.length >= MAX_ENTRIES}
				onRemove={(index) =>
					props.onQuery(withoutValue(props.query, "metrics", index))
				}
				options={(text) => () => {
					const chosen = new Set(props.query.metrics);
					const lower = text().toLowerCase();
					return choicesOf(
						exactFirst(
							(props.perf ? metricNames(props.perf) : []).filter(
								(name) =>
									!chosen.has(name) && name.toLowerCase().includes(lower),
							),
							text(),
							(name) => name,
						).map((name) => ({ value: name, label: name })),
					);
				}}
				empty="No other metrics"
				onAdd={(option) =>
					props.onQuery(withValue(props.query, "metrics", option.value))
				}
			>
				<Show when={props.query.metrics.length === 0}>
					<p class="ex-all">every metric</p>
				</Show>
			</Box>
		</div>
	);
};

export default Editor;

/** Each value's name: from what the plot drew, from a pick, or asked for once the plot has answered. */
const useLabels = (
	dimension: Dimension,
	uuids: Accessor<readonly string[]>,
	props: {
		perf: JsonConsolePerf | undefined;
		last?: JsonConsolePerf | undefined;
		settled: boolean;
	},
): Accessor<BoxValue[]> => {
	const { api, slug } = useProject();
	const drawn = createMemo(() => {
		const perf = props.perf ?? props.last;
		const table: { uuid: string; name: string; units?: string }[] = perf
			? perf[dimension]
			: [];
		return new Map(table.map((entry) => [entry.uuid, entry]));
	});
	const labels = createMemo(
		mapArray(uuids, (uuid) => {
			const fetched = useQueryResult(() => ({
				...nameQuery(api, slug(), dimension, uuid),
				enabled: props.settled && !drawn().has(uuid),
			}));
			return (): BoxValue => {
				const named = drawn().get(uuid) ?? fetched().data;
				return {
					label: named?.name ?? uuid,
					detail: named?.units,
				};
			};
		}),
	);
	return () => labels().map((label) => label());
};

/** The parameters box: sets of tags, each counting the variants it matches. */
const Sets = (props: {
	query: ExploreQuery;
	perf: JsonConsolePerf | undefined;
	readOnly?: boolean | undefined;
	versionZero?: boolean | undefined;
	onQuery: (query: ExploreQuery) => void;
}) => {
	const { api, slug } = useProject();
	// What the plot drew holds every variant a set matches, until the line cap cuts lines.
	const drawn = createMemo(() =>
		(props.perf?.variants ?? []).map(({ parameters }) => parameters),
	);
	const rows = createMemo(() => parameterRows(props.query.sets, drawn()));
	const hint = () =>
		props.query.benchmarks.length > 0
			? plural(
					matchedVariants(props.query.sets, drawn()),
					"variant",
					"variants",
				)
			: undefined;
	return (
		<Box
			title="Parameters"
			noun="set"
			values={[]}
			rows={props.query.sets.length}
			hint={hint()}
			readOnly={props.readOnly}
			full={props.query.sets.length >= MAX_ENTRIES}
			locked={
				props.versionZero
					? "Filtering by parameters needs a version 1 project."
					: undefined
			}
			onRemove={() => {}}
			options={(text) => {
				// The chosen benchmarks' every variant, read only once the reader looks.
				const all = createMemo(
					mapArray(
						() => props.query.benchmarks,
						(benchmark) =>
							useQueryResult(() => variantsQuery(api, slug(), benchmark)),
					),
				);
				return () => {
					const loaded = all().map((result) => result().data);
					const lists = loaded.every((list) => list !== undefined)
						? loaded
						: [drawn()];
					const singles = new Set(
						props.query.sets
							.filter((set) => Object.keys(set).length === 1)
							.flatMap(tagsOf),
					);
					// Each tag's count, and the position of its most recent variant in any list.
					const counts = new Map<
						string,
						[string, ParameterValue, number, number]
					>();
					for (const list of lists) {
						list.forEach((variant, rank) => {
							for (const entry of Object.entries(variant)) {
								const label = tag(entry);
								const [key, value] = entry;
								const [, , count = 0, best = rank] = counts.get(label) ?? [];
								counts.set(label, [
									key,
									value,
									count + 1,
									Math.min(best, rank),
								]);
							}
						});
					}
					const lower = text().toLowerCase();
					return choicesOf(
						exactFirst(
							[...counts.entries()]
								.filter(
									([label]) =>
										!singles.has(label) && label.toLowerCase().includes(lower),
								)
								.sort(([, [, , , a]], [, [, , , b]]) => a - b),
							text(),
							([label]) => label,
						).map(([label, [key, value, count]]) => ({
							value: JSON.stringify({ [key]: value }),
							label,
							detail: plural(count, "variant", "variants"),
						})),
					);
				};
			}}
			empty="No parameters"
			onAdd={(option) =>
				props.onQuery(withValue(props.query, "sets", JSON.parse(option.value)))
			}
		>
			<Index
				each={props.query.sets}
				fallback={<p class="ex-all">every variant</p>}
			>
				{(set, index) => {
					const row = () => rows()[index];
					const name = () => tagsOf(set()).join(" ");
					const covering = () => {
						const by = row()?.coveredBy;
						return by === undefined ? undefined : props.query.sets[by];
					};
					return (
						<>
							<Show when={index > 0}>
								<div class="ex-or" aria-hidden="true">
									or
								</div>
							</Show>
							<div
								class="ex-set"
								classList={{ "ex-covered": covering() !== undefined }}
							>
								<span class="ex-tags">
									<For each={tagsOf(set())}>
										{(text) => <Tag class="ex-tag" tag={text} tail reveal />}
									</For>
								</span>
								<span
									class="ex-pc"
									classList={{ "ex-zero": covering() !== undefined }}
								>
									{covering()
										? "adds 0 variants"
										: plural(row()?.variants ?? 0, "variant", "variants")}
								</span>
								<Show when={!props.readOnly}>
									<button
										type="button"
										class="ex-x"
										aria-label={`Remove set ${name()}`}
										onClick={() =>
											props.onQuery(withoutValue(props.query, "sets", index))
										}
									>
										<Cross />
									</button>
								</Show>
							</div>
						</>
					);
				}}
			</Index>
		</Box>
	);
};
