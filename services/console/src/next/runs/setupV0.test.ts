import { describe, expect, test } from "vitest";
import { SAMPLE_V0, runCommandV0 } from "./setupV0";

const CLOUD = "https://api.bencher.dev";

/** Each flag of a command and the value after it. */
const flags = (command: string) =>
	new Map(
		[...command.matchAll(/--([a-z-]+) (\S+)/g)].map(([, flag, value]) => [
			flag,
			value,
		]),
	);

describe("runCommandV0", () => {
	// Kills a command for another project, one that reads anything but the
	// sample, and the version 1 threshold flags, which a version 0 project
	// refuses with a 400.
	test("the run reports to this project from the sample and declares a plain threshold", () => {
		const command = runCommandV0("rosti-1a2b", CLOUD);
		const run = flags(command);
		expect(run.get("project")).toBe("rosti-1a2b");
		expect(run.get("adapter")).toBe("json");
		expect(run.get("file")).toBe("results.json");
		expect(run.get("threshold-test")).toBe("t_test");
		expect(run.get("threshold-max-sample-size")).toBe("30");
		expect(run.get("threshold-upper-boundary")).toBe("0.99");
		expect(command).not.toContain("--threshold-metric");
		expect(command).not.toContain("--threshold-parameters");
	});

	// Kills a line that does not continue, which a shell runs as a command of its own.
	test("every line but the last continues the command", () => {
		const lines = runCommandV0("rosti", "http://localhost:61016").split("\n");
		expect(lines.length).toBeGreaterThan(2);
		expect(lines.slice(0, -1).every((line) => line.endsWith(" \\"))).toBe(true);
		expect(lines.at(-1)?.endsWith("\\")).toBe(false);
	});

	// Kills a self-hosted console's run that reports to Bencher Cloud, and a
	// host spelled out for Bencher Cloud.
	test("the run names its host unless it is Bencher Cloud", () => {
		expect(flags(runCommandV0("rosti", CLOUD)).has("host")).toBe(false);
		expect(
			flags(runCommandV0("rosti", "http://localhost:61016")).get("host"),
		).toBe("http://localhost:61016");
	});

	// Kills a version 1 sample, which a version 0 project cannot read, and a
	// threshold on a measure the sample does not report.
	test("the sample is BMF v0 and reports the threshold's measure", () => {
		const measure = flags(runCommandV0("rosti", CLOUD)).get(
			"threshold-measure",
		);
		const sample: Record<
			string,
			Record<string, Record<string, number>>
		> = JSON.parse(SAMPLE_V0);
		const benchmarks = Object.values(sample);
		expect(benchmarks.length).toBeGreaterThan(0);
		for (const measures of benchmarks) {
			expect(Array.isArray(measures)).toBe(false);
			expect(measures[measure ?? ""]?.value).toBeTypeOf("number");
		}
	});
});
