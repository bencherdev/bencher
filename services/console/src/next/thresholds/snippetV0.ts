import { commandOf, quote } from "../report/snippet";
import type { ModelLike } from "./text";

/** Where the run reports; `host` is the API's URL on a self-hosted server. */
interface Place {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
}

/**
 * The `bencher run` flags that declare a version 0 threshold as it is. A
 * threshold page has no single report to name an adapter from, so the run
 * takes the CLI's default.
 */
export const declarationSnippetV0 = (
	place: Place,
	declaration: { measure: string; model: ModelLike },
) => {
	const row = (text: string, flag = false) => ({ text, flag });
	const { model } = declaration;
	const option = (name: string, value: number | null | undefined) =>
		value == null ? [] : [row(`  --threshold-${name} ${value}`, true)];
	return commandOf([
		row("export BENCHER_API_KEY=bencher_run_..."),
		row("bencher run"),
		row(`  --project ${quote(place.project)}`),
		...(place.host ? [row(`  --host ${quote(place.host)}`)] : []),
		row(`  --branch ${quote(place.branch)}`),
		row(`  --testbed ${quote(place.testbed)}`),
		row(`  --threshold-measure ${quote(declaration.measure)}`, true),
		row(`  --threshold-test ${model.test}`, true),
		...option("min-sample-size", model.min_sample_size),
		...option("max-sample-size", model.max_sample_size),
		...option("window", model.window),
		...option("lower-boundary", model.lower_boundary),
		...option("upper-boundary", model.upper_boundary),
		row("  --file results.json"),
	]);
};
