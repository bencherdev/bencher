import { measuresLayout } from "../explore/layout";
import { embedPath } from "../public/query";
import { type ExploreQuery, encodeQuery } from "../query/query";

// Each height holds a full key at any width from 320 to 1280, as measured,
// plus a margin; stacked panels grow by a phone's and a desktop's own step.
const SINGLE = 700;
const DUAL = 725;
const STACKED = [
	{ two: 760, step: 162 },
	{ two: 650, step: 192 },
];

/** The frame's height in pixels for `query`, which never scrolls inside it. */
export const embedHeight = (query: ExploreQuery) => {
	const measures = query.measures.length;
	if (measures <= 1) {
		return SINGLE;
	}
	if (measuresLayout(measures, query.layout).layout === "dual") {
		return DUAL;
	}
	return Math.max(
		...STACKED.map(({ two, step }) => two + (measures - 2) * step),
	);
};

const ENTITIES: Record<string, string> = {
	"&": "&amp;",
	'"': "&quot;",
	"'": "&#39;",
	"<": "&lt;",
	">": "&gt;",
};

const escapeAttribute = (value: string) =>
	value.replace(/[&"'<>]/g, (character) => ENTITIES[character] ?? character);

/** An `<iframe>` of the embed of `query`, for a page on another site. */
export const embedCode = (embed: {
	origin: string;
	slug: string;
	query: ExploreQuery;
	name: string;
	/** The pinned plot's title, when the query is one. */
	title?: string | undefined;
}) => {
	const { plot: _plot, ...query } = embed.query;
	const src = `${embed.origin}${embedPath(embed.slug)}${encodeQuery(query)}`;
	const title = embed.title ? `${embed.title} | ${embed.name}` : embed.name;
	return `<iframe src="${escapeAttribute(src)}" title="${escapeAttribute(title)}" width="100%" height="${embedHeight(query)}" style="border: 0" loading="lazy"></iframe>`;
};
