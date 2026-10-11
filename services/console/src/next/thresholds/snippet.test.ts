import { describe, expect, test } from "vitest";
import { declarationSnippet } from "./snippet";

const PLACE = {
	project: "hashbrown",
	branch: "main",
	testbed: "ubuntu-latest",
};
const LATENCY = {
	measure: "latency",
	metric: "value",
	model: {
		test: "t_test",
		min_sample_size: 4,
		max_sample_size: 64,
		upper_boundary: 0.99,
	},
} as const;

describe("declarationSnippet", () => {
	// Kills a flag dropped, misordered, or left without its continuation, a model
	// field the threshold does not have, and the threshold's rows not flagged.
	test("declares the threshold's model exactly, as run flags", () => {
		const snippet = declarationSnippet(PLACE, LATENCY);
		expect(snippet.kind).toBe("flags");
		expect(snippet.text).toBe(
			[
				"export BENCHER_API_KEY=bencher_run_...",
				"bencher run \\",
				"  --project hashbrown \\",
				"  --branch main \\",
				"  --testbed ubuntu-latest \\",
				"  --adapter json \\",
				"  --threshold-measure latency \\",
				"  --threshold-metric value \\",
				"  --threshold-test t_test \\",
				"  --threshold-min-sample-size 4 \\",
				"  --threshold-max-sample-size 64 \\",
				"  --threshold-upper-boundary 0.99 \\",
				"  --file results.json",
			].join("\n"),
		);
		expect(
			snippet.code.filter(({ flag }) => flag).map(({ text }) => text),
		).toEqual([
			"  --threshold-measure latency \\",
			"  --threshold-metric value \\",
			"  --threshold-test t_test \\",
			"  --threshold-min-sample-size 4 \\",
			"  --threshold-max-sample-size 64 \\",
			"  --threshold-upper-boundary 0.99 \\",
		]);
	});

	// Kills the window printed as a duration, which the CLI refuses, a zero
	// boundary taken for none, and a lower boundary left out.
	test("passes the window in seconds and every boundary it has", () => {
		const { text } = declarationSnippet(PLACE, {
			...LATENCY,
			model: {
				test: "z_score",
				window: 7_776_000,
				lower_boundary: 0,
				upper_boundary: 0.95,
			},
		});
		expect(text).toContain("  --threshold-window 7776000 \\\n");
		expect(text).toContain("  --threshold-lower-boundary 0 \\\n");
		expect(text).toContain("  --threshold-upper-boundary 0.95 \\\n");
		expect(text).not.toContain("sample-size");
	});

	// Kills a parameters set passed unquoted, which the shell splits, or with keys out of the API's order.
	test("passes a one-set filter as quoted JSON", () => {
		const { text } = declarationSnippet(PLACE, {
			...LATENCY,
			parameters: [{ simd: "avx2", "10": true }],
		});
		expect(text).toContain(
			`  --threshold-parameters '{"10": true, "simd": "avx2"}' \\\n`,
		);
	});

	// Kills names the shell would split or expand, and a self-hosted run sent to Bencher Cloud.
	test("quotes what the shell would split and names a self-hosted API", () => {
		const { text } = declarationSnippet(
			{
				project: "hashbrown",
				branch: "Eustace's branch",
				testbed: "ubuntu latest",
				host: "http://localhost:61016",
			},
			LATENCY,
		);
		expect(text).toContain(`  --branch 'Eustace'\\''s branch' \\\n`);
		expect(text).toContain("  --testbed 'ubuntu latest' \\\n");
		expect(text).toContain("  --host http://localhost:61016 \\\n");
	});

	// Kills flags offered for a filter they cannot express, each run flag taking
	// one set, and the API's model identity and times carried into the payload.
	test("declares a filter of several sets as a payload entry", () => {
		const snippet = declarationSnippet(PLACE, {
			measure: "throughput",
			metric: "value",
			parameters: [{ n: 1 }, { n: 2 }],
			model: {
				test: "percentage",
				lower_boundary: 0.1,
				...{ uuid: "8f1e9d2c-0b7a-4c3e-9f6d-5a4b3c2d1e0f", created: 0 },
			},
		});
		expect(snippet.kind).toBe("payload");
		expect(JSON.parse(snippet.text)).toEqual({
			thresholds: {
				models: [
					{
						measure: "throughput",
						metric: "value",
						parameters: [{ n: 1 }, { n: 2 }],
						model: { test: "percentage", lower_boundary: 0.1 },
					},
				],
			},
		});
	});
});
