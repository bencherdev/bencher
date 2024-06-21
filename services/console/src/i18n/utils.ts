import type Collection from "../util/collection";
import { type Language, defaultLang, showDefaultLang } from "./ui";
import { getCollection } from "astro:content";

export async function getEnPaths(collection: Collection) {
	const pages = await getPaths(collection);
	return pages.filter((page) => page.params.lang === defaultLang);
}

export async function getLangPaths(collection: Collection) {
	const pages = await getPaths(collection);
	return pages.filter((page) => page.params.lang !== defaultLang);
}

async function getPaths(collection: Collection) {
	const pages = await getCollection(collection);
	return pages.filter(filterDraft).map((page) => {
		const [lang, ...slug] =
			page.id.substring(0, page.id.lastIndexOf("."))?.split("/") ?? [];
		return {
			params: { lang, slug: slug.join("/") || undefined },
			props: page,
		};
	});
}

export async function getLangCollection(collection: Collection) {
	const pages = await getCollection(collection);
	const langPagesMap = pages
		.filter(filterDraft)
		.map((page) => {
			const [lang, ...slug] =
				page.id.substring(0, page.id.lastIndexOf("."))?.split("/") ?? [];
			page.slug = slug.join("/") || undefined;
			return { lang, page };
		})
		.reduce((lpMap, langPage) => {
			if (lpMap[langPage.lang]) {
				lpMap[langPage.lang].push(langPage.page);
			} else {
				lpMap[langPage.lang] = [langPage.page];
			}
			return lpMap;
		}, {});
	const langPagesMapSorted = {};
	for (const [key, value] of Object.entries(langPagesMap)) {
		langPagesMapSorted[key] = value.sort(
			(a, b) => a.data.sortOrder - b.data.sortOrder,
		);
	}
	return langPagesMapSorted;
}

export const langPath = (lang: Language) =>
	!showDefaultLang && lang === defaultLang ? "" : `${lang}/`;

const filterDraft = (page: { data: { draft: boolean } }): boolean => {
	switch (import.meta.env.MODE) {
		case "development":
			return true;
		case "production":
			return !page.data.draft;
		default:
			return false;
	}
};

export const getEnCollection = async (collection: Collection) => {
	const pages = await getCollection(collection);
	return pages
		.filter(filterDraft)
		.sort((a, b) => a.data.sortOrder - b.data.sortOrder);
};

export const API_DOCS_PUBLISHED = "2024-02-12T07:26:00Z";
export const API_DOCS_MODIFIED = "2024-06-21T17:47:00Z";
