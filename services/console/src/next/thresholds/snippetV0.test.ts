import { execFileSync } from "node:child_process";
import { describe, expect, test } from "vitest";
import { declarationSnippetV0 } from "./snippetV0";

const PLACE = {
	project: "crinkle-cut",
	branch: "main",
	testbed: "ubuntu-latest",
};

/** The argv `bencher` receives when a shell runs the snippet. */
const argv = (text: string): string[] =>
	execFileSync("sh", ["-c", `bencher() { printf '%s\\0' "$@"; }\n${text}`], {
		encoding: "utf8",
	})
		.split("\0")
		.slice(0, -1);

describe("declarationSnippetV0", () => {
	// Kills the version 1 flags a version 0 project refuses with a 400, an
	// adapter named where no single report says which (the run's default is
	// `magic`), a model field the threshold does not have, and a dropped flag.
	test("declares the measure's threshold with plain flags and no adapter", () => {
		const snippet = declarationSnippetV0(PLACE, {
			measure: "latency",
			model: {
				test: "t_test",
				min_sample_size: 4,
				max_sample_size: 64,
				upper_boundary: 0.99,
			},
		});
		expect(argv(snippet.text)).toEqual([
			"run",
			"--project",
			"crinkle-cut",
			"--branch",
			"main",
			"--testbed",
			"ubuntu-latest",
			"--threshold-measure",
			"latency",
			"--threshold-test",
			"t_test",
			"--threshold-min-sample-size",
			"4",
			"--threshold-max-sample-size",
			"64",
			"--threshold-upper-boundary",
			"0.99",
			"--file",
			"results.json",
		]);
		expect(snippet.code.map(({ text }) => text).join("\n")).toBe(snippet.text);
		expect(
			snippet.code.filter(({ flag }) => flag).map(({ text }) => text.trim()),
		).toEqual([
			"--threshold-measure latency \\",
			"--threshold-test t_test \\",
			"--threshold-min-sample-size 4 \\",
			"--threshold-max-sample-size 64 \\",
			"--threshold-upper-boundary 0.99 \\",
		]);
	});

	// Kills a window or a lower boundary dropped, and a self-hosted run sent
	// to Bencher Cloud.
	test("carries every model field and the host", () => {
		const host = "https://bencher.example.com:6610";
		const args = argv(
			declarationSnippetV0(
				{ ...PLACE, host },
				{
					measure: "throughput",
					model: { test: "percentage", window: 86_400, lower_boundary: 0.1 },
				},
			).text,
		);
		expect(args.slice(1, 5)).toEqual([
			"--project",
			"crinkle-cut",
			"--host",
			host,
		]);
		expect(args).toEqual(
			expect.arrayContaining([
				"--threshold-window",
				"86400",
				"--threshold-lower-boundary",
				"0.1",
			]),
		);
	});
});
