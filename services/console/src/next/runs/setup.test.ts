import { describe, expect, test } from "vitest";
import { SAMPLE, runCommand } from "./setup";

const CLOUD = "https://api.bencher.dev";

/** Each flag of a command and the value after it. */
const flags = (command: string) =>
	new Map(
		[...command.matchAll(/--([a-z-]+) (\S+)/g)].map(([, flag, value]) => [
			flag,
			value,
		]),
	);

describe("runCommand", () => {
	// Kills a command for another project, one that reads anything but the
	// BMF v1 file, and a threshold without a metric, which a version 1
	// project refuses.
	test("the run reports to this project from the sample and declares a first threshold", () => {
		const run = flags(runCommand("rosti-1a2b", CLOUD));
		expect(run.get("project")).toBe("rosti-1a2b");
		expect(run.get("adapter")).toBe("json");
		expect(run.get("file")).toBe("results.json");
		for (const flag of [
			"threshold-measure",
			"threshold-metric",
			"threshold-test",
			"threshold-upper-boundary",
		]) {
			expect(run.has(flag), flag).toBe(true);
		}
	});

	// Kills a line that does not continue, which a shell runs as a command of its own.
	test("every line but the last continues the command", () => {
		const lines = runCommand("rosti", "http://localhost:61016").split("\n");
		expect(lines.length).toBeGreaterThan(2);
		expect(lines.slice(0, -1).every((line) => line.endsWith(" \\"))).toBe(true);
		expect(lines.at(-1)?.endsWith("\\")).toBe(false);
	});

	// Kills a threshold the CLI refuses (the test's kebab spelling) or the API
	// refuses (a boundary as a percentage), and one with no sample cap.
	test("the threshold's values are ones the CLI and the API accept", () => {
		const run = flags(runCommand("rosti", CLOUD));
		expect(run.get("threshold-test")).toBe("t_test");
		expect(run.get("threshold-max-sample-size")).toBe("30");
		expect(run.get("threshold-upper-boundary")).toBe("0.99");
	});

	// Kills a self-hosted console's run that reports to Bencher Cloud, and a
	// host spelled out for Bencher Cloud or for a console that names none.
	test("the run names its host unless it is Bencher Cloud", () => {
		for (const host of [CLOUD, undefined]) {
			expect(flags(runCommand("rosti", host)).has("host")).toBe(false);
		}
		expect(
			flags(runCommand("rosti", "http://localhost:61016")).get("host"),
		).toBe("http://localhost:61016");
	});

	// Kills a threshold on a measure or a metric the sample does not report,
	// which would check nothing.
	test("the threshold checks a measure and a metric the sample reports", () => {
		const run = flags(runCommand("rosti", CLOUD));
		const sample: Record<
			string,
			{ parameters: object; measures: Record<string, Record<string, number>> }[]
		> = JSON.parse(SAMPLE);
		const variants = Object.values(sample).flat();
		expect(variants.length).toBeGreaterThan(1);
		for (const { parameters, measures } of variants) {
			expect(parameters).toBeTypeOf("object");
			const measure = measures[run.get("threshold-measure") ?? ""];
			expect(measure?.[run.get("threshold-metric") ?? ""]).toBeTypeOf("number");
		}
	});
});
