import Sheet from "@bencherdev/ui/Sheet";
import type { QueryObserverResult } from "@tanstack/solid-query";
import type { Adapter, BmfVersion } from "../../types/bencher";
import { For, Match, Show, Switch } from "solid-js";
import Code from "./Code";
import { runHost, useProject } from "../project";
import { useQueryResult } from "../query";
import type { ReportLine } from "./lines";
import { countQuery } from "./query";
import { lineLabel } from "./row";
import { thresholdSnippets } from "./snippet";
import { thresholdSnippetV0 } from "./snippetV0";
import { formatCount } from "../plot/count";

interface ThresholdSheetProps {
	/** The line no threshold checks, while the sheet is open. */
	line: ReportLine | undefined;
	/** The report's lines loaded so far, which say how the project guards a measure. */
	lines: readonly ReportLine[];
	report: string;
	branch: string;
	testbed: string;
	/** The adapter of the report, which a version 0 snippet names. */
	adapter: Adapter;
	version: BmfVersion | undefined;
	/** Where the run reports, for a server other than Bencher Cloud. */
	host: string | undefined;
	onClose: () => void;
}

/** The `bencher run` flags that declare a threshold for a line none checks: exactly this line, then every line of its measure. */
const ThresholdSheet = (props: ThresholdSheetProps) => (
	<Sheet
		open={props.line !== undefined}
		onClose={props.onClose}
		title="No threshold checks this line"
		done="Close"
		side
	>
		<Show when={props.version !== undefined && props.line}>
			{(line) => <Snippets {...props} line={line()} />}
		</Show>
	</Sheet>
);

export default ThresholdSheet;

const Snippets = (props: ThresholdSheetProps & { line: ReportLine }) => {
	const { api, slug } = useProject();
	const snippets = () => {
		const place = {
			project: slug(),
			branch: props.branch,
			testbed: props.testbed,
			host: runHost(props.host),
		};
		const line = snippetLine(props.line);
		const lines = props.lines.map(snippetLine);
		return props.version === 0
			? {
					exact: null,
					simple: thresholdSnippetV0(
						{ ...place, adapter: props.adapter },
						line,
						lines,
					),
				}
			: thresholdSnippets(place, line, lines);
	};
	const label = () =>
		lineLabel(
			{
				benchmark: props.line.benchmark.name,
				parameters: props.line.parameters,
				measure: props.line.measure.name,
				metric: props.line.metric,
			},
			false,
		);
	const count = (parameters: boolean) =>
		useQueryResult(() =>
			countQuery(api, slug(), props.report, {
				measure: props.line.measure.uuid,
				metric: props.line.metric,
				parameters: parameters ? props.line.parameters : undefined,
			}),
		);
	const exact = count(true);
	const simple = count(false);
	const where = () => `${props.branch} · ${props.testbed}`;
	return (
		<>
			<div class="rp-id">
				<b>{props.line.benchmark.name}</b>
				<For each={label().tags}>
					{(tag) => <span class="lr-ptag">{tag}</span>}
				</For>
				<span class="muted sm">
					{props.line.measure.name}, metric {props.line.metric}, on {where()}
				</span>
			</div>
			<Show when={snippets().exact}>
				{(snippet) => (
					<section class="rp-sec" aria-label="Exactly this line">
						<div class="seclabel">Exactly this line</div>
						<p class="sm">
							<Count count={exact()} />
						</p>
						<Code
							name="the command for exactly this line"
							text={snippet().text}
							code={snippet().code}
						/>
					</section>
				)}
			</Show>
			<section class="rp-sec" aria-label="Every line of the measure">
				<div class="seclabel">Every line of the measure</div>
				<p class="sm">
					<Count count={simple()} />
				</p>
				<Code
					name="the command for every line of the measure"
					text={snippets().simple.text}
					code={snippets().simple.code}
				/>
			</section>
			<a class="lnk" href="/docs/explanation/thresholds/">
				How thresholds work
			</a>
		</>
	);
};

const snippetLine = (line: ReportLine) => ({
	parameters: line.parameters,
	measure: line.measure.slug,
	metric: line.metric,
	guard: line.guard,
});

const Count = (props: { count: QueryObserverResult<number> }) => (
	<Switch fallback={<span class="ui-skeleton" data-size="text" />}>
		<Match when={props.count.data !== undefined}>
			<b>
				{props.count.data === 1
					? "1 line in this report"
					: `${formatCount(props.count.data)} lines in this report`}
			</b>
		</Match>
		<Match when={props.count.isError}>
			the count did not load{" "}
			<button type="button" class="lnk" onClick={() => props.count.refetch()}>
				Retry
			</button>
		</Match>
	</Switch>
);
