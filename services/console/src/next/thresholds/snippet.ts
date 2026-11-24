import type { ParameterSet } from "../report/row";
import { json, quote } from "../report/snippet";
import type { ModelLike } from "./text";

/** A threshold as a run declares it, with a model in at least 1 slot: `measure` is the measure's slug. */
export interface Declaration {
	measure: string;
	metric: string;
	parameters?: readonly ParameterSet[] | undefined;
	model?: ModelLike | undefined;
	changepoint_model?: ModelLike | undefined;
}

/**
 * What declares this threshold as it is: `bencher run` flags, with a group for
 * the model of each slot, or, for a filter of several sets, which the flags
 * cannot carry, the report payload's entries.
 */
export const declarationSnippet = (
	place: Place,
	declaration: Declaration,
): { kind: "flags" | "payload"; text: string; code: Row[] } => {
	const sets = declaration.parameters ?? [];
	const models = [declaration.model, declaration.changepoint_model].filter(
		(model): model is ModelLike => model != null,
	);
	if (sets.length > 1) {
		const text = payload(declaration, models);
		return {
			kind: "payload",
			text,
			code: text.split("\n").map((line) => ({ text: line, flag: false })),
		};
	}
	const row = (text: string, flag = false) => ({ text, flag });
	// The CLI pairs a group's options by place, so a group without an option another group has takes `_`.
	const options = OPTIONS.filter(([, field]) =>
		models.some((model) => model[field] != null),
	);
	const rows = [
		row("export BENCHER_API_KEY=bencher_run_..."),
		row("bencher run"),
		row(`  --project ${quote(place.project)}`),
		...(place.host ? [row(`  --host ${quote(place.host)}`)] : []),
		row(`  --branch ${quote(place.branch)}`),
		row(`  --testbed ${quote(place.testbed)}`),
		row("  --adapter json"),
		...models.flatMap((model) => [
			row(`  --threshold-measure ${quote(declaration.measure)}`, true),
			row(`  --threshold-metric ${quote(declaration.metric)}`, true),
			...sets.map((set) =>
				row(`  --threshold-parameters ${quote(json(set))}`, true),
			),
			row(`  --threshold-test ${model.test}`, true),
			...options.map(([name, field]) =>
				row(`  --threshold-${name} ${model[field] ?? "_"}`, true),
			),
		]),
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

// The `--threshold-*` options a model may set, by the field each takes.
const OPTIONS = [
	["min-sample-size", "min_sample_size"],
	["max-sample-size", "max_sample_size"],
	["window", "window"],
	["lower-boundary", "lower_boundary"],
	["upper-boundary", "upper_boundary"],
	["significance", "significance"],
	["confirmations", "confirmations"],
] as const;

/** Where the run reports; `host` is the API's URL on a self-hosted server. */
interface Place {
	project: string;
	branch: string;
	testbed: string;
	host?: string | undefined;
}

type Row = { text: string; flag: boolean };

// Only the model's own fields: the API's model also carries its identity and times.
const payload = (
	{ measure, metric, parameters }: Declaration,
	models: readonly ModelLike[],
) =>
	JSON.stringify(
		{
			thresholds: {
				models: models.map(
					({
						test,
						min_sample_size,
						max_sample_size,
						window,
						lower_boundary,
						upper_boundary,
						significance,
						confirmations,
					}) => ({
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
							significance,
							confirmations,
						},
					}),
				),
			},
		},
		null,
		2,
	);
