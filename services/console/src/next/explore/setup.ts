/** A results file in BMF v1: one benchmark, two variants, one measure. */
export const SAMPLE = `{
  "my_benchmark": [
    {
      "parameters": { "size": 64 },
      "measures": { "latency": { "value": 88.2 } }
    },
    {
      "parameters": { "size": 4096 },
      "measures": { "latency": { "value": 512.7 } }
    }
  ]
}`;

// Bencher Cloud's API, which `bencher run` reports to unless told otherwise.
const CLOUD = "https://api.bencher.dev";

/**
 * The run that sends a project's first report from the sample to `host`, with
 * a first threshold on its measure: a version 1 threshold names its metric too.
 */
export const runCommand = (project: string, host: string | undefined): string =>
	[
		"bencher run",
		`--project ${project}`,
		...(host && host !== CLOUD ? [`--host ${host}`] : []),
		"--branch main",
		"--testbed localhost",
		"--adapter json",
		"--file results.json",
		"--threshold-measure latency",
		"--threshold-metric value",
		"--threshold-test t_test",
		"--threshold-max-sample-size 30",
		"--threshold-upper-boundary 0.99",
	].join(" \\\n  ");

export const exportLine = (key: string) => `export BENCHER_API_KEY=${key}`;
