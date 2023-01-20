import Tutorial from "./sections/tutorial";
import HowTo from "./sections/how_to";
import Explanation from "./sections/explanation";
import Reference from "./sections/reference";

export const getHref = (section_slug: string, page_slug: string) => {
	return `/docs/${getPath(section_slug, page_slug)}`;
};

export const getPath = (section_slug: string, page_slug: string) => {
	return `${section_slug}/${page_slug}`;
};

export const docs = [
	{
		section: {
			name: "Tutorial",
			slug: "tutorial",
		},
		pages: Tutorial,
	},
	{
		section: {
			name: "How To",
			slug: "how-to",
		},
		pages: HowTo,
	},
	{
		section: {
			name: "Explanation",
			slug: "explanation",
		},
		pages: Explanation,
	},
	{
		section: {
			name: "Reference",
			slug: "reference",
		},
		pages: Reference,
	},
];
