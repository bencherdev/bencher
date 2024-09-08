import type { Slug } from "../types/bencher";

export const echoPath = (pathname: string) => pathname;

export const parentPath = (pathname: string) => {
	return `${pathname.substring(0, pathname.lastIndexOf("/"))}`;
};

export const createdSlugPath = (pathname: string, datum: { slug: Slug }) => {
	return `${pathname.substring(0, pathname.lastIndexOf("/"))}/${datum?.slug}`;
};

export const createdUuidPath = (pathname: string, datum: { uuid: Slug }) => {
	return `${pathname.substring(0, pathname.lastIndexOf("/"))}/${datum?.uuid}`;
};

export const addPath = (pathname: string) => {
	return `${pathname}/add`;
};

export const invitePath = (pathname: string) => {
	return `${pathname}/invite`;
};

export const viewSlugPath = (pathname: string, datum: { slug: Slug }) => {
	return `${pathname}/${datum?.slug}`;
};

export const viewUuidPath = (pathname: string, datum: { uuid: string }) => {
	return `${pathname}/${datum?.uuid}`;
};

export const resourcePath = (isConsole: undefined | boolean) =>
	isConsole !== false ? "/console/projects" : "/perf";

export const toCapitalized = (text: string) =>
	text.charAt(0).toUpperCase() + text.slice(1);

export const fmtDateTime = (date_time: string) =>
	new Date(date_time).toLocaleString(undefined, {
		weekday: "short",
		day: "numeric",
		month: "long",
		year: "numeric",
		hour: "numeric",
		minute: "numeric",
		second: "numeric",
		timeZoneName: "short",
	});
