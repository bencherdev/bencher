import { PerfQueryKey } from "../../types/bencher";

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
