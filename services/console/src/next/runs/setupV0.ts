import { runHost } from "../project";

/** A results file in BMF v0: one benchmark, one measure, its value. */
export const SAMPLE_V0 = `{
  "my_benchmark": {
    "latency": { "value": 88.2 }
  }
}`;

/** The run that sends a version 0 project's first report from the sample to `host`, with a first threshold on its measure. */
export const runCommandV0 = (
	project: string,
	host: string | undefined,
): string =>
	[
		"bencher run",
		`--project ${project}`,
		...(runHost(host) ? [`--host ${host}`] : []),
		"--branch main",
		"--testbed localhost",
		"--adapter json",
		"--file results.json",
		"--threshold-measure latency",
		"--threshold-test t_test",
		"--threshold-max-sample-size 30",
		"--threshold-upper-boundary 0.99",
	].join(" \\\n  ");
