import type { Guard } from "../plot/format";
import { type ParameterSet, parameterEntries } from "./row";

/** A report line as the no-threshold sheet sees it; `measure` is the measure's slug. */
export interface SnippetLine {
	parameters: ParameterSet;
	measure: string;
	metric: string;
	guard?: Guard | undefined;
}

/** Where the run reports; `host` is the API's URL on a self-hosted server. */
interface SnippetPlace {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
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
const guardFor = (line: SnippetLine, lines: readonly SnippetLine[]): Guard =>
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
		row("  --adapter json"),
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
	const code = rows.map(({ text, flag }, index) => ({
		text: index === 0 || index === rows.length - 1 ? text : `${text} \\`,
		flag,
	}));
	return { text: code.map(({ text }) => text).join("\n"), code };
};

// Spaced for reading, with keys in the API's order.
export const json = (parameters: ParameterSet) =>
	`{${parameterEntries(parameters)
		.map(([key, value]) => `${JSON.stringify(key)}: ${JSON.stringify(value)}`)
		.join(", ")}}`;

const SHELL_SAFE = /^[A-Za-z0-9_@%+=:,./-]+$/;

export const quote = (value: string) =>
	SHELL_SAFE.test(value) ? value : `'${value.replaceAll("'", `'\\''`)}'`;
