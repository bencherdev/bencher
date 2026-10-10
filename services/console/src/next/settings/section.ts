export type Section = "general" | "dimensions" | "keys";

/** The rail's sections, in order, each with its path under the project. */
export const SECTIONS: readonly {
	section: Section;
	label: string;
	path: string;
}[] = [
	{ section: "general", label: "General", path: "settings" },
	{ section: "dimensions", label: "Dimensions", path: "settings/dimensions" },
	{ section: "keys", label: "Keys", path: "settings/keys" },
];

// The dimension lists and pages keep their classic paths.
const DIMENSIONS = ["branches", "testbeds", "benchmarks", "measures"];

/** The section a path under Settings belongs to; the classic keys path is Keys. */
export const sectionOf = (rest: string[]): Section | undefined => {
	const [first, second, ...more] = rest;
	if (first && DIMENSIONS.includes(first)) {
		return "dimensions";
	}
	if (first === "keys") {
		return second === undefined ? "keys" : undefined;
	}
	if (first !== "settings" || more.length > 0) {
		return undefined;
	}
	switch (second) {
		case undefined:
			return "general";
		case "dimensions":
			return "dimensions";
		case "keys":
			return "keys";
		default:
			return undefined;
	}
};
