import { execFileSync } from "node:child_process";
import { describe, expect, test } from "vitest";
import { Adapter } from "../../types/bencher";
import type { SnippetLine } from "./snippet";
import {
	cliAdapter,
	declaresThresholdV0,
	thresholdSnippetV0,
} from "./snippetV0";

const PLACE = {
	project: "crinkle-cut",
	branch: "main",
	testbed: "ubuntu-latest",
	adapter: Adapter.RustCriterion,
};

const line = (more: Partial<SnippetLine> = {}): SnippetLine => ({
	parameters: {},
	measure: "latency",
	metric: "value",
	...more,
});

/** The argv `bencher` receives when a shell runs the snippet. */
const argv = (text: string): string[] =>
	execFileSync("sh", ["-c", `bencher() { printf '%s\\0' "$@"; }\n${text}`], {
		encoding: "utf8",
	})
		.split("\0")
		.slice(0, -1);

// Every `--adapter` value `bencher run` takes.
const CLI_ADAPTERS = new Set([
	"magic",
	"json",
	"c_sharp_dot_net",
	"cpp_catch2",
	"cpp_google",
	"dart_benchmark_harness",
	"go_bench",
	"java_jmh",
	"js_benchmark",
	"js_time",
	"js_vitest",
	"python_asv",
	"python_pytest",
	"ruby_benchmark",
	"rust_bench",
	"rust_criterion",
	"rust_iai",
	"rust_gungraun",
	"shell_hyperfine",
]);

describe("thresholdSnippetV0", () => {
	// Kills the version 1 flags a version 0 project refuses with a 400, a
	// snippet that names `json` rather than the report's adapter, a dropped
	// flag, and a broken continuation.
	test("declares the measure's threshold with plain flags and the report's adapter", () => {
		const snippet = thresholdSnippetV0(PLACE, line(), [line()]);
		expect(argv(snippet.text)).toEqual([
			"run",
			"--project",
			"crinkle-cut",
			"--branch",
			"main",
			"--testbed",
			"ubuntu-latest",
			"--adapter",
			"rust_criterion",
			"--threshold-measure",
			"latency",
			"--threshold-test",
			"t_test",
			"--threshold-max-sample-size",
			"30",
			"--threshold-upper-boundary",
			"0.99",
			"--file",
			"results.json",
		]);
		expect(snippet.code.map(({ text }) => text).join("\n")).toBe(snippet.text);
	});

	// Kills a guard that ignores how the measure is guarded, or a throughput
	// line guarded against rising.
	test("guards a throughput against falling", () => {
		const throughput = line({ measure: "throughput" });
		const args = argv(thresholdSnippetV0(PLACE, throughput, [throughput]).text);
		expect(args).toContain("--threshold-lower-boundary");
		expect(args).not.toContain("--threshold-upper-boundary");
	});

	// Kills a self-hosted run sent to Bencher Cloud.
	test("names the API on a self-hosted server", () => {
		const host = "https://bencher.example.com:6610";
		const args = argv(
			thresholdSnippetV0({ ...PLACE, host }, line(), [line()]).text,
		);
		expect(args.slice(1, 5)).toEqual([
			"--project",
			"crinkle-cut",
			"--host",
			host,
		]);
	});

	// Kills counting a bound line, which a version 0 threshold never checks,
	// or a line of another measure.
	test("checks the measure's value lines only", () => {
		const value = line({ parameters: { a: 1 } });
		const lower = line({ metric: "lower_value" });
		const other = line({ measure: "throughput" });
		expect(
			thresholdSnippetV0(PLACE, line(), [line(), value, lower, other]).checks,
		).toEqual([line(), value]);
	});
});

describe("declaresThresholdV0", () => {
	// Kills offering a threshold for a bound line, which has none to declare.
	test("only a value line declares a threshold", () => {
		expect(declaresThresholdV0({ metric: "value" })).toBe(true);
		expect(declaresThresholdV0({ metric: "lower_value" })).toBe(false);
		expect(declaresThresholdV0({ metric: "upper_value" })).toBe(false);
	});
});

describe("cliAdapter", () => {
	// Kills a stored adapter passed through to `--adapter`, which the CLI
	// refuses, or one spelled as a different harness.
	test("every stored adapter is one the CLI takes, for the same harness", () => {
		for (const adapter of Object.values(Adapter)) {
			const cli = cliAdapter(adapter);
			expect(CLI_ADAPTERS.has(cli), adapter).toBe(true);
			if (CLI_ADAPTERS.has(adapter)) {
				expect(cli).toBe(adapter);
			}
		}
		expect(cliAdapter(Adapter.JsonV0)).toBe("json");
		expect(cliAdapter(Adapter.ShellHyperfineV2)).toBe("shell_hyperfine");
		expect(cliAdapter(Adapter.RustGungraunJson)).toBe("rust_gungraun");
		expect(cliAdapter(Adapter.Python)).toBe("magic");
	});
});
