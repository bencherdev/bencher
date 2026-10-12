import { DIMENSIONS } from "../paths";

export type Dimension = (typeof DIMENSIONS)[number];

interface Kind {
	label: string;
	/** The name column's header. */
	title: string;
	one: string;
	/** The column after the name: what tells two rows apart at a glance. */
	column: string;
	/** The thresholds' query parameter, for the dimensions a threshold belongs to. */
	threshold?: "branch" | "testbed" | "measure";
}

export const KINDS: Record<Dimension, Kind> = {
	branches: {
		label: "Branches",
		title: "Branch",
		one: "branch",
		column: "Head",
		threshold: "branch",
	},
	testbeds: {
		label: "Testbeds",
		title: "Testbed",
		one: "testbed",
		column: "Spec",
		threshold: "testbed",
	},
	benchmarks: {
		label: "Benchmarks",
		title: "Benchmark",
		one: "benchmark",
		column: "Variants",
	},
	measures: {
		label: "Measures",
		title: "Measure",
		one: "measure",
		column: "Units",
		threshold: "measure",
	},
};

/** A dimension list, or one dimension's page, from the path after the project. */
export const placeOf = (
	rest: string[],
): { dimension: Dimension; entry?: string } | undefined => {
	const [first, entry, ...more] = rest;
	const dimension = DIMENSIONS.find((known) => known === first);
	if (!dimension || more.length > 0) {
		return undefined;
	}
	return entry === undefined ? { dimension } : { dimension, entry };
};
