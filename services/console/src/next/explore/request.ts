import {
	type ExploreQuery,
	blankQuery,
	canonicalParameters,
	encodeQuery,
} from "../query/query";

/** Whether the query names what the plot query needs: a branch, a testbed, a benchmark, and a measure. */
export const drawable = (query: ExploreQuery): boolean =>
	query.branches.length > 0 &&
	query.testbeds.length > 0 &&
	query.benchmarks.length > 0 &&
	query.measures.length > 0;

/** Whether Explore draws a plot for `query`: it can draw, or it opens a pin. */
export const drawsPlot = (query: ExploreQuery): boolean =>
	drawable(query) || query.plot !== undefined;

/** What the plot query asks, without when: the hidden and focused lines and the view never refetch. */
export const requestKey = (query: ExploreQuery): string =>
	encodeQuery({
		...blankQuery(),
		branches: query.branches,
		testbeds: query.testbeds,
		benchmarks: query.benchmarks,
		sets: query.sets,
		measures: query.measures,
		metrics: query.metrics,
		window: query.window,
	});

/** The plot query's search for `query`, with a rolling window ending `now` unless it ends earlier. */
export const perfSearch = (query: ExploreQuery, now: number): string => {
	const params = new URLSearchParams();
	const { branches, testbeds, window } = query;
	params.set("branches", branches.map(({ uuid }) => uuid).join(","));
	if (branches.some(({ head }) => head !== undefined)) {
		params.set("heads", branches.map(({ head }) => head ?? "").join(","));
	}
	params.set("testbeds", testbeds.map(({ uuid }) => uuid).join(","));
	if (testbeds.some(({ spec }) => spec !== undefined)) {
		params.set("specs", testbeds.map(({ spec }) => spec ?? "").join(","));
	}
	params.set("benchmarks", query.benchmarks.join(","));
	// The API splits these lists on commas before it decodes each entry.
	if (query.sets.length > 0) {
		params.set(
			"parameters",
			query.sets
				.map((set) => encodeURIComponent(canonicalParameters(set)))
				.join(","),
		);
	}
	params.set("measures", query.measures.join(","));
	if (query.metrics.length > 0) {
		params.set("metrics", query.metrics.map(encodeURIComponent).join(","));
	}
	const end = window.end ?? now;
	const start = "start" in window ? window.start : end - window.seconds * 1000;
	params.set("start_time", String(start));
	params.set("end_time", String(end));
	return `?${params}`;
};
