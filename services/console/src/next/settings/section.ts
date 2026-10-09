export type Section = "general" | "dimensions" | "keys";

/** The rail's sections, in order, each with its path under the project. */
export const SECTIONS: readonly {
	section: Section;
	label: string;
	path: string;
}[] = [
	{ section: "general", label: "General", path: "settings" },
	{ section: "dimensions", label: "Dimensions", path: "branches" },
	{ section: "keys", label: "Keys", path: "settings/keys" },
];

/** The section of Settings' page a path draws; the classic keys path is Keys. */
export const sectionOf = (rest: string[]): Section | undefined => {
	const [first, second, ...more] = rest;
	if (first === "keys") {
		return second === undefined ? "keys" : undefined;
	}
	if (first !== "settings" || more.length > 0) {
		return undefined;
	}
	switch (second) {
		case undefined:
			return "general";
		case "keys":
			return "keys";
		default:
			return undefined;
	}
};
