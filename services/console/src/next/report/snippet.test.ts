import { execFileSync } from "node:child_process";
import { describe, expect, test } from "vitest";
import { Adapter } from "../../types/bencher";
import { type SnippetLine, cliAdapter, thresholdSnippets } from "./snippet";

const PLACE = {
	project: "hashbrown",
	branch: "main",
	testbed: "linux-x86-64",
	adapter: Adapter.Json,
};

type TestLine = SnippetLine & { benchmark: string };

const line = (
	parameters: SnippetLine["parameters"],
	more: Partial<TestLine> = {},
): TestLine => ({
	benchmark: "blake3",
	parameters,
	measure: "latency",
	metric: "value",
	...more,
});

const THIS = line({ input_bytes: 65536, threads: 1, simd: "avx2" });

/** The argv `bencher` receives when a shell runs the snippet. */
const argv = (text: string): string[] =>
	execFileSync("sh", ["-c", `bencher() { printf '%s\\0' "$@"; }\n${text}`], {
		encoding: "utf8",
	})
		.split("\0")
		.slice(0, -1);

/** The key `bencher` finds in its environment when a shell runs the snippet. */
const apiKey = (text: string): string =>
	execFileSync(
		"sh",
		["-c", `bencher() { printf '%s' "$BENCHER_API_KEY"; }\n${text}`],
		{ encoding: "utf8", env: {} },
	);

// `--adapter` values `bencher run` takes; a snippet spells `json_v0` and `json_v1` as `json`.
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
	"js_deno",
	"js_mitata",
	"js_time",
	"js_tinybench",
	"js_vitest",
	"python_asv",
	"python_pytest",
	"ruby_benchmark",
	"ruby_benchmark_ips",
	"rust_bench",
	"rust_criterion",
	"rust_divan",
	"rust_iai",
	"rust_gungraun",
	"shell_hyperfine",
]);

const COMMON = [
	"run",
	"--project",
	"hashbrown",
	"--branch",
	"main",
	"--testbed",
	"linux-x86-64",
	"--adapter",
	"json",
];

