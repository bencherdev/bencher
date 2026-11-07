// Which URLs a page may follow, in a module of its own, which a page that
// needs no more of `url` loads alone: `url` brings the error reporter.

export const sameOriginUrl = (
	url: undefined | null | string,
	origin: string,
): undefined | string => {
	if (!url) {
		return undefined;
	}
	try {
		const { href, origin: urlOrigin } = new URL(url, origin);
		return urlOrigin === origin ? href : undefined;
	} catch {
		return undefined;
	}
};

export const isWebUrl = (url: undefined | null | string): boolean => {
	if (!url) {
		return false;
	}
	try {
		const { protocol } = new URL(url);
		return protocol === "http:" || protocol === "https:";
	} catch {
		return false;
	}
};
