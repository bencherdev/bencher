import type { Adapter } from "../../types/bencher";
import { type SnippetLine, commandOf, guardFor, quote } from "./snippet";

/** Where the run reports, and the adapter of the report the line is in. */
interface SnippetPlace {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
	adapter: Adapter;
}

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

// A language's adapter tries each of its harnesses, as `magic` does.
const LANGUAGE = /^(rust|cpp|go|java|c_sharp|js|python|ruby|shell|dart)$/;

/** The adapter as `bencher run --adapter` spells it, which takes no language and no format revision. */
export const cliAdapter = (adapter: Adapter): string =>
	LANGUAGE.test(adapter)
		? "magic"
		: adapter.replace(/_(v\d|json|stdout|text)$/, "");
