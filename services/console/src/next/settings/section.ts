export type Section = "general" | "dimensions" | "keys" | "connections";

/** The rail's sections, in order, each with its path under the project. */
export const SECTIONS: readonly {
	section: Section;
	label: string;
	path: string;
}[] = [
	{ section: "general", label: "General", path: "settings" },
	{ section: "dimensions", label: "Dimensions", path: "branches" },
	{ section: "keys", label: "Keys", path: "keys" },
	{ section: "connections", label: "Connections", path: "connections" },
];

/** The section of Settings' page a path draws. */
export const sectionOf = (rest: string[]): Section | undefined => {
	const [first, ...more] = rest;
	if (more.length > 0) {
		return undefined;
	}
	switch (first) {
		case "settings":
			return "general";
		case "keys":
			return "keys";
		case "connections":
			return "connections";
		default:
			return undefined;
	}
};
