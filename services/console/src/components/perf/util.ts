import type { Params } from "astro";
import { PerfQueryKey } from "../../types/bencher";

export enum PubResourceKind {
	Project = "project",
	Branch = "branch",
	Testbed = "testbed",
	Benchmark = "benchmark",
	Measure = "measure",
	Metric = "metric",
	Threshold = "threshold",
	Alert = "alert",
}

// biome-ignore lint/complexity/noStaticOnlyClass: enum methods
export class PubResource {
	public static resource(resource: PubResourceKind) {
		switch (resource) {
			case PubResourceKind.Project:
				return "projects";
			case PubResourceKind.Branch:
				return "branches";
			case PubResourceKind.Testbed:
				return "testbeds";
			case PubResourceKind.Benchmark:
				return "benchmarks";
			case PubResourceKind.Measure:
				return "measures";
			case PubResourceKind.Metric:
				return "metrics";
			case PubResourceKind.Threshold:
				return "thresholds";
			case PubResourceKind.Alert:
				return "alerts";
		}
	}

	public static param(resource: PubResourceKind) {
		switch (resource) {
			case PubResourceKind.Project:
				return "project";
			case PubResourceKind.Branch:
				return "branch";
			case PubResourceKind.Testbed:
				return "testbed";
			case PubResourceKind.Benchmark:
				return "benchmark";
			case PubResourceKind.Measure:
				return "measure";
			case PubResourceKind.Metric:
				return "metric";
			case PubResourceKind.Threshold:
				return "threshold";
			case PubResourceKind.Alert:
				return "alert";
		}
	}

	public static search(resource: PubResourceKind, search: Params) {
		switch (resource) {
			case PubResourceKind.Project:
			case PubResourceKind.Branch:
			case PubResourceKind.Testbed:
			case PubResourceKind.Benchmark:
			case PubResourceKind.Measure:
			case PubResourceKind.Metric:
			case PubResourceKind.Alert:
				return {};
			case PubResourceKind.Threshold:
				return { model: search?.model };
		}
	}
}

export const fetchSSR = async (url: string, timeout = 1_000) =>
	await fetch(url, { signal: AbortSignal.timeout(timeout) });

export const hasPerfImage = (url: URL) =>
	[
		PerfQueryKey.Branches,
		PerfQueryKey.Testbeds,
		PerfQueryKey.Benchmarks,
		PerfQueryKey.Measures,
	].reduce(
		(acc, dimension) =>
			acc && (url.searchParams.get(dimension)?.length ?? 0) > 0,
		true,
	);
