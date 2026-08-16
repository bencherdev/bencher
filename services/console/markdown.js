// Markdown pipeline helpers for `astro.config.mjs`.
// This file is plain ESM so the config can import it directly, with no build step.
import { defineHastPlugin } from "satteri";

const ANCHOR_STYLE = "padding-left: 0.3em; color: #fdb07e;";
const ANCHOR_LABEL = "Link to section";

const HEADINGS = ["h1", "h2", "h3", "h4", "h5", "h6"];

// Append a self link to every heading, keyed off the `id` that
// `satteriHeadingIdsPlugin()` has already assigned.
// Pagefind ignores the anchor so the link icon never lands in the search index.
export const headingAutolink = () =>
	defineHastPlugin({
		name: "heading-autolink",
		element: {
			filter: HEADINGS,
			visit(node, ctx) {
				const id = node.properties?.id;
				if (typeof id !== "string" || id.length === 0) {
					return;
				}
				ctx.appendChild(node, {
					type: "element",
					tagName: "a",
					properties: {
						style: ANCHOR_STYLE,
						"aria-label": ANCHOR_LABEL,
						"data-pagefind-ignore": "",
						href: `#${id}`,
					},
					children: [
						{
							type: "element",
							tagName: "small",
							properties: {},
							children: [
								{
									type: "element",
									tagName: "i",
									properties: { className: ["fas", "fa-link"] },
									children: [],
								},
							],
						},
					],
				});
			},
		},
	});
