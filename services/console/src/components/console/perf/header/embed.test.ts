// @vitest-environment happy-dom
import { describe, expect, test } from "vitest";
import { embedTag, imgTag } from "./embed";

const HOSTILE = `Plot" onerror="alert(document.domain)" data-x="<b>&amp;`;
const PAGE_URL = "https://bencher.dev/perf/p?branches=a&testbeds=b";
const IMG_URL =
	"https://api.bencher.dev/v0/projects/p/perf/img?branches=a&testbeds=b";

const parse = (tag: string) => {
	const template = document.createElement("template");
	template.innerHTML = tag;
	return template.content;
};

describe("imgTag", () => {
	test("keeps a hostile project name inside its attributes", () => {
		const tag = imgTag({
			pageUrl: PAGE_URL,
			imgUrl: IMG_URL,
			name: HOSTILE,
			title: null,
		});
		const html = parse(tag);
		const img = html.querySelector("img");
		expect(html.querySelector("a")?.getAttribute("href")).toBe(PAGE_URL);
		expect(img?.getAttributeNames()).toEqual(["src", "title", "alt"]);
		expect(img?.getAttribute("src")).toBe(IMG_URL);
		expect(img?.getAttribute("title")).toBe(HOSTILE);
		expect(img?.getAttribute("alt")).toBe(`${HOSTILE} - Bencher`);
		expect(tag).not.toContain("<b>");
	});

	test("keeps a hostile title inside its attributes", () => {
		const img = parse(
			imgTag({
				pageUrl: PAGE_URL,
				imgUrl: IMG_URL,
				name: "Project",
				title: HOSTILE,
			}),
		).querySelector("img");
		expect(img?.getAttributeNames()).toEqual(["src", "title", "alt"]);
		expect(img?.getAttribute("title")).toBe(HOSTILE);
		expect(img?.getAttribute("alt")).toBe(`${HOSTILE} for Project - Bencher`);
	});
});

describe("embedTag", () => {
	test("keeps a hostile project name inside its attributes", () => {
		const tag = embedTag({ embedUrl: PAGE_URL, name: HOSTILE, title: null });
		const iframe = parse(tag).querySelector("iframe");
		expect(iframe?.getAttributeNames()).toEqual([
			"src",
			"title",
			"width",
			"height",
			"allow",
		]);
		expect(iframe?.getAttribute("src")).toBe(PAGE_URL);
		expect(iframe?.getAttribute("title")).toBe(HOSTILE);
		expect(tag).not.toContain("<b>");
	});

	test("keeps a hostile title inside its attributes", () => {
		const iframe = parse(
			embedTag({ embedUrl: PAGE_URL, name: "Project", title: HOSTILE }),
		).querySelector("iframe");
		expect(iframe?.getAttributeNames()).toEqual([
			"src",
			"title",
			"width",
			"height",
			"allow",
		]);
		expect(iframe?.getAttribute("title")).toBe(HOSTILE);
	});
});
