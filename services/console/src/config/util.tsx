import type { Slug } from "../types/bencher";

export const parentPath = (pathname: string) => {
	return `${pathname.substr(0, pathname.lastIndexOf("/"))}`;
};

export const addPath = (pathname: string) => {
	return `${pathname}/add`;
};

export const invitePath = (pathname: string) => {
	return `${pathname}/invite`;
};

export const viewSlugPath = (
	pathname: string,
	datum: { [slug: string]: Slug },
) => {
	return `${pathname}/${datum?.slug}`;
};

export const viewUuidPath = (
	pathname: string,
	datum: { [uuid: string]: string },
) => {
	return `${pathname}/${datum?.uuid}`;
};

export const toCapitalized = (text: string) =>
	text.charAt(0).toUpperCase() + text.slice(1);

export const date_time_fmt = (date_time: string) =>
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
