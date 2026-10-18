import { embedHeight } from "../../../../config/types";

const escapeAttribute = (value: string) =>
	value
		.replaceAll("&", "&amp;")
		.replaceAll('"', "&quot;")
		.replaceAll("<", "&lt;");

export const imgTag = (tag: {
	pageUrl: string;
	imgUrl: string;
	name: string;
	title: null | string;
}) =>
	`<a href="${tag.pageUrl}"><img src="${tag.imgUrl}" title="${escapeAttribute(
		tag.title || tag.name,
	)}" alt="${escapeAttribute(
		`${tag.title ? `${tag.title} for ` : ""}${tag.name} - Bencher`,
	)}" /></a>`;

export const embedTag = (tag: {
	embedUrl: string;
	name: string;
	title: null | string;
}) =>
	`<iframe src="${tag.embedUrl}" title="${escapeAttribute(
		tag.title || tag.name,
	)}" width="100%" height="${embedHeight}px" allow="fullscreen"></iframe>`;
