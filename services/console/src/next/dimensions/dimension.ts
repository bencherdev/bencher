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
	lede: string;
}

export const KINDS: Record<Dimension, Kind> = {
	branches: {
		label: "Branches",
		title: "Branch",
		one: "branch",
		column: "Head",
		threshold: "branch",
		lede: "Runs create branches. Archiving one hides its lines and retires its thresholds; unarchiving it brings back each threshold whose testbed and measure are active. A run that reports an archived branch brings it back.",
	},
	testbeds: {
		label: "Testbeds",
		title: "Testbed",
		one: "testbed",
		column: "Spec",
		threshold: "testbed",
		lede: "Runs create testbeds. Archiving one hides its lines and retires its thresholds; unarchiving it brings back each threshold whose branch and measure are active. A run that reports an archived testbed brings it back.",
	},
	benchmarks: {
		label: "Benchmarks",
		title: "Benchmark",
		one: "benchmark",
		column: "Variants",
		lede: "Runs create benchmarks. Archiving one hides its lines; its thresholds stay, since a threshold belongs to a branch, testbed, and measure. A run that reports an archived benchmark brings it back.",
	},
	measures: {
		label: "Measures",
		title: "Measure",
		one: "measure",
		column: "Units",
		threshold: "measure",
		lede: "Runs create measures. Archiving one hides its lines and retires its thresholds; unarchiving it brings back each threshold whose branch and testbed are active. A run that reports an archived measure brings it back.",
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
