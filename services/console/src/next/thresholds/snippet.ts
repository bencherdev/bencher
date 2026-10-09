import type { ParameterSet } from "../report/row";
import { json, quote } from "../report/snippet";
import type { ModelLike } from "./model";

/** A threshold as a run declares it: `measure` is the measure's slug. */
export interface Declaration {
	measure: string;
	metric: string;
	parameters?: readonly ParameterSet[] | undefined;
	model: ModelLike;
}

/**
 * What declares this threshold as it is: `bencher run` flags, or, for a filter
 * of several sets, which the flags cannot carry, the report payload's entry.
 */
export const declarationSnippet = (
	place: Place,
	declaration: Declaration,
): { kind: "flags" | "payload"; text: string; code: Row[] } => {
	const sets = declaration.parameters ?? [];
	if (sets.length > 1) {
		const text = payload(declaration);
		return {
			kind: "payload",
			text,
			code: text.split("\n").map((line) => ({ text: line, flag: false })),
		};
	}
	const row = (text: string, flag = false) => ({ text, flag });
	const { model } = declaration;
	const option = (name: string, value: number | undefined) =>
		value === undefined ? [] : [row(`  --threshold-${name} ${value}`, true)];
	const rows = [
		row("export BENCHER_API_KEY=bencher_run_..."),
		row("bencher run"),
		row(`  --project ${quote(place.project)}`),
		...(place.host ? [row(`  --host ${quote(place.host)}`)] : []),
		row(`  --branch ${quote(place.branch)}`),
		row(`  --testbed ${quote(place.testbed)}`),
		row("  --adapter json"),
		row(`  --threshold-measure ${quote(declaration.measure)}`, true),
		row(`  --threshold-metric ${quote(declaration.metric)}`, true),
		...sets.map((set) =>
			row(`  --threshold-parameters ${quote(json(set))}`, true),
		),
		row(`  --threshold-test ${model.test}`, true),
		...option("min-sample-size", model.min_sample_size),
		...option("max-sample-size", model.max_sample_size),
		...option("window", model.window),
		...option("lower-boundary", model.lower_boundary),
		...option("upper-boundary", model.upper_boundary),
		row("  --file results.json"),
	];
	const code = rows.map(({ text, flag }, index) => ({
		text: index === 0 || index === rows.length - 1 ? text : `${text} \\`,
		flag,
	}));
	return {
		kind: "flags",
		text: code.map(({ text }) => text).join("\n"),
		code,
	};
};

/** Where the run reports; `host` is the API's URL on a self-hosted server. */
interface Place {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
}

type Row = { text: string; flag: boolean };

// Only the model's own fields: the API's model also carries its identity and times.
const payload = ({ measure, metric, parameters, model }: Declaration) => {
	const {
		test,
		min_sample_size,
		max_sample_size,
		window,
		lower_boundary,
		upper_boundary,
	} = model;
	return JSON.stringify(
		{
			thresholds: {
				models: [
					{
						measure,
						metric,
						parameters,
						model: {
							test,
							min_sample_size,
							max_sample_size,
							window,
							lower_boundary,
							upper_boundary,
						},
					},
				],
			},
		},
		null,
		2,
	);
};
