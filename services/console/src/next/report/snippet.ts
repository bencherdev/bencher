import type { Adapter } from "../../types/bencher";
import type { Guard } from "../plot/format";
import { commandOf, json, quote } from "./command";
import type { ParameterSet } from "./row";

/** A report line as the no-threshold sheet sees it; `measure` is the measure's slug. */
export interface SnippetLine {
	parameters: ParameterSet;
	measure: string;
	metric: string;
	guard?: Guard | undefined;
}

/** Where the run reports, and the adapter of the report the line is in; `host` is the API's URL on a self-hosted server. */
export interface SnippetPlace {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
	adapter: Adapter;
}

interface Snippet<L> {
	/** What Copy puts on the clipboard. */
	text: string;
	/** The same text by row, with the rows that declare the threshold flagged. */
	code: { text: string; flag: boolean }[];
	/** The report's lines this threshold would check. */
	checks: L[];
}

/** `bencher run` flags declaring a threshold for a line none checks: the exact one (null without parameters) and the simpler one. */
export const thresholdSnippets = <L extends SnippetLine>(
	place: SnippetPlace,
	line: L,
	lines: readonly L[],
): { exact: Snippet<L> | null; simple: Snippet<L> } => {
	const guard = guardFor(line, lines);
	const sameMetric = lines.filter(
		(other) => other.measure === line.measure && other.metric === line.metric,
	);
	const simple = {
		...snippet(place, line, guard, null),
		checks: sameMetric,
	};
	if (Object.keys(line.parameters).length === 0) {
		return { exact: null, simple };
	}
	return {
		exact: {
			...snippet(place, line, guard, line.parameters),
			checks: sameMetric.filter((other) =>
				isSubset(line.parameters, other.parameters),
			),
		},
		simple,
	};
};

// Guard the way the project already guards this measure; with no threshold on it, throughput against falling and the rest against rising.
export const guardFor = (
	line: SnippetLine,
	lines: readonly SnippetLine[],
): Guard =>
	lines.find((other) => other.measure === line.measure && other.guard)?.guard ??
	(line.measure === "throughput" ? "lower" : "upper");

const isSubset = (subset: ParameterSet, superset: ParameterSet) =>
	Object.entries(subset).every(([key, value]) => superset[key] === value);

const snippet = (
	place: SnippetPlace,
	line: SnippetLine,
	guard: Guard,
	parameters: ParameterSet | null,
) => {
	const row = (text: string, flag = false) => ({ text, flag });
	const rows = [
		row("export BENCHER_API_KEY=bencher_run_..."),
		row("bencher run"),
		row(`  --project ${quote(place.project)}`),
		...(place.host ? [row(`  --host ${quote(place.host)}`)] : []),
		row(`  --branch ${quote(place.branch)}`),
		row(`  --testbed ${quote(place.testbed)}`),
		row(`  --adapter ${cliAdapter(place.adapter)}`),
		row(`  --threshold-measure ${quote(line.measure)}`, true),
		row(`  --threshold-metric ${quote(line.metric)}`, true),
		...(parameters
			? [row(`  --threshold-parameters ${quote(json(parameters))}`, true)]
			: []),
		row("  --threshold-test t_test", true),
		row("  --threshold-max-sample-size 30", true),
		...(guard === "upper"
			? []
			: [row("  --threshold-lower-boundary 0.99", true)]),
		...(guard === "lower"
			? []
			: [row("  --threshold-upper-boundary 0.99", true)]),
		row("  --file results.json"),
	];
	return commandOf(rows);
};

// A language's adapter tries each of its harnesses, as `magic` does.
const LANGUAGE = /^(rust|cpp|go|java|c_sharp|js|python|ruby|shell|dart)$/;

/** The adapter as `bencher run --adapter` spells it, which takes no language and no format revision. */
export const cliAdapter = (adapter: Adapter): string =>
	LANGUAGE.test(adapter)
		? "magic"
		: adapter.replace(/_(v\d|json|stdout|text|xml)$/, "");
