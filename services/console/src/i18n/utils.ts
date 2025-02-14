import { type DataEntryMap, getCollection } from "astro:content";
import type Collection from "../util/collection";
import { splitPageId } from "../util/collection";
import { type Language, defaultLang, showDefaultLang } from "./ui";

export const getEnOnlyPaths = async (collection: Collection) => {
	const pages = await getCollection(collection as keyof DataEntryMap);
	return pages.filter(filterDraft).map((page) => {
		const [_lang, slug] = splitPageId(page.id);
		return {
			params: { slug },
			props: page,
		};
	});
};

export async function getEnPaths(collection: Collection) {
	const pages = await getPaths(collection);
	return pages.filter((page) => page.params.lang === defaultLang);
}

export async function getLangPaths(collection: Collection) {
	const pages = await getPaths(collection);
	return pages.filter((page) => page.params.lang !== defaultLang);
}

async function getPaths(collection: Collection) {
	const pages = await getCollection(collection as keyof DataEntryMap);
	return pages.filter(filterDraft).map((page) => {
		const [lang, slug] = splitPageId(page.id);
		return {
			params: { lang, slug },
			props: page,
		};
	});
}

export const getEnOnlyCollection = async (collection: Collection) => {
	const pages = await getCollection(collection as keyof DataEntryMap);
	return pages
		.filter(filterDraft)
		.sort((a, b) => a.data.sortOrder - b.data.sortOrder);
};

export async function getLangCollection(collection: Collection) {
	const pages = await getCollection(collection as keyof DataEntryMap);
	const langPagesMap = pages
		.filter(filterDraft)
		.map((page) => {
			const [lang, _slug] = splitPageId(page.id);
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

const filterDraft = (page: { data: { draft?: boolean } }): boolean => {
	switch (import.meta.env.MODE) {
		case "development":
			return true;
		case "production":
			return !page.data.draft;
		default:
			return false;
	}
};

export const API_DOCS_PUBLISHED = "2024-02-12T07:26:00Z";
export const API_DOCS_MODIFIED = "2024-06-21T17:47:00Z";
