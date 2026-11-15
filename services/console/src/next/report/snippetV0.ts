import { commandOf, quote } from "./command";
import {
	type SnippetLine,
	type SnippetPlace,
	cliAdapter,
	guardFor,
} from "./snippet";

/** On a version 0 project a threshold checks `value`, so only a `value` line can declare one. */
export const declaresThresholdV0 = (line: Pick<SnippetLine, "metric">) =>
	line.metric === "value";

/** The `bencher run` flags that declare a version 0 threshold on the line's measure, and the report's lines it would check. */
export const thresholdSnippetV0 = <L extends SnippetLine>(
	place: SnippetPlace,
	line: L,
	lines: readonly L[],
) => {
	const guard = guardFor(line, lines);
	const row = (text: string, flag = false) => ({ text, flag });
	return {
		...commandOf([
			row("export BENCHER_API_KEY=bencher_run_..."),
			row("bencher run"),
			row(`  --project ${quote(place.project)}`),
			...(place.host ? [row(`  --host ${quote(place.host)}`)] : []),
			row(`  --branch ${quote(place.branch)}`),
			row(`  --testbed ${quote(place.testbed)}`),
			row(`  --adapter ${cliAdapter(place.adapter)}`),
			row(`  --threshold-measure ${quote(line.measure)}`, true),
			row("  --threshold-test t_test", true),
			row("  --threshold-max-sample-size 30", true),
			...(guard === "upper"
				? []
				: [row("  --threshold-lower-boundary 0.99", true)]),
			...(guard === "lower"
				? []
				: [row("  --threshold-upper-boundary 0.99", true)]),
			row("  --file results.json"),
		]),
		checks: lines.filter(
			(other) => other.measure === line.measure && declaresThresholdV0(other),
		),
	};
};