describe("thresholdSnippets", () => {
	// Kills a dropped flag, a broken line continuation, or the key left out.
	test("the exact threshold names this line's metric and parameters", () => {
		const { exact } = thresholdSnippets(PLACE, THIS, [THIS]);
		expect(apiKey(exact?.text ?? "")).toMatch(/^bencher_run_/);
		expect(argv(exact?.text ?? "")).toEqual([
			...COMMON,
			"--threshold-measure",
			"latency",
			"--threshold-metric",
			"value",
			"--threshold-parameters",
			'{"input_bytes": 65536, "simd": "avx2", "threads": 1}',
			"--threshold-test",
			"t_test",
			"--threshold-max-sample-size",
			"30",
			"--threshold-upper-boundary",
			"0.99",
			"--file",
			"results.json",
		]);
	});

	// Kills a simpler threshold without a metric, which a version 1 project refuses.
	test("the simpler threshold keeps the metric and drops the parameters", () => {
		const { simple } = thresholdSnippets(PLACE, THIS, [THIS]);
		expect(apiKey(simple.text)).toMatch(/^bencher_run_/);
		expect(argv(simple.text)).toEqual([
			...COMMON,
			"--threshold-measure",
			"latency",
			"--threshold-metric",
			"value",
			"--threshold-test",
			"t_test",
			"--threshold-max-sample-size",
			"30",
			"--threshold-upper-boundary",
			"0.99",
			"--file",
			"results.json",
		]);
	});

	// Kills a self-hosted run sent to Bencher Cloud.
	test("names the API on a self-hosted server", () => {
		const host = "https://bencher.example.com:6610";
		const { exact, simple } = thresholdSnippets({ ...PLACE, host }, THIS, [
			THIS,
		]);
		for (const snippet of [exact, simple]) {
			const args = argv(snippet?.text ?? "");
			expect(args.slice(1, 5)).toEqual([
				"--project",
				"hashbrown",
				"--host",
				host,
			]);
		}
	});

	// Kills highlighting the wrong rows, or copying text the sheet does not show.
	test("flags the threshold rows and copies every row", () => {
		const { exact, simple } = thresholdSnippets(PLACE, THIS, [THIS]);
		for (const snippet of [exact, simple]) {
			expect(snippet?.code.map(({ text }) => text).join("\n")).toBe(
				snippet?.text,
			);
			expect(
				snippet?.code
					.filter(({ flag }) => flag)
					.map(({ text }) => text.trim().split(" ")[0]),
			).toEqual([
				"--threshold-measure",
				"--threshold-metric",
				...(snippet === exact ? ["--threshold-parameters"] : []),
				"--threshold-test",
				"--threshold-max-sample-size",
				"--threshold-upper-boundary",
			]);
		}
	});

	// Kills matching by benchmark, by exact parameters, by loose equality, or across metrics and measures.
	test("each threshold checks the lines of its measure and metric that its parameters match", () => {
		const wider = line({
			input_bytes: 65536,
			threads: 1,
			simd: "avx2",
			unroll: true,
		});
		const otherBenchmark = line(
			{ threads: 1, simd: "avx2", input_bytes: 65536 },
			{ benchmark: "sha256" },
		);
		const otherValue = line({ input_bytes: 65536, threads: 4, simd: "avx2" });
		const missingKey = line({ input_bytes: 65536, simd: "avx2" });
		const stringly = line({ input_bytes: "65536", threads: 1, simd: "avx2" });
		const otherMetric = line(
			{ input_bytes: 65536, threads: 1, simd: "avx2" },
			{ metric: "p99" },
		);
		const otherMeasure = line(
			{ input_bytes: 65536, threads: 1, simd: "avx2" },
			{ measure: "throughput" },
		);
		const lines = [
			otherValue,
			THIS,
			wider,
			otherBenchmark,
			missingKey,
			stringly,
			otherMetric,
			otherMeasure,
		];
		const { exact, simple } = thresholdSnippets(PLACE, THIS, lines);
		expect(exact?.checks).toEqual([THIS, wider, otherBenchmark]);
		expect(simple.checks).toEqual([
			otherValue,
			THIS,
			wider,
			otherBenchmark,
			missingKey,
			stringly,
		]);
	});

	// Kills an exact threshold that is really the simpler one, or a parameters filter of `{}`.
	test("a line with no parameters has only the simpler threshold", () => {
		const bare = line({});
		const other = line({ threads: 4 });
		const { exact, simple } = thresholdSnippets(PLACE, bare, [bare, other]);
		expect(exact).toBeNull();
		expect(simple.checks).toEqual([bare, other]);
		expect(simple.text).not.toContain("--threshold-parameters");
	});

	// Kills unquoted or wrongly escaped values, which the shell would split or end early.
	test("quotes every value the shell would otherwise change", () => {
		const parameters = {
			name: 'it\'s "fine" $HOME `x` \\ 名前',
			"10": 1.5,
			"9": false,
		};
		const tricky = line(parameters, { metric: "p99 tail" });
		const { exact } = thresholdSnippets(PLACE, tricky, [tricky]);
		const args = argv(exact?.text ?? "");
		const at = (flag: string) => args[args.indexOf(flag) + 1];
		expect(at("--threshold-metric")).toBe("p99 tail");
		expect(JSON.parse(at("--threshold-parameters") ?? "")).toEqual(parameters);
	});

	describe("the guarded side", () => {
		const boundaries = (snippet: { text: string }) =>
			argv(snippet.text).filter((arg) => arg.endsWith("-boundary"));

		// Kills ignoring how the project already guards this measure.
		test("follows another threshold on the same measure", () => {
			const checked = line({ threads: 4 }, { metric: "p99", guard: "lower" });
			const { simple } = thresholdSnippets(PLACE, THIS, [THIS, checked]);
			expect(boundaries(simple)).toEqual(["--threshold-lower-boundary"]);
		});

		// Kills a two-sided threshold losing a side.
		test("keeps both sides when that threshold guards both", () => {
			const checked = line({ threads: 4 }, { guard: "both" });
			const { exact } = thresholdSnippets(PLACE, THIS, [THIS, checked]);
			expect(boundaries(exact ?? { text: "" })).toEqual([
				"--threshold-lower-boundary",
				"--threshold-upper-boundary",
			]);
		});

		// Kills taking the side from another measure.
		test("ignores thresholds on other measures", () => {
			const elsewhere = line(
				{ threads: 4 },
				{ measure: "instructions", guard: "lower" },
			);
			const { simple } = thresholdSnippets(PLACE, THIS, [THIS, elsewhere]);
			expect(boundaries(simple)).toEqual(["--threshold-upper-boundary"]);
		});

		// Kills guarding throughput against getting faster.
		test("guards throughput from below", () => {
			const fast = line({ threads: 1 }, { measure: "throughput" });
			const { simple } = thresholdSnippets(PLACE, fast, [fast]);
			expect(boundaries(simple)).toEqual(["--threshold-lower-boundary"]);
		});
	});
});

describe("the adapter", () => {
	const adapterOf = (adapter: Adapter) => {
		const { exact, simple } = thresholdSnippets({ ...PLACE, adapter }, THIS, [
			THIS,
		]);
		return [exact, simple].map((snippet) => {
			const args = argv(snippet?.text ?? "");
			return args[args.indexOf("--adapter") + 1];
		});
	};

	// Kills a snippet that names `json` for every report, which reads a
	// harness's output as JSON and fails the run.
	test("names the report's harness, as the CLI spells it", () => {
		expect(adapterOf(Adapter.RustCriterionJson)).toEqual([
			"rust_criterion",
			"rust_criterion",
		]);
		expect(adapterOf(Adapter.RustDivan)).toEqual(["rust_divan", "rust_divan"]);
	});

	// Kills a report's adapter passed through as stored, which the CLI refuses
	// for a language, or a fixed adapter in place of the report's.
	test("names magic for a magic or a language report, and json for a json report", () => {
		expect(adapterOf(Adapter.Magic)).toEqual(["magic", "magic"]);
		expect(adapterOf(Adapter.Rust)).toEqual(["magic", "magic"]);
		expect(adapterOf(Adapter.Json)).toEqual(["json", "json"]);
		expect(adapterOf(Adapter.JsonV1)).toEqual(["json", "json"]);
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
